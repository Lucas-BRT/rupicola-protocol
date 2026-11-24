# rupicola-protocol


Lightweight UI inspection micro-protocol and headless multi-backend capture engine for Web, Linux Desktop, and Android applications.

## Getting Started

This walks through capturing a small page with `rupicola-capture`: describe the capture as a plan, run it on a backend, and read what it produced. The finished program is [`crates/rupicola-capture/examples/capture_a_page.rs`](crates/rupicola-capture/examples/capture_a_page.rs).

### 1. Add the crate and choose how captures run

```toml
[dependencies]
rupicola-protocol = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", branch = "feat/capture-session" }
rupicola-capture = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", branch = "feat/capture-session", features = ["async"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

`rupicola-capture` has no default features: you pick how captures run, and a build with neither feature fails with a message that says so.

| Feature | Module | How it runs |
| :--- | :--- | :--- |
| `async` | `rupicola_capture::asynchronous` | Processes and files on tokio; needs a tokio runtime. |
| `sync` | `rupicola_capture::sync` | Blocks the calling thread on its processes. |

Both modules have the same names (`run_plan`, `CaptureBackend`, the backends, `annotate_png`, ...) and the same behaviour, and both features can be on at once. The rest of this guide uses `async`; with `sync`, import from `rupicola_capture::sync` and drop every `.await`.

> Until a release ships these features, depend on the branch as above; a release tag is pinned the same way as in [Usage as an External Dependency](#usage-as-an-external-dependency).

### 2. Install what the backend needs

The web backend drives a browser through Playwright and encodes clips with `ffmpeg`:

```sh
npm install playwright
npx playwright install chromium
export RUPICOLA_PLAYWRIGHT_DIR="$PWD/node_modules/playwright"
```

Every backend lists its own requirements in [Capture Backends](#capture-backends-rupicola-capture).

### 3. Describe the capture as a plan

A `Plan` says *what* to capture (`target`), *how it is shown* (`environment`) and *what to do* (`steps`). It does not name a backend, so the same plan can run on any backend that can honour it.

```rust
use rupicola_capture::session::{Environment, Plan, Step, Target, WaitCondition};
use rupicola_protocol::UiAction;

