use rupicola_protocol::AnnotationBox;
use std::path::Path;
use std::process::{Command, Stdio};

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
    scale_factor: f32,
) -> Result<(), String> {
    if boxes.is_empty() {
        return Err("annotate_png requires at least one AnnotationBox".to_owned());
    }
    let filters: Vec<String> = boxes
        .iter()
        .filter(|b| b.bounds.is_valid())
        .map(|b| {
            let [x, y, w, h] = b.bounds.to_pixel_box(scale_factor);
            let color = b.color.as_deref().unwrap_or("#c24518");
            format!("drawbox=x={x}:y={y}:w={w}:h={h}:color={color}:t=2")
        })
        .collect();
    if filters.is_empty() {
        return Err("annotate_png found no valid bounding boxes".to_owned());
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
