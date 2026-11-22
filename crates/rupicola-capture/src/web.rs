mod driver;

use crate::session::{
    self, Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, ErrorClass,
    Target, WaitCondition,
};
use driver::{DriverReply, DriverRequest, Frame};
use rupicola_protocol::{UiAction, read_ndjson, write_ndjson};
use std::fmt::Write as _;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Folder, inside the output directory, where a recording's frames wait for encoding.
const FRAMES_DIR: &str = ".recording-frames";
/// Display time given to the last frame of a clip, in seconds.
const LAST_FRAME_SECONDS: f64 = 0.1;
/// Milliseconds per second.
const MS_PER_SECOND: f64 = 1000.0;

/// Headless Web capture backend powered by Playwright (`chromium`, `firefox`, or `webkit`).
#[derive(Clone, Debug)]
pub struct WebBackend {
    pub browser_engine: String,
    pub node_bin: PathBuf,
    pub playwright_module: PathBuf,
}

impl WebBackend {
    /// Auto-discovers `node` and a local `playwright` module installation.
    #[must_use]
    pub fn discover(browser_engine: &str) -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let node_candidates = [
            PathBuf::from("node"),
            home.join(".local/share/mise/installs/node/lts/bin/node"),
        ];
        let node_bin = node_candidates.into_iter().find(|p| {
            (p.as_os_str() == "node" && Command::new(p).arg("--version").output().is_ok())
                || p.is_file()
        })?;
        let pw_candidates = [
            std::env::var_os("RUPICOLA_PLAYWRIGHT_DIR")
                .map(PathBuf::from)
                .unwrap_or_default(),
            home.join("Projects/pokemon-survivors/node_modules/playwright"),
        ];
        let playwright_module = pw_candidates
            .into_iter()
            .find(|p| !p.as_os_str().is_empty() && p.exists())?;
        Some(Self {
            browser_engine: browser_engine.to_owned(),
            node_bin,
            playwright_module,
        })
    }
}

const SESSION_DRIVER: &str = include_str!("playwright_session_driver.js");

impl session::CaptureBackend for WebBackend {
    fn id(&self) -> &'static str {
        "web-playwright"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[
            Capability::Actions,
            Capability::UiTree,
            Capability::Video,
            Capability::ColorScheme,
            Capability::Scale,
        ])
    }

    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        let mut child = Command::new(&self.node_bin)
            .arg("-e")
            .arg(SESSION_DRIVER)
            .env("PW_DIR", &self.playwright_module)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run node: {e}")))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(CaptureError::environment("the driver has no pipes"));
        };
        let mut session = WebSession {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            out: out.to_path_buf(),
        };
        session.call(&DriverRequest::Launch {
            engine: &self.browser_engine,
            url: &target.command,
            viewport: environment.viewport,
            scale: environment.scale,
            color_scheme: environment.color_scheme,
            reduced_motion: environment.reduced_motion,
            warmup_ms: environment.warmup_ms,
        })?;
        Ok(Box::new(session))
    }
}

/// A browser page driven through a long-lived Playwright process.
struct WebSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    out: PathBuf,
}

impl WebSession {
    /// Sends `request` and waits for the driver's answer.
    fn call(&mut self, request: &DriverRequest) -> Result<DriverReply, CaptureError> {
        write_ndjson(&mut self.stdin, request)
            .map_err(|e| CaptureError::environment(format!("write to the driver: {e}")))?;
        let reply: DriverReply = read_ndjson(&mut self.stdout)
            .map_err(|e| CaptureError::environment(format!("read from the driver: {e}")))?
            .ok_or_else(|| CaptureError::environment("the driver exited unexpectedly"))?;
        if reply.ok {
            Ok(reply)
        } else {
            Err(CaptureError {
                class: reply.class.unwrap_or(ErrorClass::Target),
                message: reply
                    .error
                    .unwrap_or_else(|| "the driver failed".to_owned()),
            })
        }
    }

    /// Encodes `frames` into `mp4`, holding each until the next was taken.
    fn encode(frames: &[Frame], mp4: &Path, list: &Path) -> Result<(), CaptureError> {
        let mut script = String::new();
        for (index, frame) in frames.iter().enumerate() {
            let seconds = frames.get(index + 1).map_or(LAST_FRAME_SECONDS, |next| {
                (next.ms - frame.ms) / MS_PER_SECOND
            });
            let _ = write!(
                script,
                "file '{}'\nduration {seconds:.3}\n",
                frame.file.display()
            );
        }
        // The concat demuxer ignores the duration of the last entry unless it is repeated.
        if let Some(last) = frames.last() {
            let _ = writeln!(script, "file '{}'", last.file.display());
        }
        fs::write(list, script)
            .map_err(|e| CaptureError::environment(format!("write the frame list: {e}")))?;
        let status = Command::new("ffmpeg")
            .args(["-y", "-f", "concat", "-safe", "0", "-i"])
            .arg(list)
            .args([
                "-vf",
                "scale=trunc(iw/2)*2:trunc(ih/2)*2",
                "-fps_mode",
                "vfr",
            ])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(mp4)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| CaptureError::environment(format!("run ffmpeg: {e}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(CaptureError::environment(format!(
                "ffmpeg exited with {status}"
            )))
        }
    }

    fn stop_driver(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for WebSession {
    fn drop(&mut self) {
        self.stop_driver();
    }
}

impl session::CaptureSession for WebSession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        self.call(&DriverRequest::Perform { action }).map(|_| ())
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        self.call(&DriverRequest::Wait { condition }).map(|_| ())
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.out.join(format!("{}.png", session::checked_id(id)?));
        let reply = self.call(&DriverRequest::Screenshot { path: &png })?;
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: reply.snapshot,
        })
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        let dir = self.out.join(FRAMES_DIR);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", dir.display())))?;
        self.call(&DriverRequest::StartRecording { dir: &dir })?;
        Ok(())
    }

    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        let mp4 = self.out.join(format!("{}.mp4", session::checked_id(id)?));
        let reply = self.call(&DriverRequest::StopRecording)?;
        let dir = self.out.join(FRAMES_DIR);
        let encoded = if reply.frames.is_empty() {
            Err(CaptureError::environment("the recording has no frames"))
        } else {
            Self::encode(&reply.frames, &mp4, &dir.join("frames.txt"))
        };
        let _ = fs::remove_dir_all(&dir);
        encoded?;
        Ok(Clip {
            id: id.to_owned(),
            mp4,
        })
    }

    fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
        let closed = self.call(&DriverRequest::Close).map(|_| ());
        self.stop_driver();
        let _ = fs::remove_dir_all(self.out.join(FRAMES_DIR));
        closed
    }
}
