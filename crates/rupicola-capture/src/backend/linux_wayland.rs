use crate::session::Target;
use std::path::Path;
use std::process::{Command, Stdio};

/// Display number of the Xvfb the nested compositor draws on, unless the backend sets one.
const DEFAULT_DISPLAY_NUM: u16 = 88;
/// Milliseconds between looks for the compositor's socket, and how many looks.
pub(crate) const SOCKET_POLLS: u32 = 50;
pub(crate) const SOCKET_POLL_MS: u64 = 100;
/// Name of the D-Bus socket inside the private runtime directory.
const BUS_SOCKET: &str = "bus";
/// Compositor log, written next to the capture files.
pub(crate) const COMPOSITOR_LOG: &str = "compositor.log";
/// Folders of the private scratch directory; the first is the runtime directory.
pub(crate) const SCRATCH_DIRS: [&str; 5] = ["runtime", "home", "config", "cache", "data"];

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

    /// The command that starts the private D-Bus daemon.
    pub(crate) fn bus(runtime: &Path) -> Command {
        let mut command = Command::new("dbus-daemon");
        command
            .args(["--session", "--nofork"])
            .arg(format!(
                "--address=unix:path={}",
                runtime.join(BUS_SOCKET).display()
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// The command that starts the nested compositor on `display`, logging to `log`.
    pub(crate) fn compositor(&self, display: &str, scratch: &Path, log: Stdio) -> Command {
        let mut command = Command::new(&self.compositor);
        Self::private_env(&mut command, scratch);
        command
            .env("DISPLAY", display)
            .env("COSMIC_BACKEND", "winit")
            .stdout(Stdio::null())
            .stderr(log);
        command
    }

    /// The command that starts the target as a client of the compositor's `socket`.
    pub(crate) fn app(target: &Target, scratch: &Path, socket: &str) -> Command {
        let mut command = Command::new(&target.command);
        Self::private_env(&mut command, scratch);
        command
            .args(&target.args)
            .env("WAYLAND_DISPLAY", socket)
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("WINIT_UNIX_BACKEND", "wayland")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// Which of the runtime directory's entries is a Wayland socket.
    pub(crate) fn socket_among(names: impl Iterator<Item = String>) -> Option<String> {
        names
            .into_iter()
            .find(|name| name.starts_with("wayland-") && Path::new(name).extension().is_none())
    }

    /// The message for a compositor that never created its socket.
    pub(crate) fn no_socket(out: &Path) -> String {
        format!(
            "the nested compositor did not create a Wayland socket; see {}",
            out.join(COMPOSITOR_LOG).display()
        )
    }
}
