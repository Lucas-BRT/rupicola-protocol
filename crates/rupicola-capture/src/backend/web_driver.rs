use super::web::WebBackend;
use crate::session::{CaptureError, ColorScheme, Environment, ErrorClass, Target, WaitCondition};
use rupicola_protocol::{UiAction, UiSnapshot};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A command for `playwright_session_driver.js`, one JSON line each.
#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub(crate) enum DriverRequest<'a> {
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
pub(crate) struct Frame {
    pub(crate) file: PathBuf,
    pub(crate) ms: f64,
}

/// The driver's answer to a command.
#[derive(Deserialize)]
pub(crate) struct DriverReply {
    pub(crate) ok: bool,
    #[serde(default)]
    pub(crate) class: Option<ErrorClass>,
    #[serde(default)]
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) snapshot: Option<UiSnapshot>,
    #[serde(default)]
    pub(crate) frames: Vec<Frame>,
}

const DRIVER_FAILED: &str = "the driver failed";

impl<'a> DriverRequest<'a> {
    /// The command that opens `target` in `environment` with the backend's browser.
    pub(crate) fn launch(
        backend: &'a WebBackend,
        target: &'a Target,
        environment: &Environment,
    ) -> Self {
        Self::Launch {
            engine: &backend.browser_engine,
            url: &target.command,
            viewport: environment.viewport,
            scale: environment.scale,
            color_scheme: environment.color_scheme,
            reduced_motion: environment.reduced_motion,
            warmup_ms: environment.warmup_ms,
        }
    }
}

impl DriverReply {
    /// The reply when the driver says it worked; otherwise the error it reports.
    pub(crate) fn into_result(self) -> Result<Self, CaptureError> {
        if self.ok {
            Ok(self)
        } else {
            Err(CaptureError {
                class: self.class.unwrap_or(ErrorClass::Target),
                message: self.error.unwrap_or_else(|| DRIVER_FAILED.to_owned()),
            })
        }
    }
}
