use super::{CaptureBackend, CaptureSession};
use crate::backend::WebBackend;
use crate::backend::artifacts::Artifacts;
use crate::backend::web::{Encoder, FRAME_LIST, FRAMES_DIR};
use crate::backend::web_driver::{DriverReply, DriverRequest, Frame};
use crate::session::{
    Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use rupicola_protocol::{UiAction, read_ndjson, write_ndjson};
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout};

/// Finds `node` and a Playwright module.
pub trait WebDiscovery: Sized {
    /// Auto-discovers `node` and a local `playwright` module installation.
    #[must_use]
    fn discover(browser_engine: &str) -> Option<Self>;
}

impl WebDiscovery for WebBackend {
    fn discover(browser_engine: &str) -> Option<Self> {
        let candidates = Self::candidates()?;
        let node_bin = candidates.nodes.into_iter().find(|node| {
            (node.as_os_str() == "node"
                && Self::node_probe(node)
                    .status()
                    .is_ok_and(|status| status.success()))
                || node.is_file()
        })?;
        let playwright_module = candidates
            .modules
            .into_iter()
            .find(|module| module.exists())?;
        Some(Self {
            browser_engine: browser_engine.to_owned(),
            node_bin,
            playwright_module,
        })
    }
}

impl CaptureBackend for WebBackend {
    fn id(&self) -> &'static str {
        "web-playwright"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[
            Capability::Actions,
            Capability::UiTree,
            Capability::Video,
            Capability::ColorScheme,
            Capability::Scale,
        ])
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        let mut child = self
            .driver()
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run node: {e}")))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(CaptureError::environment("the driver has no pipes"));
        };
        let mut session = WebSession {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            artifacts: Artifacts::new(out),
        };
        session.call(&DriverRequest::launch(self, target, environment))?;
        Ok(Box::new(session))
    }
}

/// A browser page driven through a long-lived Playwright process.
struct WebSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    artifacts: Artifacts,
}

impl WebSession {
    /// Sends `request` and waits for the driver's answer.
    fn call(&mut self, request: &DriverRequest) -> Result<DriverReply, CaptureError> {
        write_ndjson(&mut self.stdin, request)
            .map_err(|e| CaptureError::environment(format!("write to the driver: {e}")))?;
        read_ndjson::<_, DriverReply>(&mut self.stdout)
            .map_err(|e| CaptureError::environment(format!("read from the driver: {e}")))?
            .ok_or_else(|| CaptureError::environment("the driver exited unexpectedly"))?
            .into_result()
    }

    /// Encodes `frames` into `mp4`, holding each until the next was taken.
    fn encode(frames: &[Frame], mp4: &Path, list: &Path) -> Result<(), CaptureError> {
        fs::write(list, Encoder::frame_list(frames))
            .map_err(|e| CaptureError::environment(format!("write the frame list: {e}")))?;
        let status = Encoder::command(list, mp4)
            .status()
            .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(CaptureError::environment(format!(
                "ffmpeg exited with {status}"
            )))
        }
    }

    fn stop_driver(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for WebSession {
    fn drop(&mut self) {
        self.stop_driver();
    }
}

impl CaptureSession for WebSession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        self.call(&DriverRequest::Perform { action }).map(|_| ())
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        self.call(&DriverRequest::Wait { condition }).map(|_| ())
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.artifacts.png(id)?;
        let reply = self.call(&DriverRequest::Screenshot { path: &png })?;
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: reply.snapshot,
        })
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        let dir = self.artifacts.file(FRAMES_DIR);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", dir.display())))?;
        self.call(&DriverRequest::StartRecording { dir: &dir })?;
        Ok(())
    }

    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        let mp4 = self.artifacts.mp4(id)?;
        let reply = self.call(&DriverRequest::StopRecording)?;
        let dir = self.artifacts.file(FRAMES_DIR);
        let encoded = if reply.frames.is_empty() {
            Err(CaptureError::environment("the recording has no frames"))
        } else {
            Self::encode(&reply.frames, &mp4, &dir.join(FRAME_LIST))
        };
        let _ = fs::remove_dir_all(&dir);
        encoded?;
        Ok(Clip {
            id: id.to_owned(),
            mp4,
        })
    }

    fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
        let closed = self.call(&DriverRequest::Close).map(|_| ());
        self.stop_driver();
        let _ = fs::remove_dir_all(self.artifacts.file(FRAMES_DIR));
        closed
    }
}
