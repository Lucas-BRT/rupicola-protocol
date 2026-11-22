//! Capture the project workspace served at the supplied URL.
use rupicola_capture::{CaptureBackend, CaptureRequest, WebBackend};
use rupicola_protocol::UiAction;

pub fn capture(framework: &str, url: Option<&str>) -> Result<(), String> {
    let backend = WebBackend::discover("chromium").ok_or("Playwright not found")?;
    let app_dir = crate::root()
        .join("examples/apps/advanced/project-workspace")
        .join(framework);
    let (url, server) = match url {
        Some(url) => (url.to_owned(), None),
        None if framework == "html" => {
            let (url, stop) = crate::support::serve_dir(app_dir.clone());
            (url, Some(stop))
        }
        None => return Err("provide the URL of the running React app".into()),
    };
    let output = backend.capture(&CaptureRequest {
        command: url,
        args: vec![],
        viewport: [1280, 720],
        warmup_ms: 500,
        actions: vec![
            UiAction::Click {
                target_id: "[data-page=projects]".into(),
            },
            UiAction::WaitMs { ms: 500 },
            UiAction::Click {
                target_id: "#new-project".into(),
            },
            UiAction::WaitMs { ms: 800 },
        ],
        record_video_ms: Some(2000),
        annotations: vec![],
        output_dir: app_dir.join("media"),
    });
    if let Some(stop) = server {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let output = output?;
    if output.video_gif.is_none() {
        return Err("GIF conversion failed; MP4 retained".into());
    }
    if let Some(mp4) = output.video_mp4 {
        std::fs::remove_file(mp4).map_err(|e| e.to_string())?;
    }
    Ok(())
}
