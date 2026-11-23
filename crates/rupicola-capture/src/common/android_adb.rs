//! The Android ADB backend as a session, written once and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::AndroidAdbBackend;
    use crate::backend::android_adb::{ADB, RECORDING_FLUSH_MS};
    use crate::backend::artifacts::Artifacts;
    use crate::backend::messages::{NO_RECORDING, NO_UI_TREE_WAIT, RECORDING_RUNNING};
    use crate::backend::tool::Tool;
    use crate::common::rt::{Child, Runtime};
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::session::{
        Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target,
        WaitCondition,
    };
    use rupicola_protocol::UiAction;
    use std::path::Path;

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for AndroidAdbBackend {
        fn id(&self) -> &'static str {
            "mobile-android-adb"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::of(&[Capability::Video])
        }

        async fn start(
            &self,
            _target: &Target,
            _environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            Runtime::require(Tool(ADB)).await?;
            let state = Runtime::output(self.get_state())
                .await
                .map_err(|e| CaptureError::environment(format!("run {ADB}: {e}")))?;
            if !state.status.success() {
                return Err(CaptureError::environment(format!(
                    "no Android device: {}",
                    String::from_utf8_lossy(&state.stderr).trim()
                )));
            }
            Runtime::create_dir_all(out)
                .await
                .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
            Ok(Box::new(AndroidSession {
                backend: self.clone(),
                artifacts: Artifacts::new(out),
                recording: None,
            }))
        }
    }

    struct AndroidSession {
        backend: AndroidAdbBackend,
        artifacts: Artifacts,
        recording: Option<Child>,
    }

    impl AndroidSession {
        fn unsupported() -> CaptureError {
            CaptureError::environment(
                "this backend cannot act on the device: only wait-ms is supported",
            )
        }

        async fn discard_recording(&mut self) {
            if let Some(mut child) = self.recording.take() {
                child.kill().await;
            }
            let _ = Runtime::status(self.backend.remove()).await;
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for AndroidSession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            match action {
                UiAction::WaitMs { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                _ => Err(Self::unsupported()),
            }
        }

        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            match condition {
                WaitCondition::Delay { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                WaitCondition::Visible { .. } | WaitCondition::Text { .. } => {
                    Err(CaptureError::environment(NO_UI_TREE_WAIT))
                }
            }
        }

        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            let png = self.artifacts.png(id)?;
            let output = Runtime::output(self.backend.screencap())
                .await
                .map_err(|e| CaptureError::environment(format!("run {ADB} screencap: {e}")))?;
            if !output.status.success() || output.stdout.is_empty() {
                return Err(CaptureError::target(format!(
                    "adb screencap failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
            Runtime::write(&png, &output.stdout)
                .await
                .map_err(|e| CaptureError::environment(format!("write {}: {e}", png.display())))?;
            Ok(Checkpoint {
                id: id.to_owned(),
                png,
                snapshot: None,
            })
        }

        async fn start_recording(&mut self) -> Result<(), CaptureError> {
            if self.recording.is_some() {
                return Err(CaptureError::environment(RECORDING_RUNNING));
            }
            let child = Runtime::spawn(self.backend.record())
                .map_err(|e| CaptureError::environment(format!("run {ADB} screenrecord: {e}")))?;
            self.recording = Some(child);
            Ok(())
        }

        async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
            let mp4 = self.artifacts.mp4(id)?;
            let Some(mut child) = self.recording.take() else {
                return Err(CaptureError::environment(NO_RECORDING));
            };
            let stopped = Runtime::status(self.backend.stop_record())
                .await
                .map_err(|e| CaptureError::environment(format!("run {ADB}: {e}")))?
                .success();
            let _ = child.wait().await;
            Runtime::sleep(RECORDING_FLUSH_MS).await;
            let pulled = stopped
                && Runtime::status(self.backend.pull(&mp4))
                    .await
                    .is_ok_and(|status| status.success());
            let _ = Runtime::status(self.backend.remove()).await;
            if pulled && Runtime::exists(&mp4).await {
                Ok(Clip {
                    id: id.to_owned(),
                    mp4,
                })
            } else {
                Err(CaptureError::target("the recording could not be retrieved"))
            }
        }

        async fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
            self.discard_recording().await;
            Ok(())
        }
    }
}
