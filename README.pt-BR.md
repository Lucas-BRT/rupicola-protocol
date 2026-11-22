# rupicola-protocol

> Idioma: [English](README.md) | **Português (BR)**

Micro-protocolo leve de middleware para snapshots de árvore de widgets, *bounding boxes* e interação remota entre Web DOM e toolkits Desktop (`Iced`, `COSMIC`, `egui`, `GTK4`, `Tauri`, `Qt`).

## Transporte e Formato

As mensagens usam NDJSON (um objeto JSON por linha) via `stdio` ou Unix domain sockets:
- `ProtocolRequest`: `Handshake`, `Ping`, `GetSnapshot`, `PerformAction`
- `ProtocolResponse`: `HandshakeAck`, `Pong`, `Snapshot`, `ActionAck`, `Error`

## Exemplos e Testes

- [Exemplos básicos hello-world](examples/basic/README.pt-BR.md)
- [Tasks CRUD (`examples/apps/crud-tasks`)](examples/apps/crud-tasks/README.pt-BR.md)
- [Pomodoro Timer (`examples/apps/pomodoro-timer`)](examples/apps/pomodoro-timer/README.pt-BR.md)
- [Unit Calculator (`examples/apps/unit-calculator`)](examples/apps/unit-calculator/README.pt-BR.md)
- [GTK4 Notes (`examples/apps/gtk4-notes`, Desktop Linux Nativo)](examples/apps/gtk4-notes/README.pt-BR.md)

```sh
cargo test
cargo run --example linux_iced
cargo run --example linux_gtk4
cargo run --example linux_libcosmic
cargo run --example web_html
cargo run --example web_react
```
