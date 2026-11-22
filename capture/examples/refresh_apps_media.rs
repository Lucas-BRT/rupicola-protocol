//! Regenerates all visual media (clean & annotated PNG, MP4, and inline GIF)
//! for `examples/apps/{crud-tasks,pomodoro-timer,unit-calculator,gtk4-notes}`
//! using `rupicola-capture` directly.

use rupicola_capture::{CaptureBackend, CaptureRequest, LinuxX11Backend, WebBackend};
use rupicola_protocol::{AnnotationBox, AnnotationShape, AnnotationStyle, Rect, UiAction};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn serve_dir(dir: PathBuf) -> (String, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local server");
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0_u8; 1024];
                let _ = stream.read(&mut buf);
                if let Ok(body) = fs::read(dir.join("index.html")) {
                    let hdr = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(hdr.as_bytes());
                    let _ = stream.write_all(&body);
                }
            } else {
                thread::sleep(Duration::from_millis(10));
            }
        }
    });
    (format!("http://{addr}/index.html"), stop)
}

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
    let app_dir = root.join("examples/apps").join(name);
    let (url, stop) = serve_dir(app_dir.clone());
    web.capture(&CaptureRequest {
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

fn capture_gtk4_app(root: &Path) {
    let target_dir = std::env::temp_dir().join("rupicola-refresh-gtk4");
    let build = Command::new("cargo")
        .args(["build", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("examples/apps/gtk4-notes/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .expect("build gtk4-notes");
    assert!(build.success(), "gtk4-notes build failed");

    let x11 = LinuxX11Backend { display_num: 93 };
    x11.capture(&CaptureRequest {
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
        output_dir: root.join("examples/apps/gtk4-notes/media"),
    })
    .expect("capture gtk4-notes");
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    if let Some(web) = WebBackend::discover("chromium") {
        capture_crud_tasks(&web, &root);
        capture_pomodoro_timer(&web, &root);
        capture_unit_calculator(&web, &root);
    }
    capture_gtk4_app(&root);
    println!("Regenerated clean + annotated PNG, MP4, and GIF for all 4 example apps.");
}
