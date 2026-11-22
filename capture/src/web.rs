use crate::annotate::finalize_media;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use crate::session::{
    self, Capabilities, Capability, CaptureError, Checkpoint, Clip, ColorScheme, Environment,
    ErrorClass, Target, WaitCondition,
};
use rupicola_protocol::{UiAction, UiSnapshot, read_ndjson, write_ndjson};
use serde::{Deserialize, Serialize};
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

const PLAYWRIGHT_DRIVER: &str = include_str!("playwright_driver.js");
const SESSION_DRIVER: &str = include_str!("playwright_session_driver.js");

impl CaptureBackend for WebBackend {
    fn name(&self) -> &'static str {
        "web-playwright"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;

        let screenshot_png = request.output_dir.join("preview.png");
        let record_video = request.record_video_ms.is_some_and(|m| m > 0);
        let tmp_video_dir = request.output_dir.join(".tmp-video");
        if record_video {
            let _ = fs::remove_dir_all(&tmp_video_dir);
            fs::create_dir_all(&tmp_video_dir)
                .map_err(|e| format!("failed to create temp video dir: {e}"))?;
        }

        let actions_json = serde_json::to_string(&request.actions)
            .map_err(|e| format!("failed to serialize actions: {e}"))?;
        let out = Command::new(&self.node_bin)
            .arg("-e")
            .arg(PLAYWRIGHT_DRIVER)
            .env("PW_DIR", &self.playwright_module)
            .env("PW_ENGINE", &self.browser_engine)
            .env("TARGET_URL", &request.command)
            .env("VP_W", request.viewport[0].to_string())
            .env("VP_H", request.viewport[1].to_string())
            .env("OUT_PNG", &screenshot_png)
            .env("ACTIONS_JSON", actions_json)
            .env(
                "VIDEO_DIR",
                if record_video {
                    tmp_video_dir.to_string_lossy().into_owned()
                } else {
                    String::new()
                },
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("failed to execute playwright driver: {e}"))?;

        if !out.status.success() {
            return Err(format!(
                "playwright driver failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }

        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| format!("invalid driver JSON output: {e}"))?;
        let snapshot: Option<UiSnapshot> = parsed
            .get("snapshot")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());

        let video_mp4 = if let Some(webm) = parsed.get("video_path").and_then(|v| v.as_str()) {
            let mp4_path = request.output_dir.join("walkthrough.mp4");
            let conv = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i",
                    webm,
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
            let _ = fs::remove_dir_all(&tmp_video_dir);
            match conv {
                Ok(s) if s.success() => Some(mp4_path),
                _ => None,
            }
        } else {
            None
        };

        finalize_media(
            &request.output_dir,
            screenshot_png,
            video_mp4,
            &request.annotations,
            snapshot,
        )
    }
}

/// A command for `playwright_session_driver.js`, one JSON line each.
#[derive(Serialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
enum DriverRequest<'a> {
    Launch {
        engine: &'a str,
        url: &'a str,
        viewport: [u32; 2],
        scale: f64,
        color_scheme: Option<ColorScheme>,
        reduced_motion: bool,
        warmup_ms: u64,
    },
    Perform {
        action: &'a UiAction,
    },
    Wait {
        condition: &'a WaitCondition,
    },
    Screenshot {
        path: &'a Path,
    },
    StartRecording {
        dir: &'a Path,
    },
    StopRecording,
    Close,
}

/// One recorded frame: its file and when it was taken (Unix milliseconds).
#[derive(Deserialize)]
struct Frame {
    file: PathBuf,
    ms: f64,
}

/// The driver's answer to a command.
#[derive(Deserialize)]
struct DriverReply {
    ok: bool,
    #[serde(default)]
    class: Option<ErrorClass>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    snapshot: Option<UiSnapshot>,
    #[serde(default)]
    frames: Vec<Frame>,
}

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
