use crate::session::CaptureError;
use std::process::{Command, Stdio};

/// An external program a backend needs on `PATH`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tool(pub &'static str);

impl Tool {
    /// The command that tells whether the program is on `PATH`.
    pub(crate) fn probe(self) -> Command {
        let mut command = Command::new("which");
        command
            .arg(self.0)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// The error for a program that is not on `PATH`.
    pub(crate) fn missing(self) -> CaptureError {
        CaptureError::environment(format!("{} not found in PATH", self.0))
    }
}
