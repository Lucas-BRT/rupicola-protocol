use rupicola_capture::{CaptureBackend, CaptureRequest, LinuxX11Backend, WebBackend};
use rupicola_protocol::{
    AnnotationBox, PROTOCOL_VERSION, ProtocolRequest, ProtocolResponse, Rect, TimeRange, UiAction,
    UnixProtocolClient, serve_once_unix,
};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[test]
fn temporal_annotation_box_filters_by_time_and_normalizes_percent() {
    let ann = AnnotationBox {
        bounds: Rect {
            x: 64.0,
            y: 48.0,
            width: 320.0,
            height: 96.0,
        },
        label: "toast".into(),
        color: None,
        time_range: Some(TimeRange {
            start_ms: 500,
            end_ms: 1500,
        }),
    };
    assert!(!ann.active_at(499));
    assert!(ann.active_at(500));
    assert!(ann.active_at(1500));
    assert!(!ann.active_at(1501));

    let pct = ann.to_percent_box([640, 480]).unwrap();
    assert!((pct[0] - 10.0).abs() < 0.01);
    assert!((pct[1] - 10.0).abs() < 0.01);
    assert!((pct[2] - 50.0).abs() < 0.01);
    assert!((pct[3] - 20.0).abs() < 0.01);
}

#[cfg(unix)]
#[test]
fn unix_protocol_client_and_server_roundtrip() {
    let sock = std::env::temp_dir().join("rupicola-proto-ipc-test.sock");
    let sock_srv = sock.clone();
    let handle = thread::spawn(move || {
        serve_once_unix(&sock_srv, |req| match req {
            ProtocolRequest::Ping => ProtocolResponse::Pong,
            _ => ProtocolResponse::Error {
                code: "unsupported".into(),
                message: "only ping in test".into(),
            },
        })
        .unwrap();
    });

    thread::sleep(Duration::from_millis(80));
    let mut client = UnixProtocolClient::connect(&sock).unwrap();
    let resp = client.call(&ProtocolRequest::Ping).unwrap();
    assert_eq!(resp, ProtocolResponse::Pong);
    drop(client);
    handle.join().unwrap();
    let _ = fs::remove_file(&sock);
}

#[cfg(unix)]
#[test]
fn linux_x11_backend_captures_real_gtk4_app_and_annotates_in_rust() {
    let has_xvfb = Command::new("which")
        .arg("Xvfb")
        .output()
        .is_ok_and(|o| o.status.success());
    let has_ffmpeg = Command::new("which")
        .arg("ffmpeg")
        .output()
        .is_ok_and(|o| o.status.success());
    let has_gtk4 = Command::new("pkg-config")
        .args(["--exists", "gtk4"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !has_xvfb || !has_ffmpeg || !has_gtk4 {
        return;
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let target_dir = std::env::temp_dir().join("rupicola-capture-gtk4-target");
    let build = Command::new("cargo")
        .args(["build", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("examples/apps/gtk4-notes/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .unwrap();
    assert!(build.success());

    let out_dir = std::env::temp_dir().join("rupicola-capture-gtk4-out");
    let _ = fs::remove_dir_all(&out_dir);

    let backend = LinuxX11Backend { display_num: 94 };
    let out = backend
        .capture(&CaptureRequest {
            command: target_dir
                .join("debug/gtk4-notes")
                .to_string_lossy()
                .into_owned(),
            args: vec![],
            viewport: [640, 480],
            warmup_ms: 1200,
            actions: vec![],
            record_video_ms: Some(1000),
            annotations: vec![AnnotationBox {
                bounds: Rect {
                    x: 24.0,
                    y: 66.0,
                    width: 492.0,
                    height: 38.0,
                },
                label: "note-entry".into(),
                color: Some("#c24518".into()),
                time_range: None,
            }],
            output_dir: out_dir.clone(),
        })
        .unwrap();

    assert!(out.screenshot_png.is_file());
    assert!(out.annotated_png.as_ref().is_some_and(|p| p.is_file()));
    assert!(out.video_mp4.as_ref().is_some_and(|p| p.is_file()));
    assert_eq!(PROTOCOL_VERSION, 1);
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn web_backend_captures_real_crud_app_with_actions_video_and_annotations() {
    let Some(backend) = WebBackend::discover("chromium") else {
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let app_dir = root.join("examples/apps/crud-tasks");

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);
    thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0_u8; 1024];
                let _ = stream.read(&mut buf);
                if let Ok(body) = fs::read(app_dir.join("index.html")) {
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

    let out_dir = std::env::temp_dir().join("rupicola-capture-web-out");
    let _ = fs::remove_dir_all(&out_dir);
    let out = backend
        .capture(&CaptureRequest {
            command: format!("http://{addr}/index.html"),
            args: vec![],
            viewport: [1280, 720],
            warmup_ms: 0,
            actions: vec![
                UiAction::TypeText {
                    target_id: "new-task".into(),
                    text: "Captured by WebBackend".into(),
                },
                UiAction::Click {
                    target_id: "add-btn".into(),
                },
            ],
            record_video_ms: Some(1000),
            annotations: vec![AnnotationBox {
                bounds: Rect {
                    x: 345.0,
                    y: 258.0,
                    width: 590.0,
                    height: 38.0,
                },
                label: "create-form".into(),
                color: Some("#c24518".into()),
                time_range: None,
            }],
            output_dir: out_dir.clone(),
        })
        .unwrap();
    stop.store(true, Ordering::Relaxed);

    assert!(out.screenshot_png.is_file());
    assert!(out.annotated_png.as_ref().is_some_and(|p| p.is_file()));
    assert!(out.video_mp4.as_ref().is_some_and(|p| p.is_file()));
    let snap = out.snapshot.expect("live DOM UiSnapshot");
    assert_eq!(
        snap.find_by_id("counter").unwrap().label.as_deref(),
        Some("3 remaining · 4 total")
    );
    let _ = fs::remove_dir_all(&out_dir);
}
