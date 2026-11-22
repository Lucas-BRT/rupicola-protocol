//! A virtual X display shared by the X11 and Wayland session backends: grabs frames and
//! clips with `ffmpeg`, injects pointer input with `XTest`, and owns every process started
//! for the session.

use crate::session::{self, CaptureError, Checkpoint, Clip, WaitCondition};
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
/// Pixels one scroll-wheel notch stands for.
const SCROLL_NOTCH_PX: f64 = 40.0;
/// Largest pointer coordinate sent to the X server.
const MAX_COORDINATE: f64 = 65_535.0;
/// X11 pointer buttons: left click and the four wheel directions.
const BUTTON_LEFT: u8 = 1;
const BUTTON_WHEEL_UP: u8 = 4;
const BUTTON_WHEEL_DOWN: u8 = 5;
const BUTTON_WHEEL_LEFT: u8 = 6;
const BUTTON_WHEEL_RIGHT: u8 = 7;
/// Moves the pointer to `x y`, then presses and releases `button` `repeats` times (once
/// per wheel notch). Arguments: x, y, button (0 = move only), repeats.
const POINTER_SCRIPT: &str = r#"
import sys, ctypes as c
x11 = c.CDLL("libX11.so.6")
xt = c.CDLL("libXtst.so.6")
x11.XOpenDisplay.restype = c.c_void_p
display = c.c_void_p(x11.XOpenDisplay(None))
if not display:
    raise SystemExit("cannot open the display")
x, y, button, repeats = map(int, sys.argv[1:5])
xt.XTestFakeMotionEvent(display, -1, x, y, 0)
if button:
    for _ in range(repeats):
        xt.XTestFakeButtonEvent(display, button, 1, 0)
        xt.XTestFakeButtonEvent(display, button, 0, 0)
x11.XSync(display, 0)
x11.XCloseDisplay(display)
"#;

/// A running Xvfb display with the session's processes on it.
pub struct XvfbSession {
    display: String,
    size: String,
    out: PathBuf,
    /// Everything started for the session, in start order; stopped in reverse.
    processes: Vec<Child>,
    recording: Option<Child>,
    pointer: [i32; 2],
    scratch: Option<PathBuf>,
}

/// A coordinate as a pixel the X server accepts.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and clamped to the X11 coordinate range first"
)]
fn pixel(value: f64) -> i32 {
    value.round().clamp(0.0, MAX_COORDINATE) as i32
}

impl XvfbSession {
    /// Starts Xvfb as display `display_num`, sized to `viewport`; files go under `out`.
    ///
    /// # Errors
    ///
    /// An environment error when the output directory or Xvfb cannot be set up.
    pub fn open(display_num: u16, viewport: [u32; 2], out: &Path) -> Result<Self, CaptureError> {
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        let display = format!(":{display_num}");
        let [width, height] = viewport;
        let xvfb = Command::new("Xvfb")
            .args([
                &display,
                "-screen",
                "0",
                &format!("{width}x{height}x{SCREEN_DEPTH}"),
                "-nolisten",
                "tcp",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::environment(format!("spawn Xvfb: {e}")))?;
        thread::sleep(XVFB_SETTLE);
        Ok(Self {
            display,
            size: format!("{width}x{height}"),
            out: out.to_path_buf(),
            processes: vec![xvfb],
            recording: None,
            pointer: [
                i32::try_from(width / 2).unwrap_or(0),
                i32::try_from(height / 2).unwrap_or(0),
            ],
            scratch: None,
        })
    }

    /// The display name, such as `:93`.
    #[must_use]
    pub fn display(&self) -> &str {
        &self.display
    }

    /// The directory files are written to.
    #[must_use]
    pub fn out(&self) -> &Path {
        &self.out
    }

    /// Hands a process to the session, which stops it on finish or drop.
    pub fn adopt(&mut self, child: Child) {
        self.processes.push(child);
    }

    /// A private scratch directory (mode 0700), removed when the session ends.
    ///
    /// # Errors
    ///
    /// An environment error when it cannot be created.
    pub fn scratch(&mut self) -> Result<PathBuf, CaptureError> {
        if let Some(dir) = &self.scratch {
            return Ok(dir.clone());
        }
        let dir = std::env::temp_dir().join(format!(
            "rupicola-session-{}-{}",
            std::process::id(),
            self.display.trim_start_matches(':')
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", dir.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
        }
        self.scratch = Some(dir.clone());
        Ok(dir)
    }

    /// `ffmpeg` arguments that read the display, up to the output file.
    ///
    /// `interactive` leaves stdin open so a recording can be told to quit with `q`.
    fn grab(&self, interactive: bool) -> Command {
        let mut command = Command::new("ffmpeg");
        if !interactive {
            command.arg("-nostdin");
        }
        command
            .args(["-y", "-f", "x11grab", "-video_size", &self.size])
            .args(["-draw_mouse", "0", "-i", &self.display]);
        command
    }

    /// Moves the pointer to `[x, y]`, then presses `button` `repeats` times (0 = only move).
    fn pointer_input(
        &mut self,
        [x, y]: [i32; 2],
        button: u8,
        repeats: u32,
    ) -> Result<(), CaptureError> {
        let status = Command::new("python3")
            .args(["-c", POINTER_SCRIPT])
            .args([x.to_string(), y.to_string()])
            .args([button.to_string(), repeats.to_string()])
            .env("DISPLAY", &self.display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| CaptureError::environment(format!("run python3: {e}")))?;
        if !status.success() {
            return Err(CaptureError::environment(format!(
                "pointer input failed ({status}); it needs python3, libX11 and libXtst"
            )));
        }
        self.pointer = [x, y];
        Ok(())
    }

    /// Stops everything the session started; safe to call twice.
    fn shutdown(&mut self) {
        if let Some(mut recording) = self.recording.take() {
            let _ = recording.kill();
            let _ = recording.wait();
        }
        while let Some(mut child) = self.processes.pop() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(dir) = self.scratch.take() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

impl Drop for XvfbSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl session::CaptureSession for XvfbSession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        match action {
            UiAction::ClickAt { x, y } => {
                self.pointer_input([pixel(*x), pixel(*y)], BUTTON_LEFT, 1)
            }
            UiAction::Scroll {
                target_id: None,
                dx,
                dy,
            } => {
                let at = self.pointer;
                for (delta, negative, positive) in [
                    (*dy, BUTTON_WHEEL_UP, BUTTON_WHEEL_DOWN),
                    (*dx, BUTTON_WHEEL_LEFT, BUTTON_WHEEL_RIGHT),
                ] {
                    let notches = pixel((delta.abs() / SCROLL_NOTCH_PX).ceil());
                    if notches > 0 {
                        let button = if delta < 0.0 { negative } else { positive };
                        self.pointer_input(at, button, u32::try_from(notches).unwrap_or(0))?;
                    }
                }
                Ok(())
            }
            UiAction::WaitMs { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            _ => Err(CaptureError::environment(
                "this backend has no UI tree: use click-at, scroll without a target or wait-ms",
            )),
        }
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        match condition {
            WaitCondition::Delay { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            WaitCondition::Visible { .. } | WaitCondition::Text { .. } => Err(
                CaptureError::environment("this backend has no UI tree to wait on"),
            ),
        }
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.out.join(format!("{}.png", session::checked_id(id)?));
        let status = self
            .grab(false)
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
        let mut command = self.grab(true);
        command
            .args(["-framerate", RECORDING_FRAMERATE])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(self.out.join(RECORDING_FILE))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = command
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
