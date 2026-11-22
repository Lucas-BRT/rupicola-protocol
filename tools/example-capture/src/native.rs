//! Capture a built native workspace using Xvfb and `FFmpeg`.
use crate::media::Walkthrough;
use rupicola_capture::LinuxX11Backend;
use rupicola_capture::session::{Environment, Target};

const DISPLAY: u16 = 98;
const VIEWPORT: [u32; 2] = [1024, 768];
const WARMUP_MS: u64 = 2500;
const HOLD_MS: u64 = 1500;

pub fn capture(framework: &str, command: &str) -> Result<(), String> {
    let mut environment = Environment::new(VIEWPORT);
    environment.warmup_ms = WARMUP_MS;
    Walkthrough {
        target: Target {
            command: command.into(),
            args: vec![],
        },
        environment,
        actions: vec![],
        hold_ms: HOLD_MS,
        annotations: vec![],
        out: crate::root()
            .join("examples/apps/advanced/project-workspace")
            .join(framework)
            .join("media"),
    }
    .record(&LinuxX11Backend {
        display_num: DISPLAY,
    })
}
