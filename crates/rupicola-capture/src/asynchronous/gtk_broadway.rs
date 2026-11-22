use super::{CaptureBackend, CaptureSession, IntoTokio as _, tool::ToolCheck as _};
use crate::backend::GtkBroadwayBackend;
use crate::backend::gtk_broadway::{
    BROADWAY_DAEMON, CONNECT_TIMEOUT_MS, DAEMON_POLL_MS, DAEMON_READY_TIMEOUT_MS,
};
use crate::backend::tool::Tool;
use crate::session::{
    Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use async_trait::async_trait;
use rupicola_protocol::UiAction;
use std::path::Path;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::Child;
use tokio::time::{Instant, sleep, timeout};

impl GtkBroadwayBackend {
    /// Waits until the daemon accepts connections on its port.
    async fn await_daemon(&self) -> Result<(), CaptureError> {
        let address = self.address();
        let deadline = Instant::now() + Duration::from_millis(DAEMON_READY_TIMEOUT_MS);
        loop {
            let attempt = timeout(
                Duration::from_millis(CONNECT_TIMEOUT_MS),
                TcpStream::connect(address),
            )
            .await;
            if matches!(attempt, Ok(Ok(_))) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(CaptureError::environment(self.not_listening()));
            }
            sleep(Duration::from_millis(DAEMON_POLL_MS)).await;
        }
    }
}

#[async_trait]
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
        Tool(BROADWAY_DAEMON).require().await?;
        let daemon = self
            .daemon()
            .into_tokio()
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run {BROADWAY_DAEMON}: {e}")))?;
        // Dropped in reverse on any early return: `kill_on_drop` stops them.
        let mut processes = Processes(vec![daemon]);
        self.await_daemon().await?;

        let app = self
            .app(target)
            .into_tokio()
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        processes.0.push(app);
        sleep(Duration::from_millis(environment.warmup_ms)).await;

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
            let _ = child.kill().await;
        }
    }
}

/// A web session on the Broadway page, owning the processes that serve it.
struct BroadwaySession {
    inner: Box<dyn CaptureSession>,
    processes: Processes,
}

#[async_trait]
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
