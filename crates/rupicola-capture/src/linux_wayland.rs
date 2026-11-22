use crate::session::{self, Capabilities, Capability, CaptureError, Environment, Target};
use crate::xvfb::XvfbSession;
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

/// Display number of the Xvfb the nested compositor draws on, unless the backend sets one.
const DEFAULT_DISPLAY_NUM: u16 = 88;
/// How many times, and how often, to look for the compositor's socket.
const SOCKET_POLLS: u32 = 50;
const SOCKET_POLL: Duration = Duration::from_millis(100);
/// Name of the D-Bus socket inside the private runtime directory.
const BUS_SOCKET: &str = "bus";
/// Compositor log, written next to the capture files.
const COMPOSITOR_LOG: &str = "compositor.log";

/// Headless Linux Wayland backend using an isolated nested compositor (`weston` or `cosmic-comp`)
/// and `grim` for screencopy capture.
#[derive(Clone, Debug)]
pub struct LinuxWaylandBackend {
    pub compositor: String,
    pub socket_name: String,
    /// X display the nested compositor draws on, for the session contract.
    pub display_num: u16,
}

impl Default for LinuxWaylandBackend {
    fn default() -> Self {
        Self {
            compositor: "cosmic-comp".to_owned(),
            socket_name: "rupicola-wayland-0".to_owned(),
            display_num: DEFAULT_DISPLAY_NUM,
        }
    }
}

impl LinuxWaylandBackend {
    /// The environment every process of the session shares: private runtime, home and
    /// bus, so nothing touches the user's own desktop session.
    fn private_env(command: &mut Command, scratch: &Path) {
        let runtime = scratch.join("runtime");
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", scratch.join("home"))
            .env("XDG_CONFIG_HOME", scratch.join("config"))
            .env("XDG_CACHE_HOME", scratch.join("cache"))
            .env("XDG_DATA_HOME", scratch.join("data"))
            .env("XDG_RUNTIME_DIR", &runtime)
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", runtime.join(BUS_SOCKET).display()),
            )
            .env("LIBGL_ALWAYS_SOFTWARE", "1");
    }

    /// Waits for the compositor to create its Wayland socket; returns its name.
    fn wait_for_socket(runtime: &Path, out: &Path) -> Result<String, CaptureError> {
        for _ in 0..SOCKET_POLLS {
            let socket = fs::read_dir(runtime).ok().and_then(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .find(|name| {
                        name.starts_with("wayland-") && Path::new(name).extension().is_none()
                    })
            });
            if let Some(socket) = socket {
                return Ok(socket);
            }
            thread::sleep(SOCKET_POLL);
        }
        Err(CaptureError::environment(format!(
            "the nested compositor did not create a Wayland socket; see {}",
            out.join(COMPOSITOR_LOG).display()
        )))
    }

    fn spawn_compositor(
        &self,
        session: &XvfbSession,
        scratch: &Path,
    ) -> Result<Child, CaptureError> {
        let log = fs::File::create(session.out().join(COMPOSITOR_LOG))
            .map_err(|e| CaptureError::environment(format!("create the compositor log: {e}")))?;
        let mut command = Command::new(&self.compositor);
        Self::private_env(&mut command, scratch);
        command
            .env("DISPLAY", session.display())
            .env("COSMIC_BACKEND", "winit")
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .map_err(|e| {
                CaptureError::environment(format!("start compositor '{}': {e}", self.compositor))
            })
    }
}

impl session::CaptureBackend for LinuxWaylandBackend {
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
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out)?;
        let scratch = session.scratch()?;
        let runtime = scratch.join("runtime");
        for dir in ["runtime", "home", "config", "cache", "data"] {
            fs::create_dir_all(scratch.join(dir))
                .map_err(|e| CaptureError::environment(format!("create {dir}: {e}")))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700));
        }
        let mut bus = Command::new("dbus-daemon");
        bus.args(["--session", "--nofork"])
            .arg(format!(
                "--address=unix:path={}",
                runtime.join(BUS_SOCKET).display()
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Apps tolerate a missing bus, so only a bus that is there gets adopted.
        if let Ok(bus) = bus.spawn() {
            session.adopt(bus);
        }
        let compositor = self.spawn_compositor(&session, &scratch)?;
        session.adopt(compositor);
        let socket = Self::wait_for_socket(&runtime, out)?;
        let mut app = Command::new(&target.command);
        Self::private_env(&mut app, &scratch);
        let app = app
            .args(&target.args)
            .env("WAYLAND_DISPLAY", socket)
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("WINIT_UNIX_BACKEND", "wayland")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));
        Ok(Box::new(session))
    }
}
