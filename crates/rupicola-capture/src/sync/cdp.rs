use super::{CaptureBackend, CaptureSession, tool::ToolCheck as _};
use crate::backend::CdpBackend;
use crate::backend::artifacts::Artifacts;
use crate::backend::cdp::BROWSER_CANDIDATES;
use crate::backend::tool::Tool;
use crate::session::{
    Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use rupicola_protocol::UiAction;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

/// Finds a Chromium-family browser.
pub trait CdpDiscovery: Sized {
    /// Discovers a Chromium-family browser binary in `PATH`.
    #[must_use]
    fn discover() -> Option<Self>;
}

impl CdpDiscovery for CdpBackend {
    fn discover() -> Option<Self> {
        BROWSER_CANDIDATES
            .into_iter()
            .find(|candidate| Tool(candidate).is_available())
            .map(|candidate| Self {
                browser_bin: PathBuf::from(candidate),
            })
    }
}

impl CaptureBackend for CdpBackend {
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
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        Ok(Box::new(CdpSession {
            backend: self.clone(),
            url: target.command.clone(),
            environment: environment.clone(),
            artifacts: Artifacts::new(out),
        }))
    }
}

/// A page captured by one headless browser run per screenshot.
struct CdpSession {
    backend: CdpBackend,
    url: String,
    environment: Environment,
    artifacts: Artifacts,
}

impl CdpSession {
    fn unsupported(what: &str) -> CaptureError {
        CaptureError::environment(format!(
            "this backend cannot {what}: it only takes screenshots"
        ))
    }
}

impl CaptureSession for CdpSession {
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
        let png = self.artifacts.png(id)?;
        let status = self
            .backend
            .screenshot(&self.url, &self.environment, &png)
            .status()
            .map_err(|e| {
                CaptureError::environment(format!(
                    "run {}: {e}",
                    self.backend.browser_bin.display()
                ))
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
