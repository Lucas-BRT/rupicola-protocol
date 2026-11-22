use serde::{Deserialize, Serialize};
use std::fmt;

/// Who is to blame for a failure, so callers can tell a broken machine from a broken app.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorClass {
    /// Missing tools, display servers or browsers; the backend could not run.
    Environment,
    /// The target failed to launch, crashed, or did not answer.
    Target,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureError {
    pub class: ErrorClass,
    pub message: String,
}

impl CaptureError {
    #[must_use]
    pub fn environment(message: impl Into<String>) -> Self {
        Self {
            class: ErrorClass::Environment,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn target(message: impl Into<String>) -> Self {
        Self {
            class: ErrorClass::Target,
            message: message.into(),
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = match self.class {
            ErrorClass::Environment => "environment",
            ErrorClass::Target => "target",
        };
        write!(f, "{class} error: {}", self.message)
    }
}

impl std::error::Error for CaptureError {}
