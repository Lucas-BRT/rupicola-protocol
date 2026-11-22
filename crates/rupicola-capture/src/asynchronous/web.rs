use super::{CaptureBackend, CaptureSession, IntoTokio as _};
use crate::backend::WebBackend;
use crate::backend::artifacts::Artifacts;
use crate::backend::messages::{DRIVER_EXITED, DRIVER_NO_PIPES, NO_FRAMES};
use crate::backend::web::{Encoder, FRAME_LIST, FRAMES_DIR};
use crate::backend::web_driver::{DriverReply, DriverRequest, Frame};
use crate::session::{
    Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use async_trait::async_trait;
use rupicola_protocol::UiAction;
use std::path::Path;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

/// Finds `node` and a Playwright module.
#[async_trait]
pub trait WebDiscovery: Sized {
    /// Auto-discovers `node` and a local `playwright` module installation.
    async fn discover(browser_engine: &str) -> Option<Self>;
}

#[async_trait]
impl WebDiscovery for WebBackend {
    async fn discover(browser_engine: &str) -> Option<Self> {
        let candidates = Self::candidates()?;
        let mut node_bin = None;
        for node in candidates.nodes {
            let works = node.as_os_str() == "node"
                && Self::node_probe(&node)
                    .into_tokio()
                    .status()
                    .await
                    .is_ok_and(|status| status.success());
            let is_file = tokio::fs::metadata(&node)
                .await
                .is_ok_and(|metadata| metadata.is_file());
            if works || is_file {
                node_bin = Some(node);
                break;
            }
        }
        let mut playwright_module = None;
        for module in candidates.modules {
            if tokio::fs::try_exists(&module).await.unwrap_or(false) {
                playwright_module = Some(module);
                break;
            }
        }
        Some(Self {
            browser_engine: browser_engine.to_owned(),
            node_bin: node_bin?,
            playwright_module: playwright_module?,
        })
    }
}

#[async_trait]
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

    async fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        tokio::fs::create_dir_all(out)
            .await
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        let mut child = self
            .driver()
            .into_tokio()
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run node: {e}")))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(CaptureError::environment(DRIVER_NO_PIPES));
        };
        let mut session = WebSession {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            artifacts: Artifacts::new(out),
        };
        session
            .call(&DriverRequest::launch(self, target, environment))
            .await?;
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
    async fn call(&mut self, request: &DriverRequest<'_>) -> Result<DriverReply, CaptureError> {
        let line = request
            .to_line()
            .map_err(|e| CaptureError::environment(format!("write to the driver: {e}")))?;
        self.stdin
            .write_all(&line)
            .await
            .map_err(|e| CaptureError::environment(format!("write to the driver: {e}")))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| CaptureError::environment(format!("write to the driver: {e}")))?;
        let mut reply = String::new();
        let read = self
            .stdout
            .read_line(&mut reply)
            .await
            .map_err(|e| CaptureError::environment(format!("read from the driver: {e}")))?;
        if read == 0 {
            return Err(CaptureError::environment(DRIVER_EXITED));
        }
        DriverReply::from_line(&reply)
            .map_err(|e| CaptureError::environment(format!("read from the driver: {e}")))?
            .into_result()
    }

    /// Encodes `frames` into `mp4`, holding each until the next was taken.
    async fn encode(frames: &[Frame], mp4: &Path, list: &Path) -> Result<(), CaptureError> {
        tokio::fs::write(list, Encoder::frame_list(frames))
            .await
            .map_err(|e| CaptureError::environment(format!("write the frame list: {e}")))?;
        let status = Encoder::command(list, mp4)
            .into_tokio()
            .status()
            .await
            .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(CaptureError::environment(format!(
                "ffmpeg exited with {status}"
            )))
        }
    }

    async fn stop_driver(&mut self) {
        let _ = self.child.kill().await;
    }
}

#[async_trait]
impl CaptureSession for WebSession {
    async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        self.call(&DriverRequest::Perform { action })
            .await
            .map(|_| ())
    }

    async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        self.call(&DriverRequest::Wait { condition })
            .await
            .map(|_| ())
    }

    async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.artifacts.png(id)?;
        let reply = self.call(&DriverRequest::Screenshot { path: &png }).await?;
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: reply.snapshot,
        })
    }

    async fn start_recording(&mut self) -> Result<(), CaptureError> {
        let dir = self.artifacts.file(FRAMES_DIR);
        let _ = tokio::fs::remove_dir_all(&dir).await;
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", dir.display())))?;
        self.call(&DriverRequest::StartRecording { dir: &dir })
            .await?;
        Ok(())
    }

    async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        let mp4 = self.artifacts.mp4(id)?;
        let reply = self.call(&DriverRequest::StopRecording).await?;
        let dir = self.artifacts.file(FRAMES_DIR);
        let encoded = if reply.frames.is_empty() {
            Err(CaptureError::environment(NO_FRAMES))
        } else {
            Self::encode(&reply.frames, &mp4, &dir.join(FRAME_LIST)).await
        };
        let _ = tokio::fs::remove_dir_all(&dir).await;
        encoded?;
        Ok(Clip {
            id: id.to_owned(),
            mp4,
        })
    }

    async fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
        let closed = self.call(&DriverRequest::Close).await.map(|_| ());
        self.stop_driver().await;
        let _ = tokio::fs::remove_dir_all(self.artifacts.file(FRAMES_DIR)).await;
        closed
    }
}
