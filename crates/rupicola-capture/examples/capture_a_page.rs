//! Captures a small page twice, records the click in between, then annotates the first
//! screenshot and turns the clip into a GIF.
//!
//! ```sh
//! RUPICOLA_PLAYWRIGHT_DIR=/path/to/node_modules/playwright \
//!     cargo run -p rupicola-capture --example capture_a_page
//! ```

use rupicola_capture::asynchronous::{
    CaptureBackend as _, WebBackend, WebDiscovery as _, annotate_png, mp4_to_gif, run_plan,
};
use rupicola_capture::session::{CaptureError, Environment, Plan, Step, Target, WaitCondition};
use rupicola_protocol::{AnnotationBox, AnnotationShape, AnnotationStyle, Rect, UiAction};
use std::path::Path;

/// A page with one button that changes its heading.
const PAGE: &str = "data:text/html,<h1 id=heading>Hello</h1>\
    <button id=go onclick=\"heading.textContent='Done'\">Go</button>";
const VIEWPORT: [u32; 2] = [480, 240];
const OUT: &str = "target/capture-a-page";
/// How long the clip keeps recording after the click, in milliseconds.
const CLIP_MS: u64 = 500;
/// Widest GIF, in pixels.
const GIF_WIDTH: u32 = 320;

#[tokio::main]
async fn main() -> Result<(), CaptureError> {
    // The web backend needs Node.js and a Playwright module.
    let Some(backend) = WebBackend::discover("chromium").await else {
        eprintln!("Playwright not found: set RUPICOLA_PLAYWRIGHT_DIR to its module directory");
        return Ok(());
    };

    let plan = Plan {
        target: Target {
            command: PAGE.to_owned(),
            args: Vec::new(),
        },
        environment: Environment::new(VIEWPORT),
        steps: vec![
            Step::Checkpoint {
                id: "before".to_owned(),
            },
            Step::StartRecording,
            Step::Action {
                action: UiAction::Click {
                    target_id: "go".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Text {
                    text: "Done".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Delay { ms: CLIP_MS },
            },
            Step::Checkpoint {
                id: "after".to_owned(),
            },
            Step::StopRecording {
                id: "click".to_owned(),
            },
        ],
        allow_degraded: false,
    };

    println!(
        "{} can do {}",
        backend.id(),
        serde_json::to_string(&backend.capabilities()).unwrap_or_default()
    );
    let bundle = run_plan(&backend, &plan, Path::new(OUT)).await?;
    for checkpoint in &bundle.checkpoints {
        println!("checkpoint {}: {}", checkpoint.id, checkpoint.png.display());
    }
    for clip in &bundle.clips {
        println!("clip {}: {}", clip.id, clip.mp4.display());
    }

    // What the bundle holds can be annotated and turned into a GIF afterwards.
    let button = AnnotationBox {
        bounds: Rect {
            x: 8.0,
            y: 66.0,
            width: 34.0,
            height: 22.0,
        },
        label: "go".to_owned(),
        shape: AnnotationShape::Rect,
        style: AnnotationStyle::default(),
        time_range: None,
        keyframes: Vec::new(),
    };
    let annotated = Path::new(OUT).join("before-annotated.png");
    annotate_png(&bundle.checkpoints[0].png, &annotated, &[button], 1.0)
        .await
        .map_err(CaptureError::environment)?;
    let gif = Path::new(OUT).join("click.gif");
    mp4_to_gif(&bundle.clips[0].mp4, &gif, GIF_WIDTH)
        .await
        .map_err(CaptureError::environment)?;
    println!("annotated: {}\ngif: {}", annotated.display(), gif.display());
    Ok(())
}