let plan = Plan {
    target: Target {
        // A URL for web backends, an executable for desktop backends.
        command: PAGE.to_owned(),
        args: Vec::new(),
    },
    environment: Environment::new([480, 240]),
    steps: vec![
        Step::Checkpoint { id: "before".to_owned(), full_page: false },
        Step::StartRecording,
        Step::Action { action: UiAction::Click { target_id: "go".to_owned() } },
        Step::Wait { condition: WaitCondition::Text { text: "Done".to_owned() } },
        Step::Wait { condition: WaitCondition::Delay { ms: 500 } },
        Step::Checkpoint { id: "after".to_owned(), full_page: false },
        Step::StopRecording { id: "click".to_owned() },
    ],
    allow_degraded: false,
};
```

- A `Checkpoint` is a screenshot; a clip is everything between `StartRecording` and `StopRecording`. Ids become file names, so they may only hold letters, digits, `-` and `_`.
- On the web backend a `target_id` is an element id (`"go"`), or any [Playwright selector](https://playwright.dev/docs/selectors) when it says so: `"css=button.primary"`, `"text=Save"`, `"//li[2]"`.
- On the web backend an action returns once its effect is on screen: the driver waits for the page to paint a few frames without scrolling, so a `Checkpoint` right after `PressKey`, `Scroll` or `Click` shows the result. `environment.settle_ms` adds a fixed pause after every action for pages that animate for longer.
- `UiAction::Scroll` scrolls by a delta: with the mouse wheel where the pointer is, or, with a `target_id`, the element (or its closest scrollable ancestor) without moving the pointer. `ScrollIntoView` brings an element on screen without moving the pointer, `ScrollTo` goes to an offset and `ScrollToEnd` to the end, of the page or, with a `target_id`, of an element.
- `Checkpoint { full_page: true }` shoots the whole scrollable page instead of the viewport. It needs the `full-page` capability, which only `WebBackend` has.
- `Environment` also sets `scale`, `color_scheme`, `reduced_motion`, `locale`, a per-step `timeout_ms` and the `frame_interval_ms` of recordings; backends ignore what they have no notion of.

A plan is plain data, so it can also be written as JSON and read with `serde_json::from_str::<Plan>`. Anything left out takes its default:

```json
{
  "target": { "command": "https://example.com" },
  "environment": { "viewport": [480, 240], "warmup_ms": 0 },
  "steps": [
    { "step": "checkpoint", "id": "before" },
    { "step": "start-recording" },
    { "step": "action", "action": { "action": "click", "target_id": "go" } },
    { "step": "wait", "condition": { "wait": "text", "text": "Done" } },
    { "step": "checkpoint", "id": "after" },
    { "step": "stop-recording", "id": "click" }
  ]
}
```

### 4. Run it

```rust
use rupicola_capture::asynchronous::{CaptureBackend as _, WebBackend, WebDiscovery as _, run_plan};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), rupicola_capture::session::CaptureError> {
    // Finds Node.js and the Playwright module named by RUPICOLA_PLAYWRIGHT_DIR.
    let Some(backend) = WebBackend::discover("chromium").await else {
        eprintln!("Playwright not found");
        return Ok(());
    };

    let bundle = run_plan(&backend, &plan, Path::new("target/capture-a-page")).await?;
    for checkpoint in &bundle.checkpoints {
        println!("checkpoint {}: {}", checkpoint.id, checkpoint.png.display());
    }
    for clip in &bundle.clips {
        println!("clip {}: {}", clip.id, clip.mp4.display());
    }
    Ok(())
}
```

```console
$ cargo run -p rupicola-capture --example capture_a_page
web-playwright can do ["actions","ui-tree","video","color-scheme","scale"]
checkpoint before: target/capture-a-page/before.png
checkpoint after: target/capture-a-page/after.png
clip click: target/capture-a-page/click.mp4
annotated: target/capture-a-page/before-annotated.png
gif: target/capture-a-page/click.gif
```

`run_plan` opens one session for the whole plan, so the target launches once however many checkpoints are taken. It returns a `Bundle` with the `checkpoints` (each with its PNG and, on backends that have one, a UI tree snapshot), the `clips` and any `warnings`.

A checkpoint keeps a UI tree only when the plan asks: `environment.ui_tree` is `off` (the default: no tree, and a backend does not collect one), `structure` (kinds, bounds and states, with no titles, labels or values) or `full`. A tree lists the text on screen, which can be personal, so `structure` is the mode for an application that shows user data. Every node has a `kind` from one small vocabulary (`button`, `text-box`, `list`, ...) next to the platform's own `role`, and each tree says where it came from (`source`). Bounds are logical: multiply by `scale_factor` to get the pixels of the screenshot taken with the tree.

Before anything starts, `run_plan` compares what the plan asks for with what the backend can do (`backend.capabilities()`). A plan the backend cannot honour is refused with an `Environment` error naming what is missing, unless `allow_degraded` is `true`: then it runs without those steps and records a warning for each. Errors carry a class: `Environment` means the machine could not run the backend (a missing tool, no display); `Target` means the application failed to start or answer.

### 5. Use another backend

Swap the backend and keep the plan's shape. A desktop backend takes an executable as the target and, having no UI tree, reaches things by screen coordinates:

```rust
use rupicola_capture::asynchronous::LinuxX11Backend;
use rupicola_capture::session::{Environment, Plan, Step, Target};
use rupicola_protocol::UiAction;

let backend = LinuxX11Backend { display_num: 95 };
let plan = Plan {
    target: Target { command: "./my-gtk-app".to_owned(), args: Vec::new() },
    environment: Environment::new([640, 480]),
    steps: vec![
        Step::Checkpoint { id: "closed".to_owned(), full_page: false },
        Step::Action { action: UiAction::ClickAt { x: 120.0, y: 40.0 } },
        Step::Checkpoint { id: "open".to_owned(), full_page: false },
    ],
    allow_degraded: false,
};
```

Each backend's `capabilities()` says what it can do; the [table below](#capture-backends-rupicola-capture) lists them.

### 6. Annotate and make a GIF

What a bundle holds can be annotated and turned into a GIF afterwards (both need `ffmpeg`, and return their error as a `String`, which the snippet wraps in a `CaptureError`):

```rust
use rupicola_capture::asynchronous::{annotate_png, mp4_to_gif};
use rupicola_capture::session::CaptureError;
use rupicola_protocol::{AnnotationBox, AnnotationShape, AnnotationStyle, Rect};

let button = AnnotationBox {
    bounds: Rect { x: 8.0, y: 66.0, width: 34.0, height: 22.0 },
    label: "go".to_owned(),
    shape: AnnotationShape::Rect,
    style: AnnotationStyle::default(),
    time_range: None,
    keyframes: Vec::new(),
};
annotate_png(&bundle.checkpoints[0].png, Path::new("before-annotated.png"), &[button], 1.0)
    .await
    .map_err(CaptureError::environment)?;
