//! The Chromium CDP backend as a session (without Playwright).

#![cfg(unix)]

use rupicola_capture::{
    asynchronous::{CdpBackend, CdpDiscovery as _, run_plan},
    session::{Environment, NetworkPolicy, Plan, Step, Target, UiTreeMode, WaitCondition},
};
use rupicola_protocol::UiAction;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;

const VIEWPORT_HEIGHT: u32 = 600;
const PNG_IHDR_HEIGHT_OFFSET: usize = 20;

const HTML: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>CDP Test</title></head>
<body style="margin:0;height:1800px;">
  <button id="toggle" onclick="document.getElementById('status').textContent='clicked'">Toggle</button>
  <input id="name" type="text" />
  <div id="status">idle</div>
</body>
</html>"#;

fn png_height(path: &std::path::Path) -> u32 {
    let bytes = std::fs::read(path).unwrap();
    let slice = &bytes[PNG_IHDR_HEIGHT_OFFSET..PNG_IHDR_HEIGHT_OFFSET + 4];
    u32::from_be_bytes(slice.try_into().unwrap())
}

fn serve_html() -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                HTML.len(),
                HTML
            );
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/"), handle)
}

#[tokio::test]
async fn cdp_supports_actions_wait_and_full_page_without_playwright() {
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };

    let (url, _srv) = serve_html();
    let out = std::env::temp_dir().join("rupicola-cdp-test");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let mut env = Environment::new([800, VIEWPORT_HEIGHT]);
    env.warmup_ms = 0;
    env.ui_tree = UiTreeMode::Full;
    env.network_policy = NetworkPolicy::Unrestricted;

    let plan = Plan {
        target: Target {
            command: url,
            args: Vec::new(),
        },
        environment: env,
        steps: vec![
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=#toggle".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#toggle".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::TypeText {
                    target_id: "css=#name".to_owned(),
                    text: "alice".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::ScrollToEnd { target_id: None },
            },
            Step::Checkpoint {
                id: "full".to_owned(),
                full_page: true,
            },
        ],
        allow_degraded: false,
    };

    let bundle = run_plan(&backend, &plan, &out).await.unwrap();
    assert_eq!(bundle.checkpoints.len(), 1);
    let cp = &bundle.checkpoints[0];
    assert!(
        png_height(&cp.png) > VIEWPORT_HEIGHT,
        "expected full_page screenshot height > {VIEWPORT_HEIGHT}, got {}",
        png_height(&cp.png)
    );
    let snap = cp.snapshot.as_ref().unwrap();
    let status = snap.find_by_id("status").unwrap();
    assert_eq!(status.label.as_deref(), Some("clicked"));
    let input = snap.find_by_id("name").unwrap();
    assert_eq!(input.value.as_deref(), Some("alice"));
    let _ = std::fs::remove_dir_all(&out);
}

#[tokio::test]
async fn cdp_records_video_clip_without_playwright() {
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };

    let (url, _srv) = serve_html();
    let out = std::env::temp_dir().join("rupicola-cdp-video-test");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let mut env = Environment::new([800, VIEWPORT_HEIGHT]);
    env.warmup_ms = 0;
    env.network_policy = NetworkPolicy::Unrestricted;

    let plan = Plan {
        target: Target {
            command: url,
            args: Vec::new(),
        },
        environment: env,
        steps: vec![
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=#toggle".to_owned(),
                },
            },
            Step::StartRecording,
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#toggle".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::WaitMs { ms: 160 },
            },
            Step::StopRecording {
                id: "toggle-transition".to_owned(),
            },
        ],
        allow_degraded: false,
    };

    let bundle = run_plan(&backend, &plan, &out).await.unwrap();
    assert_eq!(bundle.clips.len(), 1);
    let clip = &bundle.clips[0];
    assert_eq!(clip.id, "toggle-transition");
    assert!(
        std::fs::metadata(&clip.mp4).unwrap().len() > 0,
        "expected non-empty MP4 clip at {}",
        clip.mp4.display()
    );
    let _ = std::fs::remove_dir_all(&out);
}
