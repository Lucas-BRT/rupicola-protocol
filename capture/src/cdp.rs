use crate::annotate::finalize_media;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Lightweight Web capture backend using a local Chromium/Chrome binary directly
/// (`--headless --screenshot`), requiring no Node.js or Playwright installation.
#[derive(Clone, Debug)]
pub struct CdpBackend {
    pub browser_bin: PathBuf,
}

impl CdpBackend {
    /// Discovers a Chromium-family browser binary in `PATH`.
    #[must_use]
    pub fn discover() -> Option<Self> {
        for candidate in ["chromium", "chromium-browser", "google-chrome", "brave"] {
            if Command::new("which")
                .arg(candidate)
                .output()
                .is_ok_and(|o| o.status.success())
            {
                return Some(Self {
                    browser_bin: PathBuf::from(candidate),
                });
            }
        }
        None
    }
}

impl CaptureBackend for CdpBackend {
    fn name(&self) -> &'static str {
        "web-chromium-headless"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;
        let screenshot_png = request.output_dir.join("preview.png");
        let win_size = format!(
            "--window-size={},{}",
            request.viewport[0], request.viewport[1]
        );

        let status = Command::new(&self.browser_bin)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
                &win_size,
            ])
            .arg(format!("--screenshot={}", screenshot_png.display()))
            .arg(&request.command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("failed to run {}: {e}", self.browser_bin.display()))?;

        if !status.success() || !screenshot_png.is_file() {
            return Err(format!("headless browser screenshot failed with {status}"));
        }

        finalize_media(
            &request.output_dir,
            screenshot_png,
            None,
            &request.annotations,
            None,
        )
    }
}
