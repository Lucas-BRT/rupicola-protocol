//! The GTK4 Broadway backend as a session, written once and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async"),
        wait_for_daemon(fn)
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::GtkBroadwayBackend;
    use crate::backend::gtk_broadway::{
        BROADWAY_DAEMON, CONNECT_TIMEOUT_MS, DAEMON_POLL_MS, DAEMON_READY_TIMEOUT_MS,
    };
    use crate::backend::tool::Tool;
    use crate::common::rt::{Child, Runtime};
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::session::{
        Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target,
        WaitCondition,
    };
    use rupicola_protocol::UiAction;
    use std::path::Path;
    use std::time::{Duration, Instant};

    impl GtkBroadwayBackend {
        /// Waits until the daemon accepts connections on its port. An associated function
        /// rather than a method: the macro renames paths, not method calls.
        async fn wait_for_daemon(backend: &Self) -> Result<(), CaptureError> {
            let address = backend.address();
            let deadline = Instant::now() + Duration::from_millis(DAEMON_READY_TIMEOUT_MS);
            while !Runtime::connects(address, CONNECT_TIMEOUT_MS).await {
                if Instant::now() >= deadline {
                    return Err(CaptureError::environment(backend.not_listening()));
                }
                Runtime::sleep(DAEMON_POLL_MS).await;
            }
            Ok(())
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for GtkBroadwayBackend {
        fn id(&self) -> &'static str {
            "linux-gtk4-broadway"
        }

        /// The page shows a canvas, so only what works on screen coordinates carries over
        /// from the web session.
        fn capabilities(&self) -> Capabilities {
            Capabilities::of(&[Capability::Pointer, Capability::Video])
        }

        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            Runtime::require(Tool(BROADWAY_DAEMON)).await?;
            let daemon = Runtime::spawn(self.daemon())
                .map_err(|e| CaptureError::environment(format!("run {BROADWAY_DAEMON}: {e}")))?;
            // Dropped, and so stopped, on any early return.
            let mut processes = Processes(vec![daemon]);
            Self::wait_for_daemon(self).await?;

            let app = Runtime::spawn(self.app(target))
                .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
            processes.0.push(app);
            Runtime::sleep(environment.warmup_ms).await;

            let page = Target {
                command: self.page_url(),
                args: Vec::new(),
            };
            let inner = CaptureBackend::start(&self.web, &page, environment, out).await?;
            Ok(Box::new(BroadwaySession { inner, processes }))
        }
    }

    /// The daemon and the app, stopped together.
    struct Processes(Vec<Child>);

    impl Processes {
        /// The app first, then the daemon it talks to.
        async fn stop(mut self) {
            while let Some(mut child) = self.0.pop() {
                child.kill().await;
            }
        }
    }

    impl Drop for Processes {
        fn drop(&mut self) {
            while let Some(child) = self.0.pop() {
                drop(child);
            }
        }
    }

    /// A web session on the Broadway page, owning the processes that serve it.
    struct BroadwaySession {
        inner: Box<dyn CaptureSession>,
        processes: Processes,
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for BroadwaySession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            self.inner.perform(action).await
        }

        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            self.inner.wait_for(condition).await
        }

        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            self.inner.screenshot(id).await
        }

        async fn start_recording(&mut self) -> Result<(), CaptureError> {
            self.inner.start_recording().await
        }

        async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
            self.inner.stop_recording(id).await
        }

        async fn finish(self: Box<Self>) -> Result<(), CaptureError> {
            let Self { inner, processes } = *self;
            let closed = inner.finish().await;
            processes.stop().await;
            closed
        }
    }
}
