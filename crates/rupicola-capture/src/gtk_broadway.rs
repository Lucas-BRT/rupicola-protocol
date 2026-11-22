use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use crate::web::WebBackend;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Native GTK4 capture backend using `gtk4-broadwayd` (HTML5 display server)
/// paired with [`WebBackend`] — captures real GTK4 apps without X11 or Wayland.
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
}

impl CaptureBackend for GtkBroadwayBackend {
    fn name(&self) -> &'static str {
        "linux-gtk4-broadway"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        let has_broadway = Command::new("which")
            .arg("gtk4-broadwayd")
            .output()
            .is_ok_and(|o| o.status.success());
        if !has_broadway {
            return Err("gtk4-broadwayd not found in PATH".to_owned());
        }

        let display = format!(":{}", self.display_num);
        let port = 8080_u16.saturating_add(self.display_num);
        let mut daemon = Command::new("gtk4-broadwayd")
            .args(["--address=127.0.0.1", &format!("--port={port}"), &display])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn gtk4-broadwayd: {e}"))?;
        thread::sleep(Duration::from_millis(400));

        let mut app = Command::new(&request.command)
            .args(&request.args)
            .env("GDK_BACKEND", "broadway")
            .env("BROADWAY_DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                let _ = daemon.kill();
                let _ = daemon.wait();
                format!("failed to launch GTK4 app '{}': {e}", request.command)
            })?;
        thread::sleep(Duration::from_millis(request.warmup_ms));

        let mut web_req = request.clone();
        web_req.command = format!("http://127.0.0.1:{port}/");
        let result = self.web.capture(&web_req);

        let _ = app.kill();
        let _ = app.wait();
        let _ = daemon.kill();
        let _ = daemon.wait();

        result
    }
}
