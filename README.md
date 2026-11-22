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
rupicola-protocol = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", tag = "v0.2.0" }
# Optional: include the capture engine
rupicola-capture = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", tag = "v0.2.0" }
```

## Real Example Applications

Every app under `examples/apps/` is a standalone functional application with **zero Rupicola code inside it**. Media is generated directly by `rupicola-capture`:

- [Tasks CRUD (`examples/apps/crud-tasks`)](examples/apps/crud-tasks/README.md) — Web form, filters, checkboxes, live DOM snapshot
- [Pomodoro Timer (`examples/apps/pomodoro-timer`)](examples/apps/pomodoro-timer/README.md) — Animated SVG ring, presets, theme toggle
- [Unit Calculator (`examples/apps/unit-calculator`)](examples/apps/unit-calculator/README.md) — Keypad grid, expression evaluation, unit converter
- [GTK4 Notes (`examples/apps/gtk4-notes`)](examples/apps/gtk4-notes/README.md) — Native Linux GTK4 app in pure Rust (captured headless via `Xvfb` and `gtk4-broadwayd`)
- [Basic protocol hello-worlds (`examples/basic`)](examples/basic/README.md) — `linux_iced`, `linux_gtk4`, `linux_libcosmic`, `web_html`, `web_react`

## Development & Verification

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

Regenerate all example screenshots, annotated videos, and inline GIFs:

```sh
cargo run -p rupicola-capture --example refresh_apps_media
```
