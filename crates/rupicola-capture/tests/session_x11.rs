//! The X11 backend as a session: one Xvfb display serves several checkpoints and a clip.

#![cfg(unix)]

use rupicola_capture::{
    session::{Environment, Plan, Step, Target, WaitCondition},
    sync::{LinuxX11Backend, run_plan},
};
use std::{fs, process::Command};

/// Display numbers, one per test since the tests run in parallel, and distinct from the
/// one the legacy test uses.
const CLIP_DISPLAY: u16 = 93;
const MISSING_TARGET_DISPLAY: u16 = 92;
const UNSAFE_ID_DISPLAY: u16 = 91;
const VIEWPORT: [u32; 2] = [320, 240];
/// Pause between the two checkpoints, so the clip has some length.
const CLIP_MS: u64 = 600;
/// How long the placeholder target lives.
const TARGET_SECONDS: &str = "30";

fn available(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn one_display_serves_two_checkpoints_and_a_clip() {
    if !available("Xvfb") || !available("ffmpeg") {
        return;
    }
    let out = std::env::temp_dir().join("rupicola-capture-session-x11");
    let _ = fs::remove_dir_all(&out);
    let mut environment = Environment::new(VIEWPORT);
    environment.warmup_ms = 100;
    let plan = Plan {
        target: Target {
            command: "sleep".to_owned(),
            args: vec![TARGET_SECONDS.to_owned()],
        },
        environment,
        steps: vec![
            Step::Checkpoint {
                id: "before".to_owned(),
            },
            Step::StartRecording,
            Step::Wait {
                condition: WaitCondition::Delay { ms: CLIP_MS },
            },
            Step::Checkpoint {
                id: "after".to_owned(),
            },
            Step::StopRecording {
                id: "walk".to_owned(),
            },
        ],
        allow_degraded: false,
    };
    let bundle = run_plan(
        &LinuxX11Backend {
            display_num: CLIP_DISPLAY,
        },
        &plan,
        &out,
    )
    .unwrap();
    assert_eq!(bundle.checkpoints.len(), 2);
    assert!(bundle.checkpoints.iter().all(|c| c.png.is_file()));
    assert!(bundle.clips[0].mp4.is_file());
    assert!(!out.join(".recording.mp4").exists());
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn a_missing_target_is_a_target_error_and_leaves_nothing_running() {
    if !available("Xvfb") {
        return;
    }
    let out = std::env::temp_dir().join("rupicola-capture-session-x11-missing");
    let plan = Plan {
        target: Target {
            command: "/nonexistent/rupicola-target".to_owned(),
            args: Vec::new(),
        },
        environment: Environment::new(VIEWPORT),
        steps: Vec::new(),
        allow_degraded: false,
    };
    let error = run_plan(
        &LinuxX11Backend {
            display_num: MISSING_TARGET_DISPLAY,
        },
        &plan,
        &out,
    )
    .unwrap_err();
    assert_eq!(error.class, rupicola_capture::session::ErrorClass::Target);
    let _ = fs::remove_dir_all(&out);
}

#[test]
fn an_unsafe_id_is_refused_before_anything_starts() {
    let plan = Plan {
        target: Target {
            command: "sleep".to_owned(),
            args: Vec::new(),
        },
        environment: Environment::new(VIEWPORT),
        steps: vec![Step::Checkpoint {
            id: "../escape".to_owned(),
        }],
        allow_degraded: false,
    };
    let out = std::env::temp_dir().join("rupicola-capture-session-x11-id");
    let error = run_plan(
        &LinuxX11Backend {
            display_num: UNSAFE_ID_DISPLAY,
        },
        &plan,
        &out,
    )
    .unwrap_err();
    assert!(error.message.contains("valid id"));
    assert!(!out.exists());
}
