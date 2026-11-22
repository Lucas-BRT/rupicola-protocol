//! Session-based capture contract.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once; the session then
//! performs actions, waits, takes screenshots and records clips for as long as the plan
//! needs, so the target launches once however many checkpoints are taken.
//! [`run_plan`] drives a session from a declarative [`Plan`], refusing up front a plan the
//! backend cannot honour unless the plan allows a degraded run.

use rupicola_protocol::{UiAction, UiSnapshot};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

mod capabilities;
mod error;
mod plan;
mod runner;
#[cfg(test)]
mod tests;

pub use capabilities::{Capabilities, Capability};
pub use error::{CaptureError, ErrorClass};
pub use plan::{Bundle, Plan, Step};
pub use runner::run_plan;

/// Pixel ratio used when a plan does not set one.
const DEFAULT_SCALE: f64 = 1.0;
/// Milliseconds a target gets to settle after launch unless the plan says otherwise.
const DEFAULT_WARMUP_MS: u64 = 1200;

const fn default_scale() -> f64 {
    DEFAULT_SCALE
}

const fn default_warmup_ms() -> u64 {
    DEFAULT_WARMUP_MS
}

/// Whether `id` is safe as a file name: ASCII letters, digits, `-` and `_`.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// `id` if it is a safe file name.
///
/// # Errors
///
/// An [`ErrorClass::Environment`] error naming the bad id; plans are checked with this
/// before anything starts.
pub fn checked_id(id: &str) -> Result<&str, CaptureError> {
    if valid_id(id) {
        Ok(id)
    } else {
        Err(CaptureError::environment(format!(
            "`{id}` is not a valid id: use letters, digits, - and _"
        )))
    }
}

/// What is captured: an executable (desktop backends) or a URL (web backends).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Target {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorScheme {
    Light,
    Dark,
}

/// How the target is shown.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Environment {
    pub viewport: [u32; 2],
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub color_scheme: Option<ColorScheme>,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default = "default_warmup_ms")]
    pub warmup_ms: u64,
}

impl Environment {
    /// A default environment with the given viewport.
    #[must_use]
    pub const fn new(viewport: [u32; 2]) -> Self {
        Self {
            viewport,
            scale: DEFAULT_SCALE,
            color_scheme: None,
            reduced_motion: false,
            warmup_ms: DEFAULT_WARMUP_MS,
        }
    }
}

/// A condition a step waits for.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "wait", rename_all = "kebab-case")]
pub enum WaitCondition {
    /// A fixed pause.
    Delay { ms: u64 },
    /// An element is on screen.
    Visible { target_id: String },
    /// Some text is on screen.
    Text { text: String },
}

/// One screenshot, with the UI tree when the backend has one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Checkpoint {
    pub id: String,
    pub png: PathBuf,
    pub snapshot: Option<UiSnapshot>,
}

/// One recorded walkthrough.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Clip {
    pub id: String,
    pub mp4: PathBuf,
}

/// A running capture of one target.
pub trait CaptureSession {
    /// # Errors
    ///
    /// Fails when the target does not accept the action.
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the condition is not met in time.
    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the frame cannot be grabbed.
    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError>;

    /// # Errors
    ///
    /// Fails when recording cannot start.
    fn start_recording(&mut self) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when no recording is running or it cannot be finished.
    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError>;

    /// Stops the target and everything started for it. Called exactly once, even after
    /// a failed step.
    ///
    /// # Errors
    ///
    /// Fails when cleanup does not complete.
    fn finish(self: Box<Self>) -> Result<(), CaptureError>;
}

/// A way of capturing: knows what it can do and how to start a [`CaptureSession`].
pub trait CaptureBackend {
    fn id(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    /// Launches `target` in `environment`; files go under `out`.
    ///
    /// # Errors
    ///
    /// [`ErrorClass::Environment`] when tools are missing, [`ErrorClass::Target`] when
    /// the target does not come up.
    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError>;
}
