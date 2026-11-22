# rupicola-protocol

> Language: **English** | [Português (BR)](README.pt-BR.md)

Lightweight middleware micro-protocol for UI widget tree snapshots, bounding boxes, and remote interaction across Web DOM and Desktop toolkits (`Iced`, `COSMIC`, `egui`, `GTK4`, `Tauri`, `Qt`).

## Transport & Wire Format

Messages use NDJSON (one JSON object per line) over `stdio` or Unix domain sockets:
- `ProtocolRequest`: `Handshake`, `Ping`, `GetSnapshot`, `PerformAction`
- `ProtocolResponse`: `HandshakeAck`, `Pong`, `Snapshot`, `ActionAck`, `Error`

## Examples & Tests

- [Basic hello-world examples](examples/basic/README.md)
- [Tasks CRUD (`examples/apps/crud-tasks`)](examples/apps/crud-tasks/README.md)
- [Pomodoro Timer (`examples/apps/pomodoro-timer`)](examples/apps/pomodoro-timer/README.md)
- [Unit Calculator (`examples/apps/unit-calculator`)](examples/apps/unit-calculator/README.md)
- [GTK4 Notes (`examples/apps/gtk4-notes`, Native Linux Desktop)](examples/apps/gtk4-notes/README.md)

```sh
cargo test
cargo run --example linux_iced
cargo run --example linux_gtk4
cargo run --example linux_libcosmic
cargo run --example web_html
cargo run --example web_react
```
