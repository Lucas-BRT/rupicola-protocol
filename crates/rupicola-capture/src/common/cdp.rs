//! The Chromium screenshot backend as a session, written once and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::CdpBackend;
    use crate::backend::artifacts::Artifacts;
    use crate::backend::cdp::BROWSER_CANDIDATES;
    use crate::backend::tool::Tool;
    use crate::common::rt::Runtime;
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::session::{
        Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
    };
    use rupicola_protocol::UiAction;
    use std::path::{Path, PathBuf};

    /// Finds a Chromium-family browser.
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    pub trait CdpDiscovery: Sized {
        /// Discovers a Chromium-family browser binary in `PATH`.
        async fn discover() -> Option<Self>;
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CdpDiscovery for CdpBackend {
        async fn discover() -> Option<Self> {
            for candidate in BROWSER_CANDIDATES {
                if Runtime::is_available(Tool(candidate)).await {
                    return Some(Self {
                        browser_bin: PathBuf::from(candidate),
                    });
                }
            }
            None
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for CdpBackend {
        fn id(&self) -> &'static str {
            "web-chromium-headless"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::default()
        }

        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            Runtime::create_dir_all(out)
                .await
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

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for CdpSession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            match action {
                UiAction::WaitMs { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                _ => Err(Self::unsupported("act on the page")),
            }
        }

        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            match condition {
                WaitCondition::Delay { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                WaitCondition::Visible { .. } | WaitCondition::Text { .. } => {
                    Err(Self::unsupported("wait on the page"))
                }
            }
        }

        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            let png = self.artifacts.png(id)?;
            let status =
                Runtime::status(self.backend.screenshot(&self.url, &self.environment, &png))
                    .await
                    .map_err(|e| {
                        CaptureError::environment(format!(
                            "run {}: {e}",
                            self.backend.browser_bin.display()
                        ))
                    })?;
            if !status.success() || !Runtime::exists(&png).await {
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

        async fn start_recording(&mut self) -> Result<(), CaptureError> {
            Err(Self::unsupported("record"))
        }

        async fn stop_recording(&mut self, _id: &str) -> Result<Clip, CaptureError> {
            Err(Self::unsupported("record"))
        }

        async fn finish(self: Box<Self>) -> Result<(), CaptureError> {
            Ok(())
        }
    }
}
