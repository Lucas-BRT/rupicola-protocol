//! Capture a built native workspace using Xvfb and `FFmpeg`.
use rupicola_capture::{CaptureBackend, CaptureRequest, LinuxX11Backend};
pub fn capture(framework: &str, command: &str) -> Result<(), String> {
    let output = (LinuxX11Backend { display_num: 98 }).capture(&CaptureRequest {
        command: command.into(),
        args: vec![],
        viewport: [1024, 768],
        warmup_ms: 2500,
        actions: vec![],
        record_video_ms: Some(1500),
        annotations: vec![],
        output_dir: crate::root()
            .join("examples/apps/advanced/project-workspace")
            .join(framework)
            .join("media"),
    })?;
    if output.video_gif.is_none() {
        return Err("GIF conversion failed; MP4 retained".into());
    }
    if let Some(path) = output.video_mp4 {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
