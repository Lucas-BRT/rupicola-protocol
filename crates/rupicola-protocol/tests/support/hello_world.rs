//! Shared hello-world UI scenarios & NDJSON server loop used by both
//! runnable `examples/` and integration `tests/hello_worlds.rs`.

#![allow(
    dead_code,
    reason = "shared helpers used selectively by individual examples and tests"
)]

use rupicola_protocol::{
    PROTOCOL_VERSION, ProtocolRequest, ProtocolResponse, Rect, ToolkitKind, UiAction, UiSnapshot,
    WidgetNode, WidgetState, read_ndjson, write_ndjson,
};
use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixListener;

#[must_use]
pub fn hello_world_tree(
    app_id: &str,
    toolkit: ToolkitKind,
    title: &str,
    style_hint: Option<&str>,
) -> UiSnapshot {
    UiSnapshot {
        protocol_version: PROTOCOL_VERSION,
        app_id: app_id.into(),
        toolkit,
        title: Some(title.into()),
        viewport: [1280, 720],
        scale_factor: 1.0,
        timestamp_ms: 1_795_300_000_000,
        root: WidgetNode {
            id: "root".into(),
            role: "window".into(),
            label: Some(title.into()),
            value: None,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 1280.0,
                height: 720.0,
            },
            state: WidgetState {
                visible: true,
                enabled: true,
                ..WidgetState::default()
            },
            style_hint: None,
            children: vec![
                WidgetNode {
                    id: "hello-heading".into(),
                    role: "heading".into(),
                    label: Some("Hello, world!".into()),
                    value: None,
                    bounds: Rect {
                        x: 48.0,
                        y: 48.0,
                        width: 320.0,
                        height: 40.0,
                    },
                    state: WidgetState {
                        visible: true,
                        enabled: true,
                        ..WidgetState::default()
                    },
                    style_hint: style_hint.map(str::to_owned),
                    children: vec![],
                },
                WidgetNode {
                    id: "counter-btn".into(),
                    role: "button".into(),
                    label: Some("Increment (0)".into()),
                    value: Some("0".into()),
                    bounds: Rect {
                        x: 48.0,
                        y: 108.0,
                        width: 160.0,
                        height: 40.0,
                    },
                    state: WidgetState {
                        visible: true,
                        enabled: true,
                        ..WidgetState::default()
                    },
                    style_hint: Some("primary".into()),
                    children: vec![],
                },
            ],
        },
    }
}

pub fn handle_session<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    snap: &mut UiSnapshot,
) -> io::Result<()> {
    let mut clicks: u32 = 0;
    while let Some(req) = read_ndjson::<_, ProtocolRequest>(&mut reader)? {
        let resp = match req {
            ProtocolRequest::Handshake { .. } => ProtocolResponse::HandshakeAck {
                protocol_version: PROTOCOL_VERSION,
                app_id: snap.app_id.clone(),
                toolkit: snap.toolkit.clone(),
                capabilities: vec!["snapshot".into(), "click".into(), "hover".into()],
            },
            ProtocolRequest::Ping => ProtocolResponse::Pong,
            ProtocolRequest::GetSnapshot => ProtocolResponse::Snapshot {
                snapshot: Box::new(snap.clone()),
            },
            ProtocolRequest::PerformAction { action } => match action {
                UiAction::Click { target_id } if target_id == "counter-btn" => {
                    clicks += 1;
                    if let Some(btn) = snap.root.children.get_mut(1) {
                        btn.label = Some(format!("Increment ({clicks})"));
                        btn.value = Some(clicks.to_string());
                        btn.state.pressed = true;
                    }
                    ProtocolResponse::ActionAck {
                        ok: true,
                        message: Some(format!("clicked ({clicks})")),
                    }
                }
                _ => ProtocolResponse::Error {
                    code: "unsupported".into(),
                    message: "unexpected hello-world action".into(),
                },
            },
        };
        write_ndjson(&mut writer, &resp)?;
    }
    Ok(())
}

/// CLI entrypoint for manual inspection:
/// - default: prints formatted `UiSnapshot` JSON
/// - `--stdio`: runs interactive NDJSON session on stdin/stdout
/// - `--socket <path>`: listens on a Unix domain socket
pub fn run_example_cli(app_id: &str, toolkit: ToolkitKind, title: &str, style_hint: Option<&str>) {
    let mut snap = hello_world_tree(app_id, toolkit, title, style_hint);
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--stdio") {
        let stdin = io::stdin();
        let stdout = io::stdout();
        handle_session(BufReader::new(stdin.lock()), stdout.lock(), &mut snap)
            .expect("stdio protocol session");
        return;
    }
    #[cfg(unix)]
    if let Some(pos) = args.iter().position(|a| a == "--socket")
        && let Some(sock_path) = args.get(pos + 1)
    {
        let _ = std::fs::remove_file(sock_path);
        let listener = UnixListener::bind(sock_path).expect("bind unix socket");
        eprintln!("{app_id}: listening on {sock_path}");
        for stream in listener.incoming().flatten() {
            if let Ok(writer) = stream.try_clone() {
                let _ = handle_session(BufReader::new(stream), writer, &mut snap);
            }
        }
        return;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&snap).expect("serialize hello-world snapshot")
    );
}
