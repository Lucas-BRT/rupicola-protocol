//! A virtual X display shared by the X11 and Wayland session backends: the commands that
//! grab frames with `ffmpeg` and inject pointer input with `XTest`, and the logic that
//! turns a UI action into pointer inputs. Running them is up to [`crate::sync`] or
//! [`crate::asynchronous`].

use crate::session::{CaptureError, WaitCondition};
use rupicola_protocol::UiAction;
use std::process::{Command, Stdio};

/// Time Xvfb gets to open its display before anything connects.
pub(crate) const XVFB_SETTLE_MS: u64 = 500;
/// Colour depth of the virtual screen, in bits.
const SCREEN_DEPTH: u32 = 24;
/// Frames per second of recorded clips.
const RECORDING_FRAMERATE: &str = "15";
/// Name of a clip while it is being recorded, before it has an id.
pub(crate) const RECORDING_FILE: &str = ".recording.mp4";
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
const NO_UI_TREE_ACTION: &str =
    "this backend has no UI tree: use click-at, scroll without a target or wait-ms";
const NO_UI_TREE_WAIT: &str = "this backend has no UI tree to wait on";
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

/// A coordinate as a pixel the X server accepts.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and clamped to the X11 coordinate range first"
)]
fn pixel(value: f64) -> i32 {
    value.round().clamp(0.0, MAX_COORDINATE) as i32
}

/// One thing a session does to the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointerMove {
    /// Moves to `at`, then presses `button` `repeats` times (0 = only move).
    Input {
        at: [i32; 2],
        button: u8,
        repeats: u32,
    },
    /// Waits.
    Pause { ms: u64 },
}

/// A virtual screen: how to start it, grab it and move its pointer.
#[derive(Clone, Debug)]
pub(crate) struct Screen {
    display: String,
    size: String,
    center: [i32; 2],
    depth: String,
}

impl Screen {
    /// A screen of `viewport` pixels on display `:display_num`.
    pub(crate) fn new(display_num: u16, viewport: [u32; 2]) -> Self {
        let [width, height] = viewport;
        Self {
            display: format!(":{display_num}"),
            size: format!("{width}x{height}"),
            center: [
                i32::try_from(width / 2).unwrap_or(0),
                i32::try_from(height / 2).unwrap_or(0),
            ],
            depth: format!("{width}x{height}x{SCREEN_DEPTH}"),
        }
    }

    /// The display name, such as `:93`.
    pub(crate) fn display(&self) -> &str {
        &self.display
    }

    /// Where the pointer starts.
    pub(crate) const fn center(&self) -> [i32; 2] {
        self.center
    }

    /// The command that starts the X server.
    pub(crate) fn server(&self) -> Command {
        let mut command = Command::new("Xvfb");
        command
            .args([&self.display, "-screen", "0", &self.depth])
            .args(["-nolisten", "tcp"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// `ffmpeg` arguments that read the display, up to the output file.
    ///
    /// `interactive` leaves stdin open so a recording can be told to quit with `q`.
    pub(crate) fn grab(&self, interactive: bool) -> Command {
        let mut command = Command::new("ffmpeg");
        if !interactive {
            command.arg("-nostdin");
        }
        command.args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "x11grab",
            "-video_size",
            &self.size,
        ]);
        if interactive {
            command.args(["-framerate", RECORDING_FRAMERATE]);
        }
        command.args(["-draw_mouse", "0", "-i", &self.display]);
        command
    }

    /// The command that moves the pointer to `at` and presses `button` `repeats` times.
    pub(crate) fn pointer(&self, at: [i32; 2], button: u8, repeats: u32) -> Command {
        let mut command = Command::new("python3");
        command
            .args(["-c", POINTER_SCRIPT])
            .args([at[0].to_string(), at[1].to_string()])
            .args([button.to_string(), repeats.to_string()])
            .env("DISPLAY", &self.display)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// The pointer moves that carry out `action`, with the pointer now at `pointer`.
    ///
    /// # Errors
    ///
    /// For an action that needs a UI tree.
    pub(crate) fn moves(
        action: &UiAction,
        pointer: [i32; 2],
    ) -> Result<Vec<PointerMove>, CaptureError> {
        match action {
            UiAction::ClickAt { x, y } => Ok(vec![PointerMove::Input {
                at: [pixel(*x), pixel(*y)],
                button: BUTTON_LEFT,
                repeats: 1,
            }]),
            UiAction::Scroll {
                target_id: None,
                dx,
                dy,
            } => Ok([
                (*dy, BUTTON_WHEEL_UP, BUTTON_WHEEL_DOWN),
                (*dx, BUTTON_WHEEL_LEFT, BUTTON_WHEEL_RIGHT),
            ]
            .into_iter()
            .filter_map(|(delta, negative, positive)| {
                let notches = pixel((delta.abs() / SCROLL_NOTCH_PX).ceil());
                (notches > 0).then(|| PointerMove::Input {
                    at: pointer,
                    button: if delta < 0.0 { negative } else { positive },
                    repeats: u32::try_from(notches).unwrap_or(0),
                })
            })
            .collect()),
            UiAction::WaitMs { ms } => Ok(vec![PointerMove::Pause { ms: *ms }]),
            _ => Err(CaptureError::environment(NO_UI_TREE_ACTION)),
        }
    }

    /// How long a wait lasts.
    ///
    /// # Errors
    ///
    /// For a condition that needs a UI tree.
    pub(crate) fn wait_ms(condition: &WaitCondition) -> Result<u64, CaptureError> {
        match condition {
            WaitCondition::Delay { ms } => Ok(*ms),
            WaitCondition::Visible { .. } | WaitCondition::Text { .. } => {
                Err(CaptureError::environment(NO_UI_TREE_WAIT))
            }
        }
    }

    /// The message for a pointer command that failed.
    pub(crate) fn pointer_failed(status: std::process::ExitStatus) -> CaptureError {
        CaptureError::environment(format!(
            "pointer input failed ({status}); it needs python3, libX11 and libXtst"
        ))
    }
}
