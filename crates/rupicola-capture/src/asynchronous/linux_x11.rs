use super::{CaptureBackend, CaptureSession, IntoTokio as _, xvfb::XvfbSession};
use crate::backend::LinuxX11Backend;
use crate::session::{Capabilities, Capability, CaptureError, Environment, Target};
use async_trait::async_trait;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;

#[async_trait]
impl CaptureBackend for LinuxX11Backend {
    fn id(&self) -> &'static str {
        "linux-x11-xvfb"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video, Capability::Pointer])
    }

    async fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out).await?;
        let app = Self::app(target, session.display())
            .into_tokio()
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        sleep(Duration::from_millis(environment.warmup_ms)).await;
        Ok(Box::new(session))
    }
}
