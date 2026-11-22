//! The GTK4 Broadway backend as a session: a real GTK4 app served over HTTP and captured
//! through the web session.

#![cfg(unix)]

use rupicola_capture::{
    GtkBroadwayBackend, WebBackend,
    session::{Environment, Plan, Step, Target, run_plan},
};
use std::{fs, path::PathBuf, process::Command};

/// Broadway display, so the HTTP port is `8080 + 17`.
const DISPLAY: u16 = 17;
const VIEWPORT: [u32; 2] = [640, 480];
const WARMUP_MS: u64 = 1000;
const BROWSER_ENGINE: &str = "chromium";

fn available(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn has_gtk4() -> bool {
    Command::new("pkg-config")
        .args(["--exists", "gtk4"])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn a_gtk4_app_is_captured_without_x11() {
    let Some(web) = WebBackend::discover(BROWSER_ENGINE) else {
        return;
    };
    if !available("gtk4-broadwayd") || !has_gtk4() {
        return;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target_dir = std::env::temp_dir().join("rupicola-capture-gtk4-target");
    let build = Command::new("cargo")
        .args(["build", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("examples/apps/medium/notes/gtk4/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .unwrap();
    assert!(build.success());

    let out = std::env::temp_dir().join("rupicola-capture-broadway-out");
    let _ = fs::remove_dir_all(&out);
    let mut environment = Environment::new(VIEWPORT);
    environment.warmup_ms = WARMUP_MS;
    let plan = Plan {
        target: Target {
            command: target_dir
                .join("debug/gtk4-notes")
                .to_string_lossy()
                .into_owned(),
            args: vec![],
        },
        environment,
        steps: vec![Step::Checkpoint {
            id: "notes".to_owned(),
        }],
        allow_degraded: false,
    };

    let bundle = run_plan(&GtkBroadwayBackend::new(DISPLAY, web), &plan, &out).unwrap();

    assert!(bundle.checkpoints[0].png.is_file());
    let _ = fs::remove_dir_all(&out);
}