mp4_to_gif(&bundle.clips[0].mp4, Path::new("click.gif"), 320)
    .await
    .map_err(CaptureError::environment)?;
```

`annotate_video` does the same for a clip, and an annotation can carry a `time_range` and `keyframes` to appear and move over time.

## Workspace Crates

| Crate | Path | Purpose |
| :--- | :--- | :--- |
| **`rupicola-protocol`** | [`crates/rupicola-protocol`](crates/rupicola-protocol) | Zero-bloat wire types (`UiSnapshot`, `WidgetNode`, `UiAction`, `AnnotationBox` with `AnnotationStyle`, `AnnotationShape`, `TimeRange` & `keyframes`), NDJSON codec, and Unix domain socket client/server. |
| **`rupicola-capture`** | [`crates/rupicola-capture`](crates/rupicola-capture) | The capture plan and session contract, headless capture backends (blocking and async), PNG/MP4 visual annotation engine (`ffmpeg drawbox`), and inline GIF generator. |

## Capture Backends (`rupicola-capture`)

| Backend | Target | Requirements | Capabilities |
| :--- | :--- | :--- | :--- |
| `WebBackend` | Web (`chromium` / `firefox` / `webkit`) | Node.js + Playwright, `ffmpeg` for clips | actions on elements, UI tree, video, color scheme, scale, full-page checkpoints |
| `CdpBackend` | Web (no Node/Playwright) | Local Chromium / Chrome / Brave, and `sh` | UI tree, color scheme, scale, waits on elements and text (screenshots always). On Unix it keeps one browser for the session and speaks the Chrome DevTools Protocol over its pipes; elsewhere it runs the browser again for each screenshot and only takes screenshots. Actions on named elements and video are not supported. |
| `LinuxX11Backend` | Linux Desktop (GTK4, Iced, egui, Qt) | `Xvfb`, `ffmpeg`, `python3` with libX11 and libXtst | pointer input, video |
| `GtkBroadwayBackend` | Linux GTK4 (no X11/Wayland) | `gtk4-broadwayd`, `WebBackend` | pointer input, video |
| `LinuxWaylandBackend` | Linux Wayland (nested `cosmic-comp`) | `Xvfb`, the compositor, `ffmpeg`, `python3` with libX11 and libXtst | pointer input, video |
| `AndroidAdbBackend` | Android Emulator / Device | `adb` | UI tree (UI Automator), video (screenshots always) |

Every backend takes screenshots. A capability a plan needs and a backend lacks is checked before anything starts (see [Getting Started](#4-run-it)).

## Usage as an External Dependency

Pin by release tag in your `Cargo.toml`:

```toml
[dependencies]
rupicola-protocol = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", tag = "v0.3.0" }
# Optional: include the capture engine
rupicola-capture = { git = "https://github.com/Lucas-BRT/rupicola-protocol.git", tag = "v0.3.0" }
```

`v0.3.0` predates the `sync` and `async` features described in [Getting Started](#getting-started). Until the next release, depend on the `feat/capture-session` branch to use them.

## Upgrading from 0.3.0 (unreleased)

- Choose how captures run: enable the `sync` or the `async` feature of `rupicola-capture` (there is no default, and a build with neither fails).
- Everything that runs a capture moved under the module of the feature you chose: `rupicola_capture::sync::{run_plan, CaptureBackend, CaptureSession, WebBackend, LinuxX11Backend, annotate_png, ...}` or `rupicola_capture::asynchronous::{...}`. The shared types stay in `rupicola_capture::session` (`Plan`, `Step`, `Environment`, `Target`, `Bundle`, `CaptureError`, ...).
- The one-shot `CaptureBackend::capture(request)` API, `CaptureRequest` and `CaptureOutput` are gone. Describe the capture as a `Plan` and call `run_plan`.
- `WebBackend::discover` and `CdpBackend::discover` are provided by the `WebDiscovery` and `CdpDiscovery` traits of the module you chose, and are `async` in `asynchronous`. `WebBackend` no longer searches a developer's home directory for Playwright: set `RUPICOLA_PLAYWRIGHT_DIR` or build the backend yourself.
- `Environment` has three new optional fields (`locale`, `timeout_ms`, `frame_interval_ms`). Build it with `Environment::new(viewport)` instead of a struct literal.

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
