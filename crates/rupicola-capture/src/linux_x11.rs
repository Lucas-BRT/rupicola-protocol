use crate::annotate::finalize_media;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use crate::session::{self, Capabilities, Capability, CaptureError, Environment, Target};
use crate::xvfb::XvfbSession;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
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

fn start_xvfb(size: &str) -> Result<(std::process::Child, String), String> {
    let screen = format!("{size}x24");

    let mut xvfb = Command::new("Xvfb")
        .args([
            "-displayfd",
            "1",
            "-screen",
            "0",
            &screen,
            "-nolisten",
            "tcp",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn Xvfb: {e}"))?;
    let mut number = String::new();
    let ready =
        BufReader::new(xvfb.stdout.take().expect("piped Xvfb stdout")).read_line(&mut number);
    let Ok(number) =
        ready.and_then(|_| number.trim().parse::<u16>().map_err(std::io::Error::other))
    else {
        let _ = xvfb.kill();
        let _ = xvfb.wait();
        return Err("Xvfb did not report a display number".into());
    };
    Ok((xvfb, format!(":{number}")))
}

impl CaptureBackend for LinuxX11Backend {
    fn name(&self) -> &'static str {
        "linux-x11-xvfb"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;

        let size = format!("{}x{}", request.viewport[0], request.viewport[1]);
        let (mut xvfb, display) = start_xvfb(&size)?;

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
            .output()
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
        if !status.status.success() {
            return Err(format!(
                "ffmpeg screenshot exited with {}: {}",
                status.status,
                String::from_utf8_lossy(&status.stderr).trim()
            ));
        }

        finalize_media(
            &request.output_dir,
            screenshot_png,
            video_mp4,
            &request.annotations,
            None,
        )
    }
}

impl session::CaptureBackend for LinuxX11Backend {
    fn id(&self) -> &'static str {
        "linux-x11-xvfb"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video, Capability::Pointer])
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        let mut session = XvfbSession::open(self.display_num, environment.viewport, out)?;
        let app = Command::new(&target.command)
            .args(&target.args)
            .env("GDK_BACKEND", "x11")
            .env("GSK_RENDERER", "cairo")
            .env("DISPLAY", session.display())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::target(format!("launch '{}': {e}", target.command)))?;
        session.adopt(app);
        thread::sleep(Duration::from_millis(environment.warmup_ms));
        Ok(Box::new(session))
    }
}
