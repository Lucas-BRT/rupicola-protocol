//! The web backend as a session: one browser page serves actions, checkpoints and a clip.

#![cfg(feature = "async")]

use rupicola_capture::{
    asynchronous::{WebBackend, WebDiscovery as _, run_plan},
    session::{ColorScheme, Environment, ErrorClass, Plan, Step, Target, WaitCondition},
};
use rupicola_protocol::UiAction;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

const VIEWPORT: [u32; 2] = [1280, 720];
/// Pause between accepts while the test server waits for a connection.
const ACCEPT_POLL: Duration = Duration::from_millis(10);
/// Length of the recorded walkthrough.
const CLIP_MS: u64 = 600;
/// What the CRUD example's counter says after adding one task to its three.
const COUNTER_AFTER_ADD: &str = "3 remaining · 4 total";
/// The shortest acceptable clip: most of the 600 ms walkthrough.
const MIN_CLIP_SECONDS: f64 = 0.4;
/// A port nothing listens on.
const CLOSED_PORT_URL: &str = "http://127.0.0.1:1/";

/// Serves the CRUD example until the returned flag is set.
fn serve_example() -> (String, Arc<AtomicBool>) {
    let page = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/apps/medium/tasks/html/index.html");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/index.html", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                if let Ok(body) = fs::read(&page) {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&body);
                }
            } else {
                thread::sleep(ACCEPT_POLL);
            }
        }
    });
    (url, stop)
}

/// Duration of a video in seconds, when `ffprobe` is installed.
fn clip_seconds(path: &std::path::Path) -> Option<f64> {
    let output = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration"])
        .args(["-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(path)
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

fn plan(url: &str, steps: Vec<Step>) -> Plan {
    let mut environment = Environment::new(VIEWPORT);
    environment.warmup_ms = 0;
    environment.color_scheme = Some(ColorScheme::Dark);
    Plan {
        target: Target {
            command: url.to_owned(),
            args: Vec::new(),
        },
        environment,
        steps,
        allow_degraded: false,
    }
}

#[tokio::test]
async fn one_page_serves_actions_two_checkpoints_and_a_clip() {
    let Some(backend) = WebBackend::discover("chromium").await else {
        return;
    };
    let (url, stop) = serve_example();
    let out = std::env::temp_dir().join("rupicola-capture-async-session-web");
    let _ = fs::remove_dir_all(&out);
    let steps = vec![
        Step::Checkpoint {
            id: "before".to_owned(),
        },
        Step::StartRecording,
        Step::Action {
            action: UiAction::TypeText {
                target_id: "new-task".to_owned(),
                text: "Captured by a session".to_owned(),
            },
        },
        Step::Action {
            action: UiAction::Click {
                target_id: "add-btn".to_owned(),
            },
        },
        Step::Wait {
            condition: WaitCondition::Text {
                text: "Captured by a session".to_owned(),
            },
        },
        Step::Wait {
            condition: WaitCondition::Delay { ms: CLIP_MS },
        },
        Step::Checkpoint {
            id: "after".to_owned(),
        },
        Step::StopRecording {
            id: "walk".to_owned(),
        },
    ];
    let bundle = run_plan(&backend, &plan(&url, steps), &out).await.unwrap();
    stop.store(true, Ordering::Relaxed);

    assert_eq!(bundle.checkpoints.len(), 2);
    assert!(bundle.checkpoints.iter().all(|c| c.png.is_file()));
    let after = bundle.checkpoints[1]
        .snapshot
        .as_ref()
        .expect("DOM snapshot");
    assert_eq!(
        after.find_by_id("counter").unwrap().label.as_deref(),
        Some(COUNTER_AFTER_ADD)
    );
    assert!(bundle.clips[0].mp4.is_file());
    assert!(
        clip_seconds(&bundle.clips[0].mp4).is_none_or(|seconds| seconds >= MIN_CLIP_SECONDS),
        "the clip is shorter than the walkthrough"
    );
    assert!(!out.join(".recording-frames").exists());
    let _ = fs::remove_dir_all(&out);
}

#[tokio::test]
async fn a_missing_element_is_a_target_error() {
    let Some(backend) = WebBackend::discover("chromium").await else {
        return;
    };
    let (url, stop) = serve_example();
    let out = std::env::temp_dir().join("rupicola-capture-async-session-web-missing");
    let steps = vec![Step::Wait {
        condition: WaitCondition::Visible {
            target_id: "no-such-element".to_owned(),
        },
    }];
    let error = run_plan(&backend, &plan(&url, steps), &out)
        .await
        .unwrap_err();
    stop.store(true, Ordering::Relaxed);
    assert_eq!(error.class, ErrorClass::Target);
    let _ = fs::remove_dir_all(&out);
}

#[tokio::test]
async fn an_unreachable_url_is_a_target_error() {
    let Some(backend) = WebBackend::discover("chromium").await else {
        return;
    };
    let out = std::env::temp_dir().join("rupicola-capture-async-session-web-closed");
    let error = run_plan(&backend, &plan(CLOSED_PORT_URL, Vec::new()), &out)
        .await
        .unwrap_err();
    assert_eq!(error.class, ErrorClass::Target);
    let _ = fs::remove_dir_all(&out);
}
