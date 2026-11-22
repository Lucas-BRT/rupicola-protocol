use crate::session::{self, Capabilities, Capability, CaptureError, Environment, Target};
use crate::xvfb::XvfbSession;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Headless Linux X11 backend using `Xvfb` + `ffmpeg -f x11grab`.
#[derive(Clone, Debug)]
pub struct LinuxX11Backend {
    pub display_num: u16,
}

impl Default for LinuxX11Backend {
    fn default() -> Self {
        Self { display_num: 95 }
    }
}

impl session::CaptureBackend for LinuxX11Backend {
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
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out)?;
        let app = Command::new(&target.command)
            .args(&target.args)
            .env("GDK_BACKEND", "x11")
            .env("GSK_RENDERER", "cairo")
            .env("DISPLAY", session.display())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));
        Ok(Box::new(session))
    }
}
