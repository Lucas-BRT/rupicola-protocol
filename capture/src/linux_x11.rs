use crate::annotate::annotate_png;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Headless Linux X11 backend using `Xvfb` + `ffmpeg -f x11grab`.
#[derive(Clone, Debug)]
pub struct LinuxX11Backend {
    pub display_num: u16,
}

impl Default for LinuxX11Backend {
    fn default() -> Self {
        Self { display_num: 95 }
    }
}

impl CaptureBackend for LinuxX11Backend {
    fn name(&self) -> &'static str {
        "linux-x11-xvfb"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;

        let display = format!(":{}", self.display_num);
        let size = format!("{}x{}", request.viewport[0], request.viewport[1]);
        let screen = format!("{size}x24");

        let mut xvfb = Command::new("Xvfb")
            .args([&display, "-screen", "0", &screen])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn Xvfb: {e}"))?;
        thread::sleep(Duration::from_millis(500));

        let mut app = Command::new(&request.command)
            .args(&request.args)
            .env("GDK_BACKEND", "x11")
            .env("GSK_RENDERER", "cairo")
            .env("DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                let _ = xvfb.kill();
                let _ = xvfb.wait();
                format!("failed to launch target app '{}': {e}", request.command)
            })?;
        thread::sleep(Duration::from_millis(request.warmup_ms));

        let screenshot_png = request.output_dir.join("preview.png");
        let png_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "x11grab",
                "-video_size",
                &size,
                "-i",
                &display,
                "-frames:v",
                "1",
            ])
            .arg(&screenshot_png)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| format!("failed to run ffmpeg x11grab: {e}"));

        let video_mp4 = if let Some(ms) = request.record_video_ms.filter(|&m| m > 0) {
            let clamped = ms.max(500);
            let secs_str = format!("{}.{:03}", clamped / 1000, clamped % 1000);
            let mp4_path = request.output_dir.join("walkthrough.mp4");
            let vid_status = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-f",
                    "x11grab",
                    "-framerate",
                    "15",
                    "-t",
                    &secs_str,
                    "-video_size",
                    &size,
                    "-i",
                    &display,
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-movflags",
                    "+faststart",
                ])
                .arg(&mp4_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            match vid_status {
                Ok(s) if s.success() => Some(mp4_path),
                _ => None,
            }
        } else {
            None
        };

        let _ = app.kill();
        let _ = app.wait();
        let _ = xvfb.kill();
        let _ = xvfb.wait();

        let status = png_status?;
        if !status.success() {
            return Err(format!("ffmpeg screenshot exited with {status}"));
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
            video_mp4,
            snapshot: None,
        })
    }
}
