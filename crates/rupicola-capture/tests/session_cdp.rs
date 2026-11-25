//! The Chromium CDP backend as a session (without Playwright).

#![cfg(unix)]

use rupicola_capture::{
    asynchronous::{CdpBackend, CdpDiscovery as _, run_plan},
    session::{Environment, NetworkPolicy, Plan, Step, Target, UiTreeMode, WaitCondition},
};
use rupicola_protocol::UiAction;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::process::Command;

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

const ANIMATED_HTML: &str = r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    #box { width: 40px; height: 40px; background: red; transition: transform 800ms linear; }
    #box.moved { transform: translateX(400px); }
  </style>
</head>
<body>
  <button id="move" onclick="document.getElementById('box').classList.add('moved')">Move</button>
  <div id="box"></div>
</body>
</html>"#;

const NAVIGATION_HTML: &str = r##"<!doctype html>
<html>
<head><meta charset="utf-8"><title>CDP navigation</title></head>
<body>
  <style>
    @view-transition { navigation: auto; }
    .button { display: inline-flex; min-height: 32px; padding: 4px 11px; transition: transform 100ms, border-color 140ms; }
    .button:active { transform: translateY(1px); }
  </style>
  <a id="spa" data-spa href="/spa">Client route</a>
  <a id="normal" class="button primary" href="/next">Document route</a>
  <p id="status">waiting</p>
  <script>
    document.addEventListener("click", (event) => {
      if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      const link = event.target.closest("a[data-spa]");
      if (!link) return;
      event.preventDefault();
      document.querySelector("#status").textContent = `trusted:${event.isTrusted}:button:${event.button}`;
      history.pushState({}, "", link.href);
    });
  </script>
</body>
</html>"##;

const NAVIGATION_RESULT_HTML: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>CDP navigation result</title></head>
<body><main id="next-page">Document navigation completed</main></body>
</html>"#;

const TRANSITION_SOURCE_HTML: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>CDP transition source</title>
<style>
  @view-transition { navigation: auto; }
  ::view-transition-old(root), ::view-transition-new(root) { animation-duration: 500ms; }
</style></head>
<body><a id="dashboard-link" href="/dashboard">Dashboard</a></body>
</html>"#;

const TRANSITION_DASHBOARD_HTML: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>CDP transition dashboard</title>
<style>
  @view-transition { navigation: auto; }
  ::view-transition-old(root), ::view-transition-new(root) { animation-duration: 500ms; }
</style></head>
<body><main><a id="projects-link" href="/projects">Projects</a></main></body>
</html>"#;

const TRANSITION_PROJECTS_HTML: &str = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>CDP transition projects</title></head>
<body><main id="projects-done">Projects route completed</main></body>
</html>"#;

fn png_height(path: &std::path::Path) -> u32 {
    let bytes = std::fs::read(path).unwrap();
    let slice = &bytes[PNG_IHDR_HEIGHT_OFFSET..PNG_IHDR_HEIGHT_OFFSET + 4];
    u32::from_be_bytes(slice.try_into().unwrap())
}

fn serve_html() -> (String, std::thread::JoinHandle<()>) {
    serve_page(HTML)
}

fn serve_page(html: &'static str) -> (String, std::thread::JoinHandle<()>) {
    serve_pages(vec![("/", html)])
}

fn serve_pages(pages: Vec<(&'static str, &'static str)>) -> (String, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut s = stream;
            let mut buf = [0u8; 1024];
            let read = s.read(&mut buf).unwrap_or_default();
            let request = String::from_utf8_lossy(&buf[..read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .unwrap_or("/");
            let html = pages
                .iter()
                .find_map(|(route, body)| (*route == path).then_some(*body))
                .unwrap_or("<!doctype html><title>Not found</title>");
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                html.len(),
                html
            );
            let _ = s.write_all(resp.as_bytes());
        }
    });
    (format!("http://127.0.0.1:{port}/"), handle)
}

#[tokio::test]
async fn cdp_click_activates_delegated_and_normal_navigation_links() {
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };

    let (url, _srv) = serve_pages(vec![
        ("/", NAVIGATION_HTML),
        ("/next", NAVIGATION_RESULT_HTML),
    ]);
    let out = std::env::temp_dir().join("rupicola-cdp-navigation-test");
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
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#spa".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Text {
                    text: "trusted:true:button:0".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#normal".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=#next-page".to_owned(),
                },
            },
            Step::Checkpoint {
                id: "after-navigation".to_owned(),
                full_page: false,
            },
        ],
        allow_degraded: false,
    };

    let bundle = run_plan(&backend, &plan, &out).await.unwrap();
    assert_eq!(bundle.checkpoints.len(), 1);
    let _ = std::fs::remove_dir_all(&out);
}

#[tokio::test]
async fn cdp_settles_cross_document_view_transition_before_next_click() {
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };
    let (url, _srv) = serve_pages(vec![
        ("/", TRANSITION_SOURCE_HTML),
        ("/dashboard", TRANSITION_DASHBOARD_HTML),
        ("/projects", TRANSITION_PROJECTS_HTML),
    ]);
    let out = std::env::temp_dir().join("rupicola-cdp-view-transition-settle-test");
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
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#dashboard-link".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#projects-link".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=#projects-done".to_owned(),
                },
            },
        ],
        allow_degraded: false,
    };
    run_plan(&backend, &plan, &out).await.unwrap();
    let _ = std::fs::remove_dir_all(&out);
}

