use crate::annotate::annotate_png;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Headless Linux Wayland backend using an isolated nested compositor (`weston` or `cosmic-comp`)
/// and `grim` for screencopy capture.
#[derive(Clone, Debug)]
pub struct LinuxWaylandBackend {
    pub compositor: String,
    pub socket_name: String,
}

impl Default for LinuxWaylandBackend {
    fn default() -> Self {
        Self {
            compositor: "cosmic-comp".to_owned(),
            socket_name: "rupicola-wayland-0".to_owned(),
        }
    }
}

impl CaptureBackend for LinuxWaylandBackend {
    fn name(&self) -> &'static str {
        "linux-wayland"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        let has_comp = Command::new("which")
            .arg(&self.compositor)
            .output()
            .is_ok_and(|o| o.status.success());
        if !has_comp {
            return Err(format!(
                "Wayland compositor '{}' not found in PATH",
                self.compositor
            ));
        }
        let has_grim = Command::new("which")
            .arg("grim")
            .output()
            .is_ok_and(|o| o.status.success());
        if !has_grim {
            return Err(
                "Wayland screencopy tool 'grim' not found in PATH; install grim or use LinuxX11Backend"
                    .to_owned(),
            );
        }

        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;
        let runtime_dir = std::env::temp_dir().join(format!("rupicola-wl-{}", std::process::id()));
        fs::create_dir_all(&runtime_dir)
            .map_err(|e| format!("failed to create XDG_RUNTIME_DIR: {e}"))?;

        let mut comp = Command::new(&self.compositor)
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .env("WLR_BACKENDS", "headless")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start compositor '{}': {e}", self.compositor))?;
        thread::sleep(Duration::from_millis(600));

        let mut app = Command::new(&request.command)
            .args(&request.args)
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("WINIT_UNIX_BACKEND", "wayland")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                let _ = comp.kill();
                let _ = comp.wait();
                format!("failed to launch target app '{}': {e}", request.command)
            })?;
        thread::sleep(Duration::from_millis(request.warmup_ms));

        let screenshot_png = request.output_dir.join("preview.png");
        let grim = Command::new("grim")
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .arg(&screenshot_png)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("failed to run grim: {e}"));

        let _ = app.kill();
        let _ = app.wait();
        let _ = comp.kill();
        let _ = comp.wait();
        let _ = fs::remove_dir_all(&runtime_dir);

        let status = grim?;
        if !status.success() {
            return Err(format!("grim exited with {status}"));
        }

        let annotated_png = if request.annotations.is_empty() {
            None
        } else {
            let ann_path = request.output_dir.join("preview-annotated.png");
            annotate_png(&screenshot_png, &ann_path, &request.annotations, 1.0)?;
            Some(ann_path)
        };

        Ok(CaptureOutput {
            screenshot_png,
            annotated_png,
            video_mp4: None,
            annotated_video_mp4: None,
            video_gif: None,
            annotated_video_gif: None,
            snapshot: None,
        })
    }
}
