use crate::backend::tool::Tool;
use crate::session::CaptureError;

/// Checks for an external program by running `which`.
pub(crate) trait ToolCheck: Copy {
    fn is_available(self) -> bool;

    /// # Errors
    ///
    /// An [`crate::session::ErrorClass::Environment`] error naming the missing program.
    fn require(self) -> Result<(), CaptureError>;
}

impl ToolCheck for Tool {
    fn is_available(self) -> bool {
        self.probe().status().is_ok_and(|status| status.success())
    }

    fn require(self) -> Result<(), CaptureError> {
        if self.is_available() {
            Ok(())
        } else {
            Err(self.missing())
        }
    }
}
