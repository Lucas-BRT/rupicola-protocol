use super::IntoTokio as _;
use crate::backend::tool::Tool;
use crate::session::CaptureError;
use async_trait::async_trait;

/// Checks for an external program by running `which`.
#[async_trait]
pub(crate) trait ToolCheck: Copy + Send {
    async fn is_available(self) -> bool;

    /// # Errors
    ///
    /// An [`crate::session::ErrorClass::Environment`] error naming the missing program.
    async fn require(self) -> Result<(), CaptureError>;
}

#[async_trait]
impl ToolCheck for Tool {
    async fn is_available(self) -> bool {
        self.probe()
            .into_tokio()
            .status()
            .await
            .is_ok_and(|status| status.success())
    }

    async fn require(self) -> Result<(), CaptureError> {
        if self.is_available().await {
            Ok(())
        } else {
            Err(self.missing())
        }
    }
}
