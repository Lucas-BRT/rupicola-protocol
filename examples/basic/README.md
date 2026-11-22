# Basic Examples (Hello World — `rupicola-protocol`)

Exemplos mínimos que demonstram o micro-protocolo [`rupicola-protocol`](../../protocol/Cargo.toml) nos principais toolkits e ambientes suportados:

- `linux_iced` (`ToolkitKind::Iced`)
- `linux_gtk4` (`ToolkitKind::Gtk4`)
- `linux_libcosmic` (`ToolkitKind::Cosmic`)
- `web_html` (`ToolkitKind::WebDom`)
- `web_react` (`ToolkitKind::WebDom`)

## Como rodar manualmente

Imprimir o `UiSnapshot` inicial em JSON:

```sh
cargo run -p rupicola-protocol --example linux_iced
cargo run -p rupicola-protocol --example linux_gtk4
cargo run -p rupicola-protocol --example linux_libcosmic
cargo run -p rupicola-protocol --example web_html
cargo run -p rupicola-protocol --example web_react
```

Rodar sessão interativa NDJSON via `stdio` ou Unix socket:

```sh
cargo run -p rupicola-protocol --example linux_iced -- --stdio
cargo run -p rupicola-protocol --example linux_libcosmic -- --socket /tmp/rupicola-cosmic.sock
```

Rodar os testes automatizados que consomem estes exemplos:

```sh
cargo test -p rupicola-protocol --test hello_worlds
```
