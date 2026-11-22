//! Real end-to-end tests running the actual apps in `examples/apps/` without
//! modifying their source code: serves each app over HTTP, launches Chromium
//! headless, extracts live `rupicola_protocol::UiSnapshot` trees from the DOM,
//! dispatches real UI actions, and verifies the resulting UI state & bounds.

use rupicola_protocol::{ProtocolResponse, UiAction};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

fn spawn_static_server(dir: PathBuf) -> (String, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);

    thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0_u8; 2048];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);
                    let raw_path = req
                        .lines()
                        .next()
                        .and_then(|l| l.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let rel = if raw_path == "/" {
                        "index.html"
                    } else {
                        raw_path.trim_start_matches('/')
                    };
                    let target = dir.join(rel);
                    if let Ok(body) = fs::read(&target) {
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(resp.as_bytes());
                        let _ = stream.write_all(&body);
                    } else {
                        let _ = stream
                            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    (format!("http://{addr}/index.html"), stop)
}

fn find_node_and_playwright() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let node_candidates = [
        PathBuf::from("node"),
        home.join(".local/share/mise/installs/node/lts/bin/node"),
    ];
    let node = node_candidates.into_iter().find(|p| {
        (p.as_os_str() == "node" && Command::new(p).arg("--version").output().is_ok())
            || p.is_file()
    })?;
    let pw = home.join("Projects/pokemon-survivors/node_modules/playwright");
    if pw.exists() { Some((node, pw)) } else { None }
}

