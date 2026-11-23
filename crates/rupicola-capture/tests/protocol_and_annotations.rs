//! Behaviour of the protocol and annotation types that does not depend on how captures run.

use rupicola_protocol::{
    AnnotationBox, AnnotationKeyframe, AnnotationShape, AnnotationStyle, Easing, LabelPosition,
    Rect, TimeRange,
};
#[cfg(unix)]
use rupicola_protocol::{ProtocolRequest, ProtocolResponse, UnixProtocolClient, serve_once_unix};
#[cfg(unix)]
use std::fs;
use std::thread;
use std::time::Duration;

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
