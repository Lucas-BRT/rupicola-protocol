use rupicola_protocol::AnnotationBox;
use std::path::Path;
use std::process::{Command, Stdio};

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
            let thickness = b.style.stroke_width.clamp(1.0, 20.0).round() as u32;
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

/// Renders `boxes` onto `input_png` and writes the annotated image to `output_png`
/// using `ffmpeg`'s `drawbox` filter.
///
/// # Errors
///
/// Returns an error if `boxes` is empty, `ffmpeg` is missing, or encoding fails.
pub fn annotate_png(
    input_png: &Path,
    output_png: &Path,
    boxes: &[AnnotationBox],
    scale_factor: f64,
) -> Result<(), String> {
    let filters = build_drawbox_filters(boxes, scale_factor, false);
    if filters.is_empty() {
        return Err("annotate_png requires at least one valid AnnotationBox".to_owned());
    }
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_png)
        .arg("-vf")
        .arg(filters.join(","))
        .arg(output_png)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("failed to run ffmpeg: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg drawbox exited with {status}"));
    }
    Ok(())
}

/// Renders `boxes` (with optional temporal `TimeRange` gates) onto `input_mp4`
/// and writes the annotated video to `output_mp4`.
///
/// # Errors
///
/// Returns an error if `boxes` is empty, `ffmpeg` is missing, or encoding fails.
pub fn annotate_video(
    input_mp4: &Path,
    output_mp4: &Path,
    boxes: &[AnnotationBox],
    scale_factor: f64,
) -> Result<(), String> {
    let filters = build_drawbox_filters(boxes, scale_factor, true);
    if filters.is_empty() {
        return Err("annotate_video requires at least one valid AnnotationBox".to_owned());
    }
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_mp4)
        .arg("-vf")
        .arg(filters.join(","))
        .args([
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
        ])
        .arg(output_mp4)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("failed to run ffmpeg: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg video drawbox exited with {status}"));
    }
    Ok(())
}

/// Converts an MP4 walkthrough into an inline-friendly animated GIF for docs/READMEs.
///
/// # Errors
///
/// Returns an error if `ffmpeg` is missing or GIF conversion fails.
pub fn mp4_to_gif(input_mp4: &Path, output_gif: &Path, max_width: u32) -> Result<(), String> {
    let vf = format!(
        "fps=10,scale='min({max_width},iw)':-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse"
    );
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(input_mp4)
        .arg("-vf")
        .arg(vf)
        .arg(output_gif)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("failed to run ffmpeg for gif: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg gif conversion exited with {status}"));
    }
    Ok(())
}