fn run_web_adapter(
    node: &Path,
    pw_dir: &Path,
    url: &str,
    actions: &[UiAction],
) -> ProtocolResponse {
    let script = r"
const pw = require(process.env.PW_DIR);

(async () => {
  const browser = await pw.chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  await page.goto(process.env.TARGET_URL, { waitUntil: 'networkidle' });

  const actions = JSON.parse(process.env.ACTIONS_JSON);
  for (const act of actions) {
    if (act.action === 'type-text') {
      await page.fill('#' + act.target_id, act.text);
    } else if (act.action === 'click') {
      const sel = act.target_id.startsWith('[') || act.target_id.startsWith('.')
        ? act.target_id
        : '#' + act.target_id;
      await page.click(sel);
    }
  }

  const snapshot = await page.evaluate(() => {
    function nodeToWidget(el, idx) {
      const r = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      const visible = style.display !== 'none' && style.visibility !== 'hidden' && r.width > 0 && r.height > 0;
      const children = [];
      let childIdx = 0;
      for (const ch of el.children) {
        children.push(nodeToWidget(ch, childIdx++));
      }
      return {
        id: el.id || `${el.tagName.toLowerCase()}-${idx}`,
        role: el.getAttribute('role') || el.tagName.toLowerCase(),
        label: (el.innerText || el.getAttribute('aria-label') || '').trim().slice(0, 120) || null,
        value: 'value' in el && typeof el.value === 'string' ? el.value : null,
        bounds: { x: r.x, y: r.y, width: r.width, height: r.height },
        state: {
          visible,
          enabled: !el.disabled,
          focused: document.activeElement === el,
          hovered: el.matches(':hover'),
          pressed: el.getAttribute('aria-pressed') === 'true',
          checked: 'checked' in el ? Boolean(el.checked) : null,
          selected: null
        },
        style_hint: el.className ? String(el.className) : null,
        children
      };
    }
    return {
      protocol_version: 1,
      app_id: document.title || 'web-app',
      toolkit: 'web-dom',
      title: document.title || null,
      viewport: [window.innerWidth, window.innerHeight],
      scale_factor: window.devicePixelRatio || 1,
      timestamp_ms: Date.now(),
      root: nodeToWidget(document.body, 0)
    };
  });

  await browser.close();
  process.stdout.write(JSON.stringify({ type: 'snapshot', snapshot }) + '\n');
})();
";

    let out = Command::new(node)
        .arg("-e")
        .arg(script)
        .env("PW_DIR", pw_dir)
        .env("TARGET_URL", url)
        .env("ACTIONS_JSON", serde_json::to_string(actions).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run playwright web adapter");
    assert!(
        out.status.success(),
        "adapter failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("parse ProtocolResponse::Snapshot from live DOM")
}

#[test]
fn real_example_apps_run_and_emit_live_protocol_snapshots() {
    let Some((node, pw)) = find_node_and_playwright() else {
        eprintln!(
            "skipping live browser e2e test: node/playwright unavailable in current environment"
        );
        return;
    };
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // 1. Live Tasks CRUD app
    let (url, stop) = spawn_static_server(manifest_dir.join("examples/apps/crud-tasks"));
    let ProtocolResponse::Snapshot { snapshot } = run_web_adapter(
        &node,
        &pw,
        &url,
        &[
            UiAction::TypeText {
                target_id: "new-task".into(),
                text: "E2E protocol item".into(),
            },
            UiAction::Click {
                target_id: "add-btn".into(),
            },
        ],
    ) else {
        panic!("expected snapshot");
    };
    stop.store(true, Ordering::Relaxed);
    assert!(snapshot.flatten_boxes().len() >= 8);
    let counter = snapshot
        .find_by_id("counter")
        .expect("counter element in live DOM");
    assert_eq!(counter.label.as_deref(), Some("3 remaining · 4 total"));
    assert!(counter.bounds.is_valid());

    // 2. Live Pomodoro Timer app
    let (url, stop) = spawn_static_server(manifest_dir.join("examples/apps/pomodoro-timer"));
    let ProtocolResponse::Snapshot { snapshot } = run_web_adapter(
        &node,
        &pw,
        &url,
        &[UiAction::Click {
            target_id: "[data-secs=\"300\"]".into(),
        }],
    ) else {
        panic!("expected snapshot");
    };
    stop.store(true, Ordering::Relaxed);
    let display = snapshot
        .find_by_id("display")
        .expect("timer display in live DOM");
    assert_eq!(display.label.as_deref(), Some("05:00"));
    assert!(display.bounds.is_valid());

    // 3. Live Unit Calculator app
    let (url, stop) = spawn_static_server(manifest_dir.join("examples/apps/unit-calculator"));
    let ProtocolResponse::Snapshot { snapshot } = run_web_adapter(
        &node,
        &pw,
        &url,
        &[
            UiAction::Click {
                target_id: "[data-k=\"+\"]".into(),
            },
            UiAction::Click {
                target_id: "[data-k=\"1\"]".into(),
            },
            UiAction::Click {
                target_id: "[data-k=\"6\"]".into(),
            },
            UiAction::Click {
                target_id: "[data-k=\"=\"]".into(),
            },
        ],
    ) else {
        panic!("expected snapshot");
    };
    stop.store(true, Ordering::Relaxed);
    let val = snapshot.find_by_id("val").expect("calc value in live DOM");
    assert_eq!(val.label.as_deref(), Some("64"));
    let conv = snapshot
        .find_by_id("conv")
        .expect("unit converter in live DOM");
    assert_eq!(conv.label.as_deref(), Some("64 px = 4.00 rem (base 16px)"));
}

#[cfg(unix)]
#[test]
fn real_gtk4_desktop_app_runs_in_background_and_renders_frame() {
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
        eprintln!("skipping GTK4 desktop e2e test: Xvfb, ffmpeg, or gtk4 unavailable");
        return;
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let app_manifest = manifest_dir.join("examples/apps/gtk4-notes/Cargo.toml");
    let target_dir = std::env::temp_dir().join("rupicola-gtk4-target");
    let build = Command::new("cargo")
        .args(["build", "--offline", "--quiet", "--manifest-path"])
        .arg(&app_manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .expect("build gtk4-notes binary");
    assert!(build.success(), "gtk4-notes failed to compile");

    let app_bin = target_dir.join("debug/gtk4-notes");
    let out_png = std::env::temp_dir().join("rupicola-gtk4-e2e.png");
    let _ = fs::remove_file(&out_png);

    let mut xvfb = Command::new("Xvfb")
        .args([":96", "-screen", "0", "640x480x24"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn Xvfb");
    thread::sleep(std::time::Duration::from_millis(500));

    let mut app = Command::new(&app_bin)
        .env("GDK_BACKEND", "x11")
        .env("GSK_RENDERER", "cairo")
        .env("DISPLAY", ":96")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn GTK4 Notes app");
    thread::sleep(std::time::Duration::from_millis(1200));

    let grab = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "x11grab",
            "-video_size",
            "640x480",
            "-i",
            ":96",
            "-frames:v",
            "1",
        ])
        .arg(&out_png)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run ffmpeg x11grab");

    let _ = app.kill();
    let _ = app.wait();
    let _ = xvfb.kill();
    let _ = xvfb.wait();

    assert!(grab.success(), "ffmpeg x11grab failed");
    let bytes = fs::read(&out_png).expect("read captured GTK4 PNG");
    let _ = fs::remove_file(&out_png);
    assert!(
        bytes.len() > 5_000,
        "expected rendered GTK4 window PNG, got {} bytes",
        bytes.len()
    );
}
