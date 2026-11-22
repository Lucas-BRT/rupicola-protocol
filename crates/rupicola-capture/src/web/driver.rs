use crate::session::{ColorScheme, ErrorClass, WaitCondition};
use rupicola_protocol::{UiAction, UiSnapshot};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A command for `playwright_session_driver.js`, one JSON line each.
#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub(super) enum DriverRequest<'a> {
    Launch {
        engine: &'a str,
        url: &'a str,
        viewport: [u32; 2],
        scale: f64,
        color_scheme: Option<ColorScheme>,
        reduced_motion: bool,
        warmup_ms: u64,
    },
    Perform {
        action: &'a UiAction,
    },
    Wait {
        condition: &'a WaitCondition,
    },
    Screenshot {
        path: &'a Path,
    },
    StartRecording {
        dir: &'a Path,
    },
    StopRecording,
    Close,
}

/// One recorded frame: its file and when it was taken (Unix milliseconds).
#[derive(Deserialize)]
pub(super) struct Frame {
    pub(super) file: PathBuf,
    pub(super) ms: f64,
}

/// The driver's answer to a command.
#[derive(Deserialize)]
pub(super) struct DriverReply {
    pub(super) ok: bool,
    #[serde(default)]
    pub(super) class: Option<ErrorClass>,
    #[serde(default)]
    pub(super) error: Option<String>,
    #[serde(default)]
    pub(super) snapshot: Option<UiSnapshot>,
    #[serde(default)]
    pub(super) frames: Vec<Frame>,
}
