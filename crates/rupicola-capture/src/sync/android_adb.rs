use super::{CaptureBackend, CaptureSession, tool::ToolCheck as _};
use crate::backend::AndroidAdbBackend;
use crate::backend::android_adb::{ADB, RECORDING_FLUSH_MS};
use crate::backend::artifacts::Artifacts;
use crate::backend::messages::{NO_RECORDING, NO_UI_TREE_WAIT, RECORDING_RUNNING};
use crate::backend::tool::Tool;
use crate::session::{
    Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use rupicola_protocol::UiAction;
use std::fs;
use std::path::Path;
use std::process::Child;
use std::thread;
use std::time::Duration;

impl CaptureBackend for AndroidAdbBackend {
    fn id(&self) -> &'static str {
        "mobile-android-adb"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video])
    }

    fn start(
        &self,
        _target: &Target,
        _environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        Tool(ADB).require()?;
        let state = self
            .get_state()
            .output()
            .map_err(|e| CaptureError::environment(format!("run {ADB}: {e}")))?;
        if !state.status.success() {
            return Err(CaptureError::environment(format!(
                "no Android device: {}",
                String::from_utf8_lossy(&state.stderr).trim()
            )));
        }
        fs::create_dir_all(out)
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

    fn discard_recording(&mut self) {
        if let Some(mut child) = self.recording.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.backend.remove().status();
    }
}

impl CaptureSession for AndroidSession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        match action {
            UiAction::WaitMs { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            _ => Err(Self::unsupported()),
        }
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        match condition {
            WaitCondition::Delay { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            WaitCondition::Visible { .. } | WaitCondition::Text { .. } => {
                Err(CaptureError::environment(NO_UI_TREE_WAIT))
            }
        }
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.artifacts.png(id)?;
        let output = self
            .backend
            .screencap()
            .output()
            .map_err(|e| CaptureError::environment(format!("run {ADB} screencap: {e}")))?;
        if !output.status.success() || output.stdout.is_empty() {
            return Err(CaptureError::target(format!(
                "adb screencap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        fs::write(&png, &output.stdout)
            .map_err(|e| CaptureError::environment(format!("write {}: {e}", png.display())))?;
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: None,
        })
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        if self.recording.is_some() {
            return Err(CaptureError::environment(RECORDING_RUNNING));
        }
        let child = self
            .backend
            .record()
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run {ADB} screenrecord: {e}")))?;
        self.recording = Some(child);
        Ok(())
    }

    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        let mp4 = self.artifacts.mp4(id)?;
        let Some(mut child) = self.recording.take() else {
            return Err(CaptureError::environment(NO_RECORDING));
        };
        let stopped = self
            .backend
            .stop_record()
            .status()
            .map_err(|e| CaptureError::environment(format!("run {ADB}: {e}")))?
            .success();
        let _ = child.wait();
        thread::sleep(Duration::from_millis(RECORDING_FLUSH_MS));
        let pulled = stopped
            && self
                .backend
                .pull(&mp4)
                .status()
                .is_ok_and(|status| status.success());
        let _ = self.backend.remove().status();
        if pulled && mp4.is_file() {
            Ok(Clip {
                id: id.to_owned(),
                mp4,
            })
        } else {
            Err(CaptureError::target("the recording could not be retrieved"))
        }
    }

    fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
        self.discard_recording();
        Ok(())
    }
}
