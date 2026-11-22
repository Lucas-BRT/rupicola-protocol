use crate::session::{
    self, Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target,
    WaitCondition,
};
use crate::tool::Tool;
use crate::web::WebBackend;
use rupicola_protocol::UiAction;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Program that serves GTK4 apps over HTTP.
const BROADWAY_DAEMON: &str = "gtk4-broadwayd";
/// First HTTP port; the display number is added to it.
const BASE_PORT: u16 = 8080;
/// How long the daemon gets to accept connections.
const DAEMON_READY_TIMEOUT: Duration = Duration::from_secs(5);
/// Pause between connection attempts while the daemon starts.
const DAEMON_POLL: Duration = Duration::from_millis(50);
/// Longest wait for a single connection attempt.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(200);

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

    const fn port(&self) -> u16 {
        BASE_PORT.saturating_add(self.display_num)
    }

    /// Waits until the daemon accepts connections on its port.
    fn wait_for_daemon(&self) -> Result<(), CaptureError> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.port()));
        let deadline = Instant::now() + DAEMON_READY_TIMEOUT;
        while TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).is_err() {
            if Instant::now() >= deadline {
                return Err(CaptureError::environment(format!(
                    "{BROADWAY_DAEMON} did not listen on port {}",
                    self.port()
                )));
            }
            thread::sleep(DAEMON_POLL);
        }
        Ok(())
    }
}

impl session::CaptureBackend for GtkBroadwayBackend {
    fn id(&self) -> &'static str {
        "linux-gtk4-broadway"
    }

    /// The page shows a canvas, so only what works on screen coordinates carries over
    /// from the web session.
    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Pointer, Capability::Video])
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        Tool(BROADWAY_DAEMON).require()?;
        let display = format!(":{}", self.display_num);
        let daemon = Command::new(BROADWAY_DAEMON)
            .args(["--address=127.0.0.1", &format!("--port={}", self.port())])
            .arg(&display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run {BROADWAY_DAEMON}: {e}")))?;
        let mut processes = Processes(vec![daemon]);
        self.wait_for_daemon()?;

        let app = Command::new(&target.command)
            .args(&target.args)
            .env("GDK_BACKEND", "broadway")
            .env("BROADWAY_DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        processes.0.push(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));

        let page = Target {
            command: format!("http://127.0.0.1:{}/", self.port()),
            args: Vec::new(),
        };
        let inner = session::CaptureBackend::start(&self.web, &page, environment, out)?;
        Ok(Box::new(BroadwaySession { inner, processes }))
    }
}

/// The daemon and the app, stopped together.
struct Processes(Vec<Child>);

impl Drop for Processes {
    fn drop(&mut self) {
        // The app first, then the daemon it talks to.
        for child in self.0.iter_mut().rev() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// A web session on the Broadway page, owning the processes that serve it.
struct BroadwaySession {
    inner: Box<dyn session::CaptureSession>,
    processes: Processes,
}

impl session::CaptureSession for BroadwaySession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        self.inner.perform(action)
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        self.inner.wait_for(condition)
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        self.inner.screenshot(id)
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        self.inner.start_recording()
    }

    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        self.inner.stop_recording(id)
    }

    fn finish(self: Box<Self>) -> Result<(), CaptureError> {
        let Self { inner, processes } = *self;
        let closed = inner.finish();
        drop(processes);
        closed
    }
}
