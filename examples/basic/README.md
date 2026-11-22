# Basic Examples (Hello World — `rupicola-protocol`)

> Language: **English** | [Português (BR)](README.pt-BR.md)

Minimal examples demonstrating the [`rupicola-protocol`](../../Cargo.toml) micro-protocol across supported toolkits and environments:

- `linux_iced` (`ToolkitKind::Iced`)
- `linux_gtk4` (`ToolkitKind::Gtk4`)
- `linux_libcosmic` (`ToolkitKind::Cosmic`)
- `web_html` (`ToolkitKind::WebDom`)
- `web_react` (`ToolkitKind::WebDom`)

## Running manually

Print the initial `UiSnapshot` JSON:

```sh
cargo run -p rupicola-protocol --example linux_iced
cargo run -p rupicola-protocol --example linux_gtk4
cargo run -p rupicola-protocol --example linux_libcosmic
cargo run -p rupicola-protocol --example web_html
cargo run -p rupicola-protocol --example web_react
```

Run an interactive NDJSON session over `stdio` or Unix domain socket:

```sh
cargo run -p rupicola-protocol --example linux_iced -- --stdio
cargo run -p rupicola-protocol --example linux_libcosmic -- --socket /tmp/rupicola-cosmic.sock
```

Run the automated integration tests backed by these examples:

```sh
cargo test -p rupicola-protocol --test hello_worlds
```
