use super::{CaptureBackend, CaptureSession, xvfb::XvfbSession};
use crate::backend::LinuxX11Backend;
use crate::session::{Capabilities, Capability, CaptureError, Environment, Target};
use std::path::Path;
use std::thread;
use std::time::Duration;

impl CaptureBackend for LinuxX11Backend {
    fn id(&self) -> &'static str {
        "linux-x11-xvfb"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video, Capability::Pointer])
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out)?;
        let app = Self::app(target, session.display())
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));
        Ok(Box::new(session))
    }
}
