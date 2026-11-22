use crate::session::Environment;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Chromium-family programs tried, in order, when looking for a browser.
pub(crate) const BROWSER_CANDIDATES: [&str; 4] =
    ["chromium", "chromium-browser", "google-chrome", "brave"];

/// Lightweight Web capture backend using a local Chromium/Chrome binary directly
/// (`--headless --screenshot`), requiring no Node.js or Playwright installation.
///
/// Every screenshot loads the page again, so the page cannot be driven between
/// checkpoints: the session only takes screenshots.
#[derive(Clone, Debug)]
pub struct CdpBackend {
    pub browser_bin: PathBuf,
}

impl CdpBackend {
    /// The command that loads `url` and saves a screenshot of it to `png`.
    pub(crate) fn screenshot(&self, url: &str, environment: &Environment, png: &Path) -> Command {
        let [width, height] = environment.viewport;
        let mut command = Command::new(&self.browser_bin);
        command
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
            ])
            .arg(format!("--window-size={width},{height}"))
            .arg(format!("--screenshot={}", png.display()))
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}
