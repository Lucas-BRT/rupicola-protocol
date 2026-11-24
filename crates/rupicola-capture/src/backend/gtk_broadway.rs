use super::web::WebBackend;
use crate::session::Target;
use std::process::{Command, Stdio};

/// Program that serves GTK4 apps over HTTP.
pub(crate) const BROADWAY_DAEMON: &str = "gtk4-broadwayd";
/// First HTTP port; the display number is added to it.
const BASE_PORT: u16 = 8080;
/// How long the daemon gets to accept connections, how often to try, and how long one
/// attempt may take, in milliseconds.
pub(crate) const DAEMON_READY_TIMEOUT_MS: u64 = 5_000;
pub(crate) const DAEMON_POLL_MS: u64 = 50;
pub(crate) const CONNECT_TIMEOUT_MS: u64 = 200;

/// Native GTK4 capture backend using `gtk4-broadwayd` (HTML5 display server)
/// paired with [`WebBackend`]: captures real GTK4 apps without X11 or Wayland.
#[derive(Clone, Debug)]
pub struct GtkBroadwayBackend {
    pub display_num: u16,
    pub web: WebBackend,
}

impl GtkBroadwayBackend {
    /// Creates a Broadway backend on display `:display_num` (HTTP port `8080 + display_num`).
    #[must_use]
    pub const fn new(display_num: u16, web: WebBackend) -> Self {
        Self { display_num, web }
    }

    /// The HTTP port the daemon listens on.
    pub(crate) const fn port(&self) -> u16 {
        BASE_PORT.saturating_add(self.display_num)
    }

    /// The address the daemon listens on.
    pub(crate) fn address(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, self.port()))
    }

    fn display(&self) -> String {
        format!(":{}", self.display_num)
    }

    /// The URL the page is served at.
    pub(crate) fn page_url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port())
    }

    /// The command that starts the daemon.
    pub(crate) fn daemon(&self) -> Command {
        let mut command = Command::new(BROADWAY_DAEMON);
        command
            .args(["--address=127.0.0.1", &format!("--port={}", self.port())])
            .arg(self.display())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// The command that starts the GTK4 app on the daemon.
    pub(crate) fn app(&self, target: &Target) -> Command {
        let mut command = Command::new(&target.command);
        command
            .args(&target.args)
            .env("GDK_BACKEND", "broadway")
            .env("BROADWAY_DISPLAY", self.display())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// The message for a daemon that never listened.
    pub(crate) fn not_listening(&self) -> String {
        format!("{BROADWAY_DAEMON} did not listen on port {}", self.port())
    }
}
