# rupicola-protocol


Lightweight UI inspection micro-protocol and headless multi-backend capture engine for Web, Linux Desktop, and Android applications.

## Workspace Crates

| Crate | Path | Purpose |
| :--- | :--- | :--- |
| **`rupicola-protocol`** | [`Cargo.toml`](Cargo.toml) | Zero-bloat wire types (`UiSnapshot`, `WidgetNode`, `UiAction`, `AnnotationBox` with `AnnotationStyle`, `AnnotationShape`, `TimeRange` & `keyframes`), NDJSON codec, and Unix domain socket client/server. |
| **`rupicola-capture`** | [`capture/Cargo.toml`](capture/Cargo.toml) | Headless capture backends, PNG/MP4 visual annotation engine (`ffmpeg drawbox`), and inline GIF generator. |

## Capture Backends (`rupicola-capture`)

| Backend | Target | Requirements | Outputs |
| :--- | :--- | :--- | :--- |
| `WebBackend` | Web (`chromium` / `firefox` / `webkit`) | Node.js + Playwright, `ffmpeg` | PNG, annotated PNG, MP4, annotated MP4, GIFs, live DOM `UiSnapshot` |
| `CdpBackend` | Web (no Node/Playwright) | Local Chromium / Chrome / Brave | PNG, annotated PNG |
| `LinuxX11Backend` | Linux Desktop (GTK4, Iced, egui, Qt) | `Xvfb`, `ffmpeg` | PNG, annotated PNG, MP4, annotated MP4, GIFs |
| `GtkBroadwayBackend` | Linux GTK4 (no X11/Wayland) | `gtk4-broadwayd`, `WebBackend` | PNG, annotated PNG, MP4, GIFs |
| `LinuxWaylandBackend` | Linux Wayland (`cosmic-comp` / `weston`) | Compositor + `grim` | PNG, annotated PNG |
| `AndroidAdbBackend` | Android Emulator / Device | `adb`, `ffmpeg` | PNG, annotated PNG, MP4, annotated MP4, GIFs |

## Usage as an External Dependency

Pin by release tag in your `Cargo.toml`:

```toml
[dependencies]
rupicola-protocol = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", tag = "v0.3.0" }
# Optional: include the capture engine
rupicola-capture = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", tag = "v0.3.0" }
```

## Upgrading from 0.2.0

- Rust APIs for coordinates, scale, and annotation interpolation now use `f64`. Update typed arguments and bindings that used `f32`.
- The NDJSON message tags and field names are unchanged. Numeric values encoded by 0.2.0 remain readable, and `PROTOCOL_VERSION` stays at 1.
- Use `rupicola_capture::session::run_plan` to perform actions, take checkpoints, and record clips in one session. The one-shot capture API remains available.

## Repository layout

- `crates/rupicola-protocol/` contains UI messages, geometry, and transport.
- `crates/rupicola-capture/` contains capture plans, sessions, backends, and media processing.
- `tools/example-capture/` generates media for the demo apps in `examples/apps/`.

## Real Example Applications

The functional apps below contain **zero Rupicola code inside them**; some basic protocol examples simulate snapshots. Media is generated directly by `rupicola-capture`:

- [Tasks CRUD (`examples/apps/medium/tasks/html`)](examples/apps/medium/tasks/html/README.md) — Web form, filters, checkboxes, live DOM snapshot
- [Pomodoro Timer (`examples/apps/medium/pomodoro/html`)](examples/apps/medium/pomodoro/html/README.md) — Animated SVG ring, presets, theme toggle
- [Unit Calculator (`examples/apps/medium/unit-calculator/html`)](examples/apps/medium/unit-calculator/html/README.md) — Keypad grid, expression evaluation, unit converter
- [GTK4 Notes (`examples/apps/medium/notes/gtk4`)](examples/apps/medium/notes/gtk4/README.md) — Native Linux GTK4 app in pure Rust (captured headless via `Xvfb` and `gtk4-broadwayd`)
- [Basic examples and protocol fixtures (`examples/apps`)](examples/apps/README.md) — `linux_iced`, `linux_gtk4`, `linux_libcosmic`, `web_html`, `web_react`

## Development & Verification

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

Regenerate all example screenshots, annotated videos, and inline GIFs:

```sh
cargo run -p example-capture -- refresh-medium
```
