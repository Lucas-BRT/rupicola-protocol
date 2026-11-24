//! The nested-Wayland backend as a session, written once and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async"),
        wait_for_socket(fn),
        spawn_compositor(fn)
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::LinuxWaylandBackend;
    use crate::backend::linux_wayland::{
        COMPOSITOR_LOG, SCRATCH_DIRS, SOCKET_POLL_MS, SOCKET_POLLS,
    };
    use crate::common::rt::{Child, Runtime};
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::common::xvfb::flavour::XvfbSession;
    use crate::session::{Capabilities, Capability, CaptureError, Environment, Target};
    use std::path::Path;

    impl LinuxWaylandBackend {
        /// Waits for the compositor to create its Wayland socket; returns its name.
        async fn wait_for_socket(runtime: &Path, out: &Path) -> Result<String, CaptureError> {
            for _ in 0..SOCKET_POLLS {
                let names = Runtime::file_names(runtime).await;
                if let Some(socket) = Self::socket_among(names.into_iter()) {
                    return Ok(socket);
                }
                Runtime::sleep(SOCKET_POLL_MS).await;
            }
            Err(CaptureError::environment(Self::no_socket(out)))
        }

        /// An associated function rather than a method: the macro renames paths, not
        /// method calls.
        async fn spawn_compositor(
            backend: &Self,
            session: &XvfbSession,
            scratch: &Path,
        ) -> Result<Child, CaptureError> {
            let log = Runtime::log_file(&session.out().join(COMPOSITOR_LOG))
                .await
                .map_err(|e| {
                    CaptureError::environment(format!("create the compositor log: {e}"))
                })?;
            Runtime::spawn(backend.compositor(session.display(), scratch, log)).map_err(|e| {
                CaptureError::environment(format!("start compositor '{}': {e}", backend.compositor))
            })
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for LinuxWaylandBackend {
        fn id(&self) -> &'static str {
            "linux-wayland-nested"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::of(&[Capability::Video, Capability::Pointer])
        }

        /// Runs a nested compositor in its own window on a virtual X display, so the target
        /// is a real Wayland client and frames come from the display.
        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            let mut session =
                XvfbSession::open(self.display_num, environment.viewport, out).await?;
            let scratch = session.scratch().await?;
            let runtime = scratch.join(SCRATCH_DIRS[0]);
            for dir in SCRATCH_DIRS {
                Runtime::create_dir_all(&scratch.join(dir))
                    .await
                    .map_err(|e| CaptureError::environment(format!("create {dir}: {e}")))?;
            }
            let _ = Runtime::set_private(&runtime).await;
            // Apps tolerate a missing bus, so only a bus that is there gets adopted.
            if let Ok(bus) = Runtime::spawn(Self::bus(&runtime)) {
                session.adopt(bus);
            }
            let compositor = Self::spawn_compositor(self, &session, &scratch).await?;
            session.adopt(compositor);
            let socket = Self::wait_for_socket(&runtime, out).await?;
            let app = Runtime::spawn(Self::app(target, &scratch, &socket))
                .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
            session.adopt(app);
            Runtime::sleep(environment.warmup_ms).await;
            Ok(Box::new(session))
        }
    }
}
