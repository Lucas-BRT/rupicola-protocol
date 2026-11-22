use crate::session::{
    self, Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use rupicola_protocol::UiAction;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Chromium-family programs tried, in order, when looking for a browser.
const BROWSER_CANDIDATES: [&str; 4] = ["chromium", "chromium-browser", "google-chrome", "brave"];

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
    /// Discovers a Chromium-family browser binary in `PATH`.
    #[must_use]
    pub fn discover() -> Option<Self> {
        BROWSER_CANDIDATES
            .into_iter()
            .find(|candidate| crate::tool::Tool(candidate).is_available())
            .map(|candidate| Self {
                browser_bin: PathBuf::from(candidate),
            })
    }
}

impl session::CaptureBackend for CdpBackend {
    fn id(&self) -> &'static str {
        "web-chromium-headless"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        Ok(Box::new(CdpSession {
            browser_bin: self.browser_bin.clone(),
            url: target.command.clone(),
            environment: environment.clone(),
            out: out.to_path_buf(),
        }))
    }
}

/// A page captured by one headless browser run per screenshot.
struct CdpSession {
    browser_bin: PathBuf,
    url: String,
    environment: Environment,
    out: PathBuf,
}

impl CdpSession {
    fn unsupported(what: &str) -> CaptureError {
        CaptureError::environment(format!(
            "this backend cannot {what}: it only takes screenshots"
        ))
    }
}

impl session::CaptureSession for CdpSession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        match action {
            UiAction::WaitMs { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            _ => Err(Self::unsupported("act on the page")),
        }
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        match condition {
            WaitCondition::Delay { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            WaitCondition::Visible { .. } | WaitCondition::Text { .. } => {
                Err(Self::unsupported("wait on the page"))
            }
        }
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.out.join(format!("{}.png", session::checked_id(id)?));
        let [width, height] = self.environment.viewport;
        let status = Command::new(&self.browser_bin)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
            ])
            .arg(format!("--window-size={width},{height}"))
            .arg(format!("--screenshot={}", png.display()))
            .arg(&self.url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| {
                CaptureError::environment(format!("run {}: {e}", self.browser_bin.display()))
            })?;
        if !status.success() || !png.is_file() {
            return Err(CaptureError::target(format!(
                "headless browser screenshot failed with {status}"
            )));
        }
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: None,
        })
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        Err(Self::unsupported("record"))
    }

    fn stop_recording(&mut self, _id: &str) -> Result<Clip, CaptureError> {
        Err(Self::unsupported("record"))
    }

    fn finish(self: Box<Self>) -> Result<(), CaptureError> {
        Ok(())
    }
}
