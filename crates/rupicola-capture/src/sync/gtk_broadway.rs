use super::{CaptureBackend, CaptureSession, tool::ToolCheck as _};
use crate::backend::GtkBroadwayBackend;
use crate::backend::gtk_broadway::{
    BROADWAY_DAEMON, CONNECT_TIMEOUT_MS, DAEMON_POLL_MS, DAEMON_READY_TIMEOUT_MS,
};
use crate::backend::tool::Tool;
use crate::session::{
    Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use rupicola_protocol::UiAction;
use std::net::TcpStream;
use std::path::Path;
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant};

impl GtkBroadwayBackend {
    /// Waits until the daemon accepts connections on its port.
    fn wait_for_daemon(&self) -> Result<(), CaptureError> {
        let address = self.address();
        let deadline = Instant::now() + Duration::from_millis(DAEMON_READY_TIMEOUT_MS);
        while TcpStream::connect_timeout(&address, Duration::from_millis(CONNECT_TIMEOUT_MS))
            .is_err()
        {
            if Instant::now() >= deadline {
                return Err(CaptureError::environment(self.not_listening()));
            }
            thread::sleep(Duration::from_millis(DAEMON_POLL_MS));
        }
        Ok(())
    }
}

impl CaptureBackend for GtkBroadwayBackend {
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
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        Tool(BROADWAY_DAEMON).require()?;
        let daemon = self
            .daemon()
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run {BROADWAY_DAEMON}: {e}")))?;
        let mut processes = Processes(vec![daemon]);
        self.wait_for_daemon()?;

        let app = self
            .app(target)
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        processes.0.push(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));

        let page = Target {
            command: self.page_url(),
            args: Vec::new(),
        };
        let inner = CaptureBackend::start(&self.web, &page, environment, out)?;
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
    inner: Box<dyn CaptureSession>,
    processes: Processes,
}

impl CaptureSession for BroadwaySession {
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
