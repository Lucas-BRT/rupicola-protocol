use super::IntoTokio as _;
use crate::backend::annotate::{
    Ffmpeg, GIF_FAILED, PNG_FAILED, RUN_FAILED, RUN_GIF_FAILED, VIDEO_FAILED,
};
use rupicola_protocol::AnnotationBox;
use std::path::Path;

/// Renders `boxes` onto `input_png` and writes the annotated image to `output_png`
/// using `ffmpeg`'s `drawbox` filter.
///
/// # Errors
///
/// Returns an error if `boxes` is empty, `ffmpeg` is missing, or encoding fails.
pub async fn annotate_png(
    input_png: &Path,
    output_png: &Path,
    boxes: &[AnnotationBox],
    scale_factor: f64,
) -> Result<(), String> {
    let command = Ffmpeg::annotate_png(input_png, output_png, boxes, scale_factor)?;
    Ffmpeg::outcome(command.into_tokio().status().await, RUN_FAILED, PNG_FAILED)
}

/// Renders `boxes` (with optional temporal `TimeRange` gates) onto `input_mp4`
/// and writes the annotated video to `output_mp4`.
///
/// # Errors
///
/// Returns an error if `boxes` is empty, `ffmpeg` is missing, or encoding fails.
pub async fn annotate_video(
    input_mp4: &Path,
    output_mp4: &Path,
    boxes: &[AnnotationBox],
    scale_factor: f64,
) -> Result<(), String> {
    let command = Ffmpeg::annotate_video(input_mp4, output_mp4, boxes, scale_factor)?;
    Ffmpeg::outcome(
        command.into_tokio().status().await,
        RUN_FAILED,
        VIDEO_FAILED,
    )
}

/// Converts an MP4 walkthrough into an inline-friendly animated GIF for docs/READMEs.
///
/// # Errors
///
/// Returns an error if `ffmpeg` is missing or GIF conversion fails.
pub async fn mp4_to_gif(input_mp4: &Path, output_gif: &Path, max_width: u32) -> Result<(), String> {
    let command = Ffmpeg::gif(input_mp4, output_gif, max_width);
    Ffmpeg::outcome(
        command.into_tokio().status().await,
        RUN_GIF_FAILED,
        GIF_FAILED,
    )
}
