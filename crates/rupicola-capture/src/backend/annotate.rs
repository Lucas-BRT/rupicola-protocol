//! The `ffmpeg` commands that draw annotations and make GIFs; running them is up to
//! [`crate::sync`] or [`crate::asynchronous`].

use rupicola_protocol::AnnotationBox;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

const FFMPEG: &str = "ffmpeg";
/// GIF filter: 10 fps, scaled down to `max_width`, with a generated palette.
const GIF_FILTER: &str = "fps=10,scale='min({max_width},iw)':-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse";
const MIN_STROKE: f64 = 1.0;
const MAX_STROKE: f64 = 20.0;

pub(crate) const NO_PNG_BOXES: &str = "annotate_png requires at least one valid AnnotationBox";
pub(crate) const NO_VIDEO_BOXES: &str = "annotate_video requires at least one valid AnnotationBox";
pub(crate) const PNG_FAILED: &str = "ffmpeg drawbox exited with";
pub(crate) const VIDEO_FAILED: &str = "ffmpeg video drawbox exited with";
pub(crate) const GIF_FAILED: &str = "ffmpeg gif conversion exited with";
pub(crate) const RUN_FAILED: &str = "failed to run ffmpeg";
pub(crate) const RUN_GIF_FAILED: &str = "failed to run ffmpeg for gif";

fn build_drawbox_filters(
    boxes: &[AnnotationBox],
    scale_factor: f64,
    temporal: bool,
) -> Vec<String> {
    boxes
        .iter()
        .filter(|b| b.bounds.is_valid())
        .map(|b| {
            let [x, y, w, h] = b.bounds.to_pixel_box(scale_factor);
            let color = b.style.stroke_color.as_str();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "clamped positive stroke thickness in pixels"
            )]
            let thickness = b.style.stroke_width.clamp(MIN_STROKE, MAX_STROKE).round() as u32;
            if temporal && let Some(range) = b.time_range {
                let start = format!("{}.{:03}", range.start_ms / 1000, range.start_ms % 1000);
                let end = format!("{}.{:03}", range.end_ms / 1000, range.end_ms % 1000);
                format!(
                    "drawbox=x={x}:y={y}:w={w}:h={h}:color={color}:t={thickness}:enable='between(t,{start},{end})'"
                )
            } else {
                format!("drawbox=x={x}:y={y}:w={w}:h={h}:color={color}:t={thickness}")
            }
        })
        .collect()
}

/// `ffmpeg` invocations, ready to run.
pub(crate) struct Ffmpeg;

impl Ffmpeg {
    fn command(input: &Path) -> Command {
        let mut command = Command::new(FFMPEG);
        command
            .arg("-y")
            .arg("-i")
            .arg(input)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// Draws `boxes` on a PNG.
    ///
    /// # Errors
    ///
    /// When `boxes` holds no valid box.
    pub(crate) fn annotate_png(
        input: &Path,
        output: &Path,
        boxes: &[AnnotationBox],
        scale_factor: f64,
    ) -> Result<Command, String> {
        let filters = build_drawbox_filters(boxes, scale_factor, false);
        if filters.is_empty() {
            return Err(NO_PNG_BOXES.to_owned());
        }
        let mut command = Self::command(input);
        command.arg("-vf").arg(filters.join(",")).arg(output);
        Ok(command)
    }

    /// Draws `boxes`, honouring their time ranges, on an MP4.
    ///
    /// # Errors
    ///
    /// When `boxes` holds no valid box.
    pub(crate) fn annotate_video(
        input: &Path,
        output: &Path,
        boxes: &[AnnotationBox],
        scale_factor: f64,
    ) -> Result<Command, String> {
        let filters = build_drawbox_filters(boxes, scale_factor, true);
        if filters.is_empty() {
            return Err(NO_VIDEO_BOXES.to_owned());
        }
        let mut command = Self::command(input);
        command
            .arg("-vf")
            .arg(filters.join(","))
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(output);
        Ok(command)
    }

    /// Converts an MP4 to an animated GIF at most `max_width` pixels wide.
    pub(crate) fn gif(input: &Path, output: &Path, max_width: u32) -> Command {
        let mut command = Self::command(input);
        command
            .arg("-vf")
            .arg(GIF_FILTER.replace("{max_width}", &max_width.to_string()))
            .arg(output);
        command
    }

    /// `Ok` when the command ran and succeeded; otherwise the message for what failed.
    pub(crate) fn outcome(
        status: io::Result<ExitStatus>,
        launch_failed: &str,
        exit_failed: &str,
    ) -> Result<(), String> {
        let status = status.map_err(|e| format!("{launch_failed}: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{exit_failed} {status}"))
        }
    }
}
