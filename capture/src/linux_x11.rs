use crate::annotate::finalize_media;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use crate::session::{
    self, Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target,
    WaitCondition,
};
use rupicola_protocol::UiAction;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

/// Time Xvfb gets to open its display before anything connects.
const XVFB_SETTLE: Duration = Duration::from_millis(500);
/// Colour depth of the virtual screen, in bits.
const SCREEN_DEPTH: u32 = 24;
/// Frames per second of recorded clips.
const RECORDING_FRAMERATE: &str = "15";
/// Name of a clip while it is being recorded, before it has an id.
const RECORDING_FILE: &str = ".recording.mp4";

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

        finalize_media(
            &request.output_dir,
            screenshot_png,
            video_mp4,
            &request.annotations,
            None,
        )
    }
}

impl LinuxX11Backend {
    fn display(&self) -> String {
        format!(":{}", self.display_num)
    }
}

impl session::CaptureBackend for LinuxX11Backend {
    fn id(&self) -> &'static str {
        "linux-x11-xvfb"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video])
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        let display = self.display();
        let [width, height] = environment.viewport;
        let mut xvfb = Command::new("Xvfb")
            .args([
                &display,
                "-screen",
                "0",
                &format!("{width}x{height}x{SCREEN_DEPTH}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::environment(format!("spawn Xvfb: {e}")))?;
        thread::sleep(XVFB_SETTLE);
        let app = Command::new(&target.command)
            .args(&target.args)
            .env("GDK_BACKEND", "x11")
            .env("GSK_RENDERER", "cairo")
            .env("DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let app = match app {
            Ok(app) => app,
            Err(error) => {
                let _ = xvfb.kill();
                let _ = xvfb.wait();
                return Err(CaptureError::target(format!(
                    "launch '{}': {error}",
                    target.command
                )));
            }
        };
        thread::sleep(Duration::from_millis(environment.warmup_ms));
        Ok(Box::new(X11Session {
            xvfb,
            app,
            display,
            size: format!("{width}x{height}"),
            out: out.to_path_buf(),
            recording: None,
        }))
    }
}

/// A running Xvfb display with the target on it.
struct X11Session {
    xvfb: Child,
    app: Child,
    display: String,
    size: String,
    out: PathBuf,
    recording: Option<Child>,
}

impl X11Session {
    /// `ffmpeg` arguments that read the display, up to the output file.
    fn grab(&self) -> Command {
        let mut command = Command::new("ffmpeg");
        command
            .args(["-y", "-f", "x11grab", "-video_size", &self.size])
            .args(["-i", &self.display]);
        command
    }

    /// Stops everything the session started; safe to call twice.
    fn shutdown(&mut self) {
        for child in self
            .recording
            .iter_mut()
            .chain([&mut self.app, &mut self.xvfb])
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.recording = None;
    }
}

impl Drop for X11Session {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl session::CaptureSession for X11Session {
    fn perform(&mut self, _: &UiAction) -> Result<(), CaptureError> {
        Err(CaptureError::environment(
            "the X11 backend cannot perform actions",
        ))
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        match condition {
            WaitCondition::Delay { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            WaitCondition::Visible { .. } | WaitCondition::Text { .. } => Err(
                CaptureError::environment("the X11 backend has no UI tree to wait on"),
            ),
        }
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.out.join(format!("{}.png", session::checked_id(id)?));
        let status = self
            .grab()
            .args(["-frames:v", "1"])
            .arg(&png)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
        if !status.success() {
            return Err(CaptureError::environment(format!(
                "ffmpeg screenshot exited with {status}"
            )));
        }
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: None,
        })
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        if self.recording.is_some() {
            return Err(CaptureError::environment("a recording is already running"));
        }
        let child = self
            .grab()
            .args(["-framerate", RECORDING_FRAMERATE])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(self.out.join(RECORDING_FILE))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
        self.recording = Some(child);
        Ok(())
    }

    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        let mp4 = self.out.join(format!("{}.mp4", session::checked_id(id)?));
        let mut child = self
            .recording
            .take()
            .ok_or_else(|| CaptureError::environment("no recording is running"))?;
        // `q` on stdin makes ffmpeg finish the file properly.
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(b"q");
        }
        let status = child
            .wait()
            .map_err(|e| CaptureError::environment(format!("wait for ffmpeg: {e}")))?;
        if !status.success() {
            return Err(CaptureError::environment(format!(
                "ffmpeg recording exited with {status}"
            )));
        }
        fs::rename(self.out.join(RECORDING_FILE), &mp4)
            .map_err(|e| CaptureError::environment(format!("save the clip: {e}")))?;
        Ok(Clip {
            id: id.to_owned(),
            mp4,
        })
    }

    fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
        self.shutdown();
        Ok(())
    }
}
