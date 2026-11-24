use rupicola_protocol::{
    PROTOCOL_VERSION, ProtocolRequest, ProtocolResponse, Rect, SnapshotSource, ToolkitKind,
    UiAction, UiSnapshot, WidgetKind, WidgetNode, WidgetState, read_ndjson, write_ndjson,
};
use std::io::Cursor;

#[test]
fn rect_validation_and_hidpi_scaling_edge_cases() {
    assert!(
        !Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 10.0
        }
        .is_valid()
    );
    assert!(
        !Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: -1.0
        }
        .is_valid()
    );
    assert!(
        !Rect {
            x: f64::NAN,
            y: 0.0,
            width: 10.0,
            height: 10.0
        }
        .is_valid()
    );
    assert!(
        !Rect {
            x: 0.0,
            y: f64::INFINITY,
            width: 10.0,
            height: 10.0
        }
        .is_valid()
    );

    let rect = Rect {
        x: -4.0,
        y: 12.5,
        width: 100.0,
        height: 32.0,
    };
    assert!(rect.is_valid());
    // Negative origin clamps cleanly to 0; non-positive/NaN scale falls back to 1.0.
    assert_eq!(rect.to_pixel_box(2.0), [0, 25, 200, 64]);
    assert_eq!(rect.to_pixel_box(0.0), [0, 13, 100, 32]);
    assert_eq!(rect.to_pixel_box(f64::NAN), [0, 13, 100, 32]);
}

#[test]
fn all_ui_actions_and_error_responses_roundtrip_ndjson() {
    let actions = [
        UiAction::Click {
            target_id: "btn-ok".into(),
        },
        UiAction::ClickAt { x: 42.5, y: 88.0 },
        UiAction::Hover {
            target_id: "card-1".into(),
        },
        UiAction::Focus {
            target_id: "email".into(),
        },
        UiAction::TypeText {
            target_id: "email".into(),
            text: "dev@rupicola.dev".into(),
        },
        UiAction::PressKey {
            key: "Enter".into(),
            modifiers: vec!["Control".into()],
        },
        UiAction::Scroll {
            target_id: Some("list".into()),
            dx: 0.0,
            dy: 240.0,
        },
        UiAction::ScrollIntoView {
            target_id: "footer".into(),
        },
        UiAction::ScrollTo {
            target_id: None,
            x: 0.0,
            y: 1200.0,
        },
        UiAction::ScrollToEnd {
            target_id: Some("list".into()),
        },
        UiAction::WaitMs { ms: 150 },
    ];

    let mut wire = Vec::new();
    for action in &actions {
        write_ndjson(
            &mut wire,
            &ProtocolRequest::PerformAction {
                action: action.clone(),
            },
        )
        .unwrap();
    }
    write_ndjson(
        &mut wire,
        &ProtocolResponse::Error {
            code: "target-not-found".into(),
            message: "No widget with id 'missing'".into(),
        },
    )
    .unwrap();

    let mut reader = Cursor::new(wire);
    for expected in &actions {
        let parsed: ProtocolRequest = read_ndjson(&mut reader).unwrap().unwrap();
        assert_eq!(
            parsed,
            ProtocolRequest::PerformAction {
                action: expected.clone()
            }
        );
    }
    let err_resp: ProtocolResponse = read_ndjson(&mut reader).unwrap().unwrap();
    assert!(
        matches!(err_resp, ProtocolResponse::Error { ref code, .. } if code == "target-not-found")
    );
    assert!(
        read_ndjson::<_, ProtocolResponse>(&mut reader)
            .unwrap()
            .is_none()
    );
}

#[test]
fn malformed_ndjson_line_returns_invalid_data_error() {
    let mut bad = Cursor::new(b"{not-valid-json\n".as_slice());
    let err = read_ndjson::<_, ProtocolRequest>(&mut bad).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn snapshot_flattens_visible_boxes_only_and_finds_nested_ids() {
    let snap = UiSnapshot {
        protocol_version: PROTOCOL_VERSION,
        app_id: "nested-demo".into(),
        toolkit: ToolkitKind::Egui,
        title: None,
        viewport: [800, 600],
        scale_factor: 1.0,
        timestamp_ms: 100,
        source: SnapshotSource::Adapter,
        root: WidgetNode {
            id: "root".into(),
            role: "window".into(),
            kind: WidgetKind::Window,
            label: None,
            value: None,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            state: WidgetState {
                visible: true,
                enabled: true,
                ..WidgetState::default()
            },
            style_hint: None,
            children: vec![
                WidgetNode {
                    id: "panel".into(),
                    role: "container".into(),
                    kind: WidgetKind::Group,
                    label: None,
                    value: None,
                    bounds: Rect {
                        x: 10.0,
                        y: 10.0,
                        width: 300.0,
                        height: 200.0,
                    },
                    state: WidgetState {
                        visible: true,
                        enabled: true,
                        ..WidgetState::default()
                    },
                    style_hint: None,
                    children: vec![WidgetNode {
                        id: "deep-input".into(),
                        role: "textbox".into(),
                        kind: WidgetKind::TextBox,
                        label: Some("Name".into()),
                        value: Some("Rupicola".into()),
                        bounds: Rect {
                            x: 20.0,
                            y: 30.0,
                            width: 200.0,
                            height: 32.0,
                        },
                        state: WidgetState {
                            visible: true,
                            enabled: true,
                            focused: true,
                            ..WidgetState::default()
                        },
                        style_hint: None,
                        children: vec![],
                    }],
                },
                WidgetNode {
                    id: "hidden-modal".into(),
                    role: "dialog".into(),
                    kind: WidgetKind::Dialog,
                    label: Some("Hidden".into()),
                    value: None,
                    bounds: Rect {
                        x: 100.0,
                        y: 100.0,
                        width: 200.0,
                        height: 150.0,
                    },
                    state: WidgetState {
                        visible: false,
                        enabled: true,
                        ..WidgetState::default()
                    },
                    style_hint: None,
                    children: vec![],
                },
            ],
        },
    };

    let boxes = snap.flatten_boxes();
    assert_eq!(boxes.len(), 3);
    assert!(boxes.iter().all(|b| b.id != "hidden-modal"));
    let deep = snap.find_by_id("deep-input").unwrap();
    assert_eq!(deep.value.as_deref(), Some("Rupicola"));
    assert!(deep.state.focused);
}
