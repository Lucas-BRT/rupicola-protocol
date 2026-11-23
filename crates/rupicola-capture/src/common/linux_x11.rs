//! The X11 backend as a session, written once and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::LinuxX11Backend;
    use crate::common::rt::Runtime;
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::common::xvfb::flavour::XvfbSession;
    use crate::session::{Capabilities, Capability, CaptureError, Environment, Target};
    use std::path::Path;

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for LinuxX11Backend {
        fn id(&self) -> &'static str {
            "linux-x11-xvfb"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::of(&[Capability::Video, Capability::Pointer])
        }

        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            let mut session =
                XvfbSession::open(self.display_num, environment.viewport, out).await?;
            let app = Runtime::spawn(Self::app(target, session.display()))
                .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
            session.adopt(app);
            Runtime::sleep(environment.warmup_ms).await;
            Ok(Box::new(session))
        }
    }
}
