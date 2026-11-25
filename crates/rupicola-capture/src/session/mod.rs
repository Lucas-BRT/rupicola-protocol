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
mod network;
mod plan;
mod probe;

pub use capabilities::{Capabilities, Capability};
pub use error::{CaptureError, ErrorClass};
pub use network::NetworkPolicy;
pub use plan::{Bundle, Plan, Preflight, Step};
pub use probe::{BackendKind, BackendReport};

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

/// Whether a checkpoint keeps the UI tree of what it shows.
///
/// A tree lists the text on screen, which can be personal, so it is off unless a plan asks.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiTreeMode {
    /// No tree, and a backend does not collect one.
    #[default]
    Off,
    /// The layout: kinds, bounds and states, with no titles, labels or values.
    Structure,
    /// The whole tree, with its text.
    Full,
}

impl UiTreeMode {
    /// What a checkpoint keeps of `snapshot` under this mode.
    #[must_use]
    pub fn apply(self, snapshot: Option<UiSnapshot>) -> Option<UiSnapshot> {
        match self {
            Self::Off => None,
            Self::Structure => snapshot.map(|tree| tree.without_text()),
            Self::Full => snapshot,
        }
    }
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
    /// Extra milliseconds a web target gets after each action, on top of waiting for its
    /// scrolling and next paint to settle; none when unset.
    #[serde(default)]
    pub settle_ms: Option<u64>,
    /// Longest the target gets for one step, in milliseconds; the backend's own limit when
    /// unset.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Milliseconds between the frames of a recording; the backend's own rate when unset.
    #[serde(default)]
    pub frame_interval_ms: Option<u64>,
    /// Whether checkpoints keep the UI tree, and how much of it.
    #[serde(default)]
    pub ui_tree: UiTreeMode,
    /// Network access policy enforced on web capture targets and subresources.
    #[serde(default)]
    pub network_policy: NetworkPolicy,
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
            settle_ms: None,
            locale: None,
            timeout_ms: None,
            frame_interval_ms: None,
            ui_tree: UiTreeMode::Off,
            network_policy: NetworkPolicy::Unrestricted,
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

/// Minimum ratio of effective FPS to requested FPS before emitting a low-FPS warning.
const LOW_FPS_WARNING_RATIO: f64 = 0.75;

/// Measured frame cadence and rate telemetry for a recorded [`Clip`].
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClipTelemetry {
    /// Total raw compositor or screencast frame events received from the backend.
    pub raw_frames: usize,
    /// Raw frame events dropped by capture-side rate throttling.
    pub dropped_frames: usize,
    /// Frames written and encoded into the clip.
    pub encoded_frames: usize,
    /// Total recording duration in milliseconds.
    pub duration_ms: f64,
    /// Effective encoded frame rate (`encoded_frames / (duration_ms / 1000.0)`).
    pub effective_fps: f64,
    /// Requested target frame rate (`1000.0 / frame_interval_ms`), when configured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_fps: Option<f64>,
    /// Maximum gap between consecutive raw frame timestamps in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_raw_gap_ms: Option<f64>,
}

/// One recorded walkthrough.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Clip {
    pub id: String,
    pub mp4: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telemetry: Option<ClipTelemetry>,
}

impl Clip {
    /// Returns a warning if measured [`ClipTelemetry`] fell significantly below the requested FPS.
    #[must_use]
    pub fn low_fps_warning(&self) -> Option<String> {
        let telemetry = self.telemetry.as_ref()?;
        let requested = telemetry.requested_fps?;
        if requested <= 0.0 || telemetry.duration_ms <= 0.0 {
            return None;
        }
        if telemetry.effective_fps < requested * LOW_FPS_WARNING_RATIO {
            let gap_note = telemetry
                .max_raw_gap_ms
                .map_or_else(String::new, |gap| format!(", max raw gap {gap:.0} ms"));
            return Some(format!(
                "clip '{}' recorded {:.1} fps ({} encoded, {} raw, {} dropped over {:.0} ms{gap_note}), below requested {:.1} fps",
                self.id,
                telemetry.effective_fps,
                telemetry.encoded_frames,
                telemetry.raw_frames,
                telemetry.dropped_frames,
                telemetry.duration_ms,
                requested,
            ));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_fps_warning_triggers_only_below_threshold() {
        let low = Clip {
            id: "slow".to_owned(),
            mp4: PathBuf::from("slow.mp4"),
            telemetry: Some(ClipTelemetry {
                raw_frames: 17,
                dropped_frames: 0,
                encoded_frames: 17,
                duration_ms: 1284.0,
                effective_fps: 13.24,
                requested_fps: Some(62.5),
                max_raw_gap_ms: Some(650.0),
            }),
        };
        let warning = low.low_fps_warning().expect("expected low-fps warning");
        assert!(warning.contains("13.2 fps"));
        assert!(warning.contains("62.5 fps"));
        assert!(warning.contains("max raw gap 650 ms"));

        let ok = Clip {
            id: "fast".to_owned(),
            mp4: PathBuf::from("fast.mp4"),
            telemetry: Some(ClipTelemetry {
                raw_frames: 50,
                dropped_frames: 0,
                encoded_frames: 50,
                duration_ms: 1000.0,
                effective_fps: 50.0,
                requested_fps: Some(62.5),
                max_raw_gap_ms: Some(17.0),
            }),
        };
        assert_eq!(ok.low_fps_warning(), None);
    }
}
