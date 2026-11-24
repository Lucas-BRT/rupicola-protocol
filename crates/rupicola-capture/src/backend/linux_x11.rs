use crate::session::Target;
use std::process::{Command, Stdio};

/// Display number of the Xvfb a backend uses unless it sets one.
const DEFAULT_DISPLAY_NUM: u16 = 95;

/// Headless Linux X11 backend using `Xvfb` + `ffmpeg -f x11grab`.
#[derive(Clone, Debug)]
pub struct LinuxX11Backend {
    pub display_num: u16,
}

impl Default for LinuxX11Backend {
    fn default() -> Self {
        Self {
            display_num: DEFAULT_DISPLAY_NUM,
        }
    }
}

impl LinuxX11Backend {
    /// The command that starts the target on `display`.
    pub(crate) fn app(target: &Target, display: &str) -> Command {
        let mut command = Command::new(&target.command);
        command
            .args(&target.args)
            .env("GDK_BACKEND", "x11")
            .env("GSK_RENDERER", "cairo")
            .env("DISPLAY", display)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}
