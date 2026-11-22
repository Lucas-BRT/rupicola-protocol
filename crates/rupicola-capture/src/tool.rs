use crate::session::CaptureError;
use std::process::Command;

/// An external program a backend needs on `PATH`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Tool(pub &'static str);

impl Tool {
    /// Whether the program is on `PATH`.
    pub(crate) fn is_available(self) -> bool {
        Command::new("which")
            .arg(self.0)
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// # Errors
    ///
    /// An [`crate::session::ErrorClass::Environment`] error naming the missing program.
    pub(crate) fn require(self) -> Result<(), CaptureError> {
        if self.is_available() {
            Ok(())
        } else {
            Err(CaptureError::environment(format!(
                "{} not found in PATH",
                self.0
            )))
        }
    }
}
