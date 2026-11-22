//! Regenerates all visual media (clean & annotated PNG, MP4, and inline GIF)
//! for `examples/apps/{crud-tasks,pomodoro-timer,unit-calculator,gtk4-notes}`
//! using `rupicola-capture` directly.

use crate::support::serve_dir;
#[cfg(unix)]
use rupicola_capture::LinuxX11Backend;
use rupicola_capture::{CaptureBackend, CaptureRequest, WebBackend};
use rupicola_protocol::{AnnotationBox, AnnotationShape, AnnotationStyle, Rect, UiAction};
use std::path::Path;
#[cfg(unix)]
use std::process::Command;
use std::sync::atomic::Ordering;

fn ann(x: f64, y: f64, width: f64, height: f64, label: &str) -> AnnotationBox {
    AnnotationBox {
        bounds: Rect {
            x,
            y,
            width,
            height,
        },
        label: label.to_owned(),
        shape: AnnotationShape::Rect,
        style: AnnotationStyle::default(),
        time_range: None,
        keyframes: vec![],
    }
}

fn remove_intermediate_videos(output: rupicola_capture::CaptureOutput) {
    assert!(
        output.video_gif.is_some(),
        "GIF conversion failed; retaining video"
    );
    if output.annotated_video_mp4.is_some() {
        assert!(
            output.annotated_video_gif.is_some(),
            "annotated GIF conversion failed; retaining video"
        );
    }
    for path in [output.video_mp4, output.annotated_video_mp4]
        .into_iter()
        .flatten()
    {
        std::fs::remove_file(path).expect("remove intermediate video");
    }
}

fn capture_web_app(
    web: &WebBackend,
    root: &Path,
    name: &str,
    actions: Vec<UiAction>,
    record_video_ms: u64,
    annotations: Vec<AnnotationBox>,
) {
    let app = match name {
        "crud-tasks" => "tasks",
        "pomodoro-timer" => "pomodoro",
        "unit-calculator" => "unit-calculator",
        _ => panic!("unknown web app: {name}"),
    };
    let app_dir = root.join("examples/apps/medium").join(app).join("html");
    let (url, stop) = serve_dir(app_dir.clone());
    let output = web
        .capture(&CaptureRequest {
            command: url,
            args: vec![],
            viewport: [1280, 720],
            warmup_ms: 0,
            actions,
            record_video_ms: Some(record_video_ms),
            annotations,
            output_dir: app_dir.join("media"),
        })
        .unwrap_or_else(|e| panic!("capture {name}: {e}"));
    remove_intermediate_videos(output);
    stop.store(true, Ordering::Relaxed);
}

fn capture_crud_tasks(web: &WebBackend, root: &Path) {
    capture_web_app(
        web,
        root,
        "crud-tasks",
        vec![
            UiAction::TypeText {
                target_id: "new-task".into(),
                text: "Verify video annotation".into(),
            },
            UiAction::Click {
                target_id: "add-btn".into(),
            },
            UiAction::WaitMs { ms: 350 },
            UiAction::Click {
                target_id: "[data-filter=\"active\"]".into(),
            },
            UiAction::WaitMs { ms: 400 },
        ],
        1500,
        vec![
            ann(345.0, 258.0, 590.0, 38.0, "create-form"),
            ann(345.0, 312.0, 590.0, 36.0, "filter-tabs"),
            ann(345.0, 365.0, 590.0, 146.0, "task-list"),
        ],
    );
}

fn capture_pomodoro_timer(web: &WebBackend, root: &Path) {
    capture_web_app(
        web,
        root,
        "pomodoro-timer",
        vec![
            UiAction::Click {
                target_id: "toggle-btn".into(),
            },
            UiAction::WaitMs { ms: 1100 },
            UiAction::Click {
                target_id: "[data-secs=\"300\"]".into(),
            },
            UiAction::WaitMs { ms: 350 },
            UiAction::Click {
                target_id: "theme-btn".into(),
            },
            UiAction::WaitMs { ms: 400 },
        ],
        2000,
        vec![
            ann(502.0, 224.0, 276.0, 36.0, "mode-switcher"),
            ann(540.0, 278.0, 200.0, 200.0, "svg-progress-ring"),
            ann(564.0, 496.0, 152.0, 38.0, "timer-controls"),
        ],
    );
}

fn capture_unit_calculator(web: &WebBackend, root: &Path) {
    capture_web_app(
        web,
        root,
        "unit-calculator",
        vec![
            UiAction::Click {
                target_id: "[data-k=\"+\"]".into(),
            },
            UiAction::WaitMs { ms: 200 },
            UiAction::Click {
                target_id: "[data-k=\"1\"]".into(),
            },
            UiAction::WaitMs { ms: 200 },
            UiAction::Click {
                target_id: "[data-k=\"6\"]".into(),
            },
            UiAction::WaitMs { ms: 200 },
            UiAction::Click {
                target_id: "[data-k=\"=\"]".into(),
            },
            UiAction::WaitMs { ms: 300 },
            UiAction::Click {
                target_id: "unit-btn".into(),
            },
            UiAction::WaitMs { ms: 400 },
        ],
        1800,
        vec![
            ann(452.0, 166.0, 376.0, 78.0, "numeric-display"),
            ann(452.0, 258.0, 376.0, 36.0, "unit-converter"),
            ann(452.0, 308.0, 376.0, 248.0, "keypad-grid"),
        ],
    );
}

#[cfg(unix)]
fn capture_gtk4_app(root: &Path) {
    let target_dir = std::env::temp_dir().join("rupicola-refresh-gtk4");
    let build = Command::new("cargo")
        .args(["build", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("examples/apps/medium/notes/gtk4/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .expect("build gtk4-notes");
    assert!(build.success(), "gtk4-notes build failed");

    let x11 = LinuxX11Backend { display_num: 93 };
    let output = x11
        .capture(&CaptureRequest {
            command: target_dir
                .join("debug/gtk4-notes")
                .to_string_lossy()
                .into_owned(),
            args: vec![],
            viewport: [640, 480],
            warmup_ms: 1200,
            actions: vec![],
            record_video_ms: Some(1500),
            annotations: vec![
                ann(24.0, 66.0, 492.0, 38.0, "note-entry"),
                ann(522.0, 66.0, 94.0, 38.0, "add-btn"),
                ann(24.0, 114.0, 592.0, 294.0, "notes-list"),
                ann(24.0, 418.0, 84.0, 38.0, "clear-btn"),
            ],
            output_dir: root.join("examples/apps/medium/notes/gtk4/media"),
        })
        .expect("capture gtk4-notes");
    remove_intermediate_videos(output);
}

pub fn run() {
    let root = crate::root();
    let web = WebBackend::discover("chromium").expect("Playwright with Chromium is required");
    capture_crud_tasks(&web, &root);
    capture_pomodoro_timer(&web, &root);
    capture_unit_calculator(&web, &root);
    #[cfg(unix)]
    capture_gtk4_app(&root);
    println!("Regenerated clean + annotated PNG and GIF for the supported medium examples.");
}
