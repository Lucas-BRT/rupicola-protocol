use rupicola_protocol::{AnnotationBox, UiAction, UiSnapshot};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Declarative capture request executed by a [`CaptureBackend`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaptureRequest {
    /// Executable path (for desktop backends) or target URL (for web backends).
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub viewport: [u32; 2],
    #[serde(default = "default_warmup_ms")]
    pub warmup_ms: u64,
    /// Optional sequence of UI actions executed before taking the final snapshot.
    #[serde(default)]
    pub actions: Vec<UiAction>,
    /// When `Some(ms)` with `ms > 0`, also records an MP4 walkthrough clip.
    #[serde(default)]
    pub record_video_ms: Option<u64>,
    #[serde(default)]
    pub annotations: Vec<AnnotationBox>,
    pub output_dir: PathBuf,
}

const fn default_warmup_ms() -> u64 {
    1200
}

/// Standardized artifacts produced by a [`CaptureBackend`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaptureOutput {
    pub screenshot_png: PathBuf,
    #[serde(default)]
    pub annotated_png: Option<PathBuf>,
    #[serde(default)]
    pub video_mp4: Option<PathBuf>,
    #[serde(default)]
    pub annotated_video_mp4: Option<PathBuf>,
    #[serde(default)]
    pub video_gif: Option<PathBuf>,
    #[serde(default)]
    pub annotated_video_gif: Option<PathBuf>,
    #[serde(default)]
    pub snapshot: Option<UiSnapshot>,
}

/// Platform-specific headless capture backend.
pub trait CaptureBackend {
    fn name(&self) -> &'static str;

    /// Executes `request` and writes media artifacts under `request.output_dir`.
    ///
    /// # Errors
    ///
    /// Returns an error if dependencies are missing, the app fails to launch,
    /// or frame/video capture fails.
    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String>;
}