#[tokio::test]
async fn cdp_clicks_navigate_the_authenticated_local_panel_when_configured() {
    let (Ok(panel_url), Ok(handle), Ok(password)) = (
        std::env::var("RUPICOLA_PANEL_URL"),
        std::env::var("RUPICOLA_DEMO_HANDLE"),
        std::env::var("RUPICOLA_DEMO_PASSWORD"),
    ) else {
        return;
    };
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };
    let out = std::env::temp_dir().join("rupicola-cdp-panel-navigation-test");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let mut env = Environment::new([1280, 800]);
    env.warmup_ms = 1200;
    env.network_policy = NetworkPolicy::Unrestricted;
    let plan = Plan {
        target: Target {
            command: format!("{panel_url}/login"),
            args: Vec::new(),
        },
        environment: env,
        steps: vec![
            Step::Action {
                action: UiAction::TypeText {
                    target_id: "css=#handle".to_owned(),
                    text: handle,
                },
            },
            Step::Action {
                action: UiAction::TypeText {
                    target_id: "css=#password".to_owned(),
                    text: password,
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=button[type=\"submit\"]".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=a.button.primary[href=\"/projects\"]".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=a.button.primary[href=\"/projects\"]".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=main#main .project-index".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=a.project-card".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=a[aria-label=\"Fixes\"]".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=a[aria-label=\"Fixes\"]".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=a[aria-label=\"Fixes\"][aria-current=\"page\"]".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=a[aria-label=\"Problems\"]".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=a[aria-label=\"Problems\"][aria-current=\"page\"]".to_owned(),
                },
            },
        ],
        allow_degraded: false,
    };

    run_plan(&backend, &plan, &out).await.unwrap();
    let _ = std::fs::remove_dir_all(&out);
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
async fn cdp_records_compositor_frames_at_the_requested_interval_during_actions() {
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };
    if Command::new("ffprobe").arg("-version").output().is_err() {
        return;
    }

    let (url, _srv) = serve_page(ANIMATED_HTML);
    let out = std::env::temp_dir().join("rupicola-cdp-screencast-interval-test");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let mut env = Environment::new([800, VIEWPORT_HEIGHT]);
    env.warmup_ms = 0;
    env.network_policy = NetworkPolicy::Unrestricted;
    env.frame_interval_ms = Some(16);

    let plan = Plan {
        target: Target {
            command: url,
            args: Vec::new(),
        },
        environment: env,
        steps: vec![
            Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "css=#move".to_owned(),
                },
            },
            Step::StartRecording,
            Step::Action {
                action: UiAction::Click {
                    target_id: "css=#move".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Delay { ms: 1_000 },
            },
            Step::StopRecording {
                id: "animated-transition".to_owned(),
            },
        ],
        allow_degraded: false,
    };

    let bundle = run_plan(&backend, &plan, &out).await.unwrap();
    let clip = &bundle.clips[0];
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(&clip.mp4)
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let frames = String::from_utf8(probe.stdout)
        .unwrap()
        .trim()
        .parse::<usize>()
        .unwrap();
    let telemetry = clip
        .telemetry
        .as_ref()
        .expect("CDP clip should include frame telemetry");
    assert!(
        (telemetry.encoded_frames..=telemetry.encoded_frames + 1).contains(&frames),
        "expected ffprobe frame count ({frames}) to match encoded_frames ({})",
        telemetry.encoded_frames
    );
    assert_eq!(
        telemetry.dropped_frames, 0,
        "60 Hz compositor frames should not be dropped at interval_ms=16, telemetry: {telemetry:?}"
    );
    assert!(
        telemetry.raw_frames >= 45 && frames >= 45,
        "expected >= 45 compositor frames during 800ms animation at interval_ms=16, got raw={}, encoded={frames}, telemetry={telemetry:?}",
        telemetry.raw_frames
    );
    assert!(
        telemetry.max_raw_gap_ms.is_some_and(|gap| gap <= 80.0),
        "expected dense compositor cadence without large stalls (max_raw_gap_ms <= 80.0), got {telemetry:?}"
    );
    assert!(
        telemetry.effective_fps > 0.0,
        "expected positive effective_fps, got {telemetry:?}"
    );
    let _ = std::fs::remove_dir_all(&out);
}

#[tokio::test]
async fn cdp_warns_when_idle_page_falls_below_requested_fps() {
    let Some(backend) = CdpBackend::discover().await else {
        return;
    };
    if Command::new("ffprobe").arg("-version").output().is_err() {
        return;
    }

    let (url, _srv) = serve_html();
    let out = std::env::temp_dir().join("rupicola-cdp-low-fps-warning-test");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let mut env = Environment::new([800, VIEWPORT_HEIGHT]);
    env.warmup_ms = 0;
    env.network_policy = NetworkPolicy::Unrestricted;
    env.frame_interval_ms = Some(16);

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
            Step::Wait {
                condition: WaitCondition::Delay { ms: 600 },
            },
            Step::StopRecording {
                id: "idle-clip".to_owned(),
            },
        ],
        allow_degraded: false,
    };

    let bundle = run_plan(&backend, &plan, &out).await.unwrap();
    let clip = &bundle.clips[0];
    let telemetry = clip
        .telemetry
        .as_ref()
        .expect("CDP clip should include frame telemetry");
    assert!(
        telemetry.effective_fps < 30.0,
        "expected idle page effective_fps < 30.0, got {telemetry:?}"
    );
    assert!(
        bundle
            .warnings
            .iter()
            .any(|warning| warning.contains("idle-clip") && warning.contains("below requested")),
        "expected low-FPS warning in bundle.warnings, got {:?}",
        bundle.warnings
    );
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
