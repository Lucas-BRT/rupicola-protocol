# rupicola-protocol

Lightweight middleware micro-protocol for UI widget tree snapshots, bounding boxes, and remote interaction across Web DOM and Desktop toolkits (`Iced`, `COSMIC`, `egui`, `GTK4`, `Tauri`, `Qt`).

## Transport & Wire Format

Messages use NDJSON (one JSON object per line) over `stdio` or Unix domain sockets:
- `ProtocolRequest`: `Handshake`, `Ping`, `GetSnapshot`, `PerformAction`
- `ProtocolResponse`: `HandshakeAck`, `Pong`, `Snapshot`, `ActionAck`, `Error`

## Examples & Tests

```sh
cargo test
cargo run --example linux_iced
cargo run --example linux_gtk4
cargo run --example linux_libcosmic
cargo run --example web_html
cargo run --example web_react
```
