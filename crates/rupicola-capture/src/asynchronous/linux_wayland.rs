use super::{CaptureBackend, CaptureSession, IntoTokio as _, xvfb::XvfbSession};
use crate::backend::LinuxWaylandBackend;
use crate::backend::linux_wayland::{COMPOSITOR_LOG, SCRATCH_DIRS, SOCKET_POLL_MS, SOCKET_POLLS};
use crate::session::{Capabilities, Capability, CaptureError, Environment, Target};
use async_trait::async_trait;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Child;
use tokio::time::sleep;

impl LinuxWaylandBackend {
    /// The names in the runtime directory; empty when it cannot be read.
    async fn entries(runtime: &Path) -> Vec<String> {
        let mut names = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(runtime).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(name) = entry.file_name().into_string() {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Waits for the compositor to create its Wayland socket; returns its name.
    async fn await_socket(runtime: &Path, out: &Path) -> Result<String, CaptureError> {
        for _ in 0..SOCKET_POLLS {
            if let Some(socket) = Self::socket_among(Self::entries(runtime).await.into_iter()) {
                return Ok(socket);
            }
            sleep(Duration::from_millis(SOCKET_POLL_MS)).await;
        }
        Err(CaptureError::environment(Self::no_socket(out)))
    }

    async fn launch_compositor(
        &self,
        session: &XvfbSession,
        scratch: &Path,
    ) -> Result<Child, CaptureError> {
        let log = tokio::fs::File::create(session.out().join(COMPOSITOR_LOG))
            .await
            .map_err(|e| CaptureError::environment(format!("create the compositor log: {e}")))?
            .into_std()
            .await;
        self.compositor(session.display(), scratch, Stdio::from(log))
            .into_tokio()
            .spawn()
            .map_err(|e| {
                CaptureError::environment(format!("start compositor '{}': {e}", self.compositor))
            })
    }
}

#[async_trait]
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
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out).await?;
        let scratch = session.scratch().await?;
        let runtime = scratch.join(SCRATCH_DIRS[0]);
        for dir in SCRATCH_DIRS {
            tokio::fs::create_dir_all(scratch.join(dir))
                .await
                .map_err(|e| CaptureError::environment(format!("create {dir}: {e}")))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                tokio::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).await;
        }
        // Apps tolerate a missing bus, so only a bus that is there gets adopted.
        if let Ok(bus) = Self::bus(&runtime).into_tokio().spawn() {
            session.adopt(bus);
        }
        let compositor = self.launch_compositor(&session, &scratch).await?;
        session.adopt(compositor);
        let socket = Self::await_socket(&runtime, out).await?;
        let app = Self::app(target, &scratch, &socket)
            .into_tokio()
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        sleep(Duration::from_millis(environment.warmup_ms)).await;
        Ok(Box::new(session))
    }
}
