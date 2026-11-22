use super::{CaptureBackend, CaptureSession, xvfb::XvfbSession};
use crate::backend::LinuxWaylandBackend;
use crate::backend::linux_wayland::{COMPOSITOR_LOG, SCRATCH_DIRS, SOCKET_POLL_MS, SOCKET_POLLS};
use crate::session::{Capabilities, Capability, CaptureError, Environment, Target};
use std::fs;
use std::path::Path;
use std::process::{Child, Stdio};
use std::thread;
use std::time::Duration;

impl LinuxWaylandBackend {
    /// Waits for the compositor to create its Wayland socket; returns its name.
    fn wait_for_socket(runtime: &Path, out: &Path) -> Result<String, CaptureError> {
        for _ in 0..SOCKET_POLLS {
            let names = fs::read_dir(runtime).ok().map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .collect::<Vec<_>>()
            });
            if let Some(socket) = Self::socket_among(names.into_iter().flatten()) {
                return Ok(socket);
            }
            thread::sleep(Duration::from_millis(SOCKET_POLL_MS));
        }
        Err(CaptureError::environment(Self::no_socket(out)))
    }

    fn spawn_compositor(
        &self,
        session: &XvfbSession,
        scratch: &Path,
    ) -> Result<Child, CaptureError> {
        let log = fs::File::create(session.out().join(COMPOSITOR_LOG))
            .map_err(|e| CaptureError::environment(format!("create the compositor log: {e}")))?;
        self.compositor(session.display(), scratch, Stdio::from(log))
            .spawn()
            .map_err(|e| {
                CaptureError::environment(format!("start compositor '{}': {e}", self.compositor))
            })
    }
}

impl CaptureBackend for LinuxWaylandBackend {
    fn id(&self) -> &'static str {
        "linux-wayland-nested"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video, Capability::Pointer])
    }

    /// Runs a nested compositor in its own window on a virtual X display, so the target
    /// is a real Wayland client and frames come from the display.
    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out)?;
        let scratch = session.scratch()?;
        let runtime = scratch.join(SCRATCH_DIRS[0]);
        for dir in SCRATCH_DIRS {
            fs::create_dir_all(scratch.join(dir))
                .map_err(|e| CaptureError::environment(format!("create {dir}: {e}")))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700));
        }
        // Apps tolerate a missing bus, so only a bus that is there gets adopted.
        if let Ok(bus) = Self::bus(&runtime).spawn() {
            session.adopt(bus);
        }
        let compositor = self.spawn_compositor(&session, &scratch)?;
        session.adopt(compositor);
        let socket = Self::wait_for_socket(&runtime, out)?;
        let app = Self::app(target, &scratch, &socket)
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));
        Ok(Box::new(session))
    }
}
