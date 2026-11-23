//! What a capture is, whichever way it runs: the declarative [`Plan`], the [`Environment`]
//! and [`Target`] it describes, and the [`Bundle`] of files it produces.
//!
//! A backend starts a session for a target once; the session then performs actions,
//! waits, takes screenshots and records clips for as long as the plan needs, so the target
//! launches once however many checkpoints are taken. The traits that run a plan live in
//! [`crate::sync`] (blocking).

use rupicola_protocol::UiSnapshot;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

mod capabilities;
mod error;
mod plan;

pub use capabilities::{Capabilities, Capability};
pub use error::{CaptureError, ErrorClass};
pub use plan::{Bundle, Plan, Preflight, Step};

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
    /// Locale of the target, such as `en-US`; backends that have none ignore it.
    #[serde(default)]
    pub locale: Option<String>,
    /// Longest the target gets for one step, in milliseconds; the backend's own limit when
    /// unset.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Milliseconds between the frames of a recording; the backend's own rate when unset.
    #[serde(default)]
    pub frame_interval_ms: Option<u64>,
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
            locale: None,
            timeout_ms: None,
            frame_interval_ms: None,
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
