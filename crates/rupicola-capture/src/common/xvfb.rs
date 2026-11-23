//! The session on a virtual X display, shared by the X11 and Wayland backends; written once
//! and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::artifacts::Artifacts;
    use crate::backend::messages::{NO_RECORDING, RECORDING_RUNNING};
    use crate::backend::xvfb::{PointerMove, RECORDING_FILE, Screen, XVFB_SETTLE_MS};
    use crate::common::rt::{Child, Runtime};
    use crate::common::traits::flavour::CaptureSession;
    use crate::session::{CaptureError, Checkpoint, Clip, WaitCondition};
    use rupicola_protocol::UiAction;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;

    /// A running Xvfb display with the session's processes on it.
    pub(crate) struct XvfbSession {
        screen: Screen,
        artifacts: Artifacts,
        /// Everything started for the session, in start order; stopped in reverse.
        processes: Vec<Child>,
        recording: Option<Child>,
        pointer: [i32; 2],
        scratch: Option<PathBuf>,
    }

    impl XvfbSession {
        /// Starts Xvfb as display `display_num`, sized to `viewport`; files go under `out`.
        ///
        /// # Errors
        ///
        /// An environment error when the output directory or Xvfb cannot be set up.
        pub(crate) async fn open(
            display_num: u16,
            viewport: [u32; 2],
            out: &Path,
        ) -> Result<Self, CaptureError> {
            Runtime::create_dir_all(out)
                .await
                .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
            let screen = Screen::new(display_num, viewport);
            let xvfb = Runtime::spawn(screen.server())
                .map_err(|e| CaptureError::environment(format!("spawn Xvfb: {e}")))?;
            Runtime::sleep(XVFB_SETTLE_MS).await;
            Ok(Self {
                pointer: screen.center(),
                screen,
                artifacts: Artifacts::new(out),
                processes: vec![xvfb],
                recording: None,
                scratch: None,
            })
        }

        /// The display name, such as `:93`.
        pub(crate) fn display(&self) -> &str {
            self.screen.display()
        }

        /// The directory files are written to.
        pub(crate) fn out(&self) -> &Path {
            self.artifacts.dir()
        }

        /// Hands a process to the session, which stops it on finish or drop.
        pub(crate) fn adopt(&mut self, child: Child) {
            self.processes.push(child);
        }

        /// A private scratch directory (mode 0700), removed when the session ends.
        ///
        /// # Errors
        ///
        /// An environment error when it cannot be created.
        pub(crate) async fn scratch(&mut self) -> Result<PathBuf, CaptureError> {
            if let Some(dir) = &self.scratch {
                return Ok(dir.clone());
            }
            let dir = std::env::temp_dir().join(format!(
                "rupicola-session-{}-{}",
                std::process::id(),
                self.screen.display().trim_start_matches(':')
            ));
            let _ = Runtime::remove_dir_all(&dir).await;
            Runtime::create_dir_all(&dir)
                .await
                .map_err(|e| CaptureError::environment(format!("create {}: {e}", dir.display())))?;
            let _ = Runtime::set_private(&dir).await;
            self.scratch = Some(dir.clone());
            Ok(dir)
        }

        /// Moves the pointer to `at`, then presses `button` `repeats` times (0 = only move).
        async fn pointer_input(
            &mut self,
            at: [i32; 2],
            button: u8,
            repeats: u32,
        ) -> Result<(), CaptureError> {
            let status = Runtime::status(self.screen.pointer(at, button, repeats))
                .await
                .map_err(|e| CaptureError::environment(format!("run python3: {e}")))?;
            if !status.success() {
                return Err(Screen::pointer_failed(status));
            }
            self.pointer = at;
            Ok(())
        }

        /// Stops everything the session started, the last started first.
        async fn shutdown(&mut self) {
            if let Some(mut recording) = self.recording.take() {
                recording.kill().await;
            }
            while let Some(mut child) = self.processes.pop() {
                child.kill().await;
            }
            if let Some(dir) = self.scratch.take() {
                let _ = Runtime::remove_dir_all(&dir).await;
            }
        }
    }

    impl Drop for XvfbSession {
        /// Children stop when dropped, the last started first; the scratch directory
        /// needs a blocking removal, which only matters when the session was dropped
        /// without `finish`.
        fn drop(&mut self) {
            drop(self.recording.take());
            while let Some(child) = self.processes.pop() {
                drop(child);
            }
            if let Some(dir) = self.scratch.take() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for XvfbSession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            for step in Screen::moves(action, self.pointer)? {
                match step {
                    PointerMove::Input {
                        at,
                        button,
                        repeats,
                    } => self.pointer_input(at, button, repeats).await?,
                    PointerMove::Pause { ms } => Runtime::sleep(ms).await,
                }
            }
            Ok(())
        }

        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            Runtime::sleep(Screen::wait_ms(condition)?).await;
            Ok(())
        }

        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            let png = self.artifacts.png(id)?;
            let mut command = self.screen.grab(false);
            command.args(["-frames:v", "1"]).arg(&png);
            let output = Runtime::output(command)
                .await
                .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
            if !output.status.success() {
                return Err(CaptureError::environment(format!(
                    "ffmpeg screenshot exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                )));
            }
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
            let mut command = self.screen.grab(true);
            command
                .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
                .args(["-movflags", "+faststart"])
                .arg(self.artifacts.file(RECORDING_FILE))
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit());
            let child = Runtime::spawn(command)
                .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
            self.recording = Some(child);
            Ok(())
        }

        async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
            let mp4 = self.artifacts.mp4(id)?;
            let mut child = self
                .recording
                .take()
                .ok_or_else(|| CaptureError::environment(NO_RECORDING))?;
            // `q` on stdin makes ffmpeg finish the file properly.
            let _ = child.write(b"q").await;
            child.close_stdin();
            let status = child
                .wait()
                .await
                .map_err(|e| CaptureError::environment(format!("wait for ffmpeg: {e}")))?;
            if !status.success() {
                return Err(CaptureError::environment(format!(
                    "ffmpeg recording exited with {status}"
                )));
            }
            Runtime::rename(&self.artifacts.file(RECORDING_FILE), &mp4)
                .await
                .map_err(|e| CaptureError::environment(format!("save the clip: {e}")))?;
            Ok(Clip {
                id: id.to_owned(),
                mp4,
            })
        }

        async fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
            self.shutdown().await;
            Ok(())
        }
    }
}
