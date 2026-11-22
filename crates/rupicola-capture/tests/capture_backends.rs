#[cfg(unix)]
use rupicola_capture::LinuxX11Backend;
use rupicola_capture::session::{
    self, Environment, ErrorClass, Plan, Step, Target, WaitCondition, run_plan,
};
use rupicola_capture::{AndroidAdbBackend, annotate_png, annotate_video};
use rupicola_protocol::{
    AnnotationBox, AnnotationKeyframe, AnnotationShape, AnnotationStyle, Easing, LabelPosition,
    Rect, TimeRange,
};
#[cfg(unix)]
use rupicola_protocol::{ProtocolRequest, ProtocolResponse, UnixProtocolClient, serve_once_unix};
use std::fs;
#[cfg(unix)]
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Display number of the annotation test; distinct from every session test.
#[cfg(unix)]
const ANNOTATE_DISPLAY: u16 = 94;
const VIEWPORT: [u32; 2] = [320, 240];
const WARMUP_MS: u64 = 100;
/// Pause that gives the clip some length.
#[cfg(unix)]
const CLIP_MS: u64 = 600;
/// How long the placeholder target lives.
#[cfg(unix)]
const TARGET_SECONDS: &str = "30";

#[test]
fn temporal_annotation_box_interpolates_keyframes_and_normalizes_percent() {
    let ann = AnnotationBox {
        bounds: Rect {
            x: 64.0,
            y: 48.0,
            width: 320.0,
            height: 96.0,
        },
        label: "toast".into(),
        shape: AnnotationShape::Spotlight,
        style: AnnotationStyle {
            stroke_color: "#ef7e4d".into(),
            stroke_width: 3.0,
            fill_color: Some("rgba(239,126,77,0.12)".into()),
            border_radius: 12.0,
            dashed: true,
            label_position: LabelPosition::Bottom,
        },
        time_range: Some(TimeRange {
            start_ms: 500,
            end_ms: 1500,
        }),
        keyframes: vec![
            AnnotationKeyframe {
                timestamp_ms: 500,
                bounds: Rect {
                    x: 64.0,
                    y: 48.0,
                    width: 320.0,
                    height: 96.0,
                },
                easing: Easing::Linear,
            },
            AnnotationKeyframe {
                timestamp_ms: 1500,
                bounds: Rect {
                    x: 164.0,
                    y: 148.0,
                    width: 320.0,
                    height: 96.0,
                },
                easing: Easing::Linear,
            },
        ],
    };
    assert!(!ann.active_at(499));
    assert!(ann.active_at(500));
    assert!(ann.active_at(1500));
    assert!(!ann.active_at(1501));

    let mid = ann.bounds_at(1000);
    assert!((mid.x - 114.0).abs() < 0.01);
    assert!((mid.y - 98.0).abs() < 0.01);

    let pct = ann.percent_box_at(500, [640, 480]).unwrap();
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
fn annotates_a_session_screenshot_and_clip() {
    let available = |tool: &str| {
        Command::new("which")
            .arg(tool)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if !available("Xvfb") || !available("ffmpeg") {
        return;
    }
    let out_dir = std::env::temp_dir().join("rupicola-capture-annotate-session");
    let _ = fs::remove_dir_all(&out_dir);
    let mut environment = Environment::new(VIEWPORT);
    environment.warmup_ms = WARMUP_MS;
    let plan = Plan {
        target: Target {
            command: "sleep".to_owned(),
            args: vec![TARGET_SECONDS.to_owned()],
        },
        environment,
        steps: vec![
            Step::StartRecording,
            Step::Wait {
                condition: WaitCondition::Delay { ms: CLIP_MS },
            },
            Step::Checkpoint {
                id: "shot".to_owned(),
            },
            Step::StopRecording {
                id: "clip".to_owned(),
            },
        ],
        allow_degraded: false,
    };
    let bundle = run_plan(
        &LinuxX11Backend {
            display_num: ANNOTATE_DISPLAY,
        },
        &plan,
        &out_dir,
    )
    .unwrap();
    let boxes = [AnnotationBox {
        bounds: Rect {
            x: 24.0,
            y: 66.0,
            width: 100.0,
            height: 38.0,
        },
        label: "note-entry".into(),
        shape: AnnotationShape::Rect,
        style: AnnotationStyle::default(),
        time_range: None,
        keyframes: vec![],
    }];

    let annotated_png = out_dir.join("shot-annotated.png");
    annotate_png(&bundle.checkpoints[0].png, &annotated_png, &boxes, 1.0).unwrap();
    let annotated_mp4 = out_dir.join("clip-annotated.mp4");
    annotate_video(&bundle.clips[0].mp4, &annotated_mp4, &boxes, 1.0).unwrap();

    assert!(annotated_png.is_file());
    assert!(annotated_mp4.is_file());
    let _ = fs::remove_dir_all(&out_dir);
}

#[test]
fn android_session_reports_a_missing_adb_or_device_as_an_environment_error() {
    let backend = AndroidAdbBackend::default();
    assert_eq!(session::CaptureBackend::id(&backend), "mobile-android-adb");
    let target = Target {
        command: String::new(),
        args: vec![],
    };
    let out = std::env::temp_dir().join("rupicola-capture-android-out");
    match session::CaptureBackend::start(&backend, &target, &Environment::new(VIEWPORT), &out) {
        // A device is attached: the session must be usable and end cleanly.
        Ok(session) => session.finish().unwrap(),
        Err(error) => assert_eq!(error.class, ErrorClass::Environment),
    }
    let _ = fs::remove_dir_all(&out);
}
