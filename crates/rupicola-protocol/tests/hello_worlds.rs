#[path = "support/hello_world.rs"]
mod examples_common;

use rupicola_protocol::{
    PROTOCOL_VERSION, ProtocolRequest, ProtocolResponse, ToolkitKind, UiAction, read_ndjson,
    write_ndjson,
};
use std::io::BufReader;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::thread;

#[cfg(unix)]
fn verify_hello_world_example(
    app_id: &str,
    toolkit: &ToolkitKind,
    title: &str,
    style_hint: Option<&str>,
) {
    let (mut client_tx, server_rx) = UnixStream::pair().unwrap();
    let mut client_rx = BufReader::new(client_tx.try_clone().unwrap());
    let server_tx = server_rx.try_clone().unwrap();
    let mut snap = examples_common::hello_world_tree(app_id, toolkit.clone(), title, style_hint);

    let app_thread = thread::spawn(move || {
        examples_common::handle_session(BufReader::new(server_rx), server_tx, &mut snap).unwrap();
    });

    write_ndjson(
        &mut client_tx,
        &ProtocolRequest::Handshake {
            protocol_version: PROTOCOL_VERSION,
            client: "rupicola-test".into(),
        },
    )
    .unwrap();
    let ack: ProtocolResponse = read_ndjson(&mut client_rx).unwrap().unwrap();
    assert!(matches!(ack, ProtocolResponse::HandshakeAck { toolkit: ref t, .. } if t == toolkit));

    write_ndjson(&mut client_tx, &ProtocolRequest::Ping).unwrap();
    assert_eq!(
        read_ndjson::<_, ProtocolResponse>(&mut client_rx)
            .unwrap()
            .unwrap(),
        ProtocolResponse::Pong
    );

    write_ndjson(
        &mut client_tx,
        &ProtocolRequest::PerformAction {
            action: UiAction::Click {
                target_id: "counter-btn".into(),
            },
        },
    )
    .unwrap();
    let act: ProtocolResponse = read_ndjson(&mut client_rx).unwrap().unwrap();
    assert!(matches!(act, ProtocolResponse::ActionAck { ok: true, .. }));

    write_ndjson(&mut client_tx, &ProtocolRequest::GetSnapshot).unwrap();
    let ProtocolResponse::Snapshot { snapshot } =
        read_ndjson::<_, ProtocolResponse>(&mut client_rx)
            .unwrap()
            .unwrap()
    else {
        panic!("expected snapshot response");
    };
    assert_eq!(snapshot.flatten_boxes().len(), 3);
    assert_eq!(
        snapshot.find_by_id("counter-btn").unwrap().value.as_deref(),
        Some("1")
    );

    drop(client_rx);
    drop(client_tx);
    app_thread.join().unwrap();
}

#[cfg(unix)]
#[test]
fn hello_world_linux_iced() {
    verify_hello_world_example(
        "hello-iced",
        &ToolkitKind::Iced,
        "Hello Iced",
        Some("iced::widget::text"),
    );
}

#[cfg(unix)]
#[test]
fn hello_world_linux_gtk4() {
    verify_hello_world_example(
        "hello-gtk4",
        &ToolkitKind::Gtk4,
        "Hello GTK4",
        Some("gtk4::Label"),
    );
}

#[cfg(unix)]
#[test]
fn hello_world_linux_libcosmic() {
    verify_hello_world_example(
        "hello-cosmic",
        &ToolkitKind::Cosmic,
        "Hello COSMIC",
        Some("cosmic::widget::text"),
    );
}

#[cfg(unix)]
#[test]
fn hello_world_web_html_and_react() {
    verify_hello_world_example(
        "hello-html",
        &ToolkitKind::WebDom,
        "Hello HTML",
        Some("h1.hero"),
    );
    verify_hello_world_example(
        "hello-react",
        &ToolkitKind::WebDom,
        "Hello React",
        Some("data-testid=hello"),
    );
}
