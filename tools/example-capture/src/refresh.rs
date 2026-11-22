//! Regenerates all visual media (clean & annotated PNG, MP4, and inline GIF)
//! for `examples/apps/{crud-tasks,pomodoro-timer,unit-calculator,gtk4-notes}`
//! using `rupicola-capture` directly.

use crate::media::Walkthrough;
use crate::support::serve_dir;
use rupicola_capture::session::{Environment, Target};
#[cfg(unix)]
use rupicola_capture::sync::LinuxX11Backend;
use rupicola_capture::sync::{WebBackend, WebDiscovery as _};
use rupicola_protocol::{AnnotationBox, AnnotationShape, AnnotationStyle, Rect, UiAction};
use std::path::Path;
#[cfg(unix)]
use std::process::Command;
use std::sync::atomic::Ordering;

const WEB_VIEWPORT: [u32; 2] = [1280, 720];
const WEB_WARMUP_MS: u64 = 0;
#[cfg(unix)]
const GTK_VIEWPORT: [u32; 2] = [640, 480];
#[cfg(unix)]
const GTK_WARMUP_MS: u64 = 1200;
#[cfg(unix)]
const GTK_HOLD_MS: u64 = 1500;
#[cfg(unix)]
const GTK_DISPLAY: u16 = 93;

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
    let mut environment = Environment::new(WEB_VIEWPORT);
    environment.warmup_ms = WEB_WARMUP_MS;
    let result = Walkthrough {
        target: Target {
            command: url,
            args: vec![],
        },
        environment,
        actions,
        hold_ms: record_video_ms,
        annotations,
        out: app_dir.join("media"),
    }
    .record(web);
    stop.store(true, Ordering::Relaxed);
    result.unwrap_or_else(|e| panic!("capture {name}: {e}"));
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

    let mut environment = Environment::new(GTK_VIEWPORT);
    environment.warmup_ms = GTK_WARMUP_MS;
    Walkthrough {
        target: Target {
            command: target_dir
                .join("debug/gtk4-notes")
                .to_string_lossy()
                .into_owned(),
            args: vec![],
        },
        environment,
        actions: vec![],
        hold_ms: GTK_HOLD_MS,
        annotations: vec![
            ann(24.0, 66.0, 492.0, 38.0, "note-entry"),
            ann(522.0, 66.0, 94.0, 38.0, "add-btn"),
            ann(24.0, 114.0, 592.0, 294.0, "notes-list"),
            ann(24.0, 418.0, 84.0, 38.0, "clear-btn"),
        ],
        out: root.join("examples/apps/medium/notes/gtk4/media"),
    }
    .record(&LinuxX11Backend {
        display_num: GTK_DISPLAY,
    })
    .expect("capture gtk4-notes");
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
