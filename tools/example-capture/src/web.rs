//! Capture the project workspace served at the supplied URL.
use crate::media::Walkthrough;
use rupicola_capture::session::{Environment, Target};
use rupicola_capture::sync::{WebBackend, WebDiscovery as _};
use rupicola_protocol::UiAction;

const VIEWPORT: [u32; 2] = [1280, 720];
const WARMUP_MS: u64 = 500;
const HOLD_MS: u64 = 2000;
const STEP_PAUSE_MS: u64 = 500;
const NEW_PROJECT_PAUSE_MS: u64 = 800;

pub fn capture(framework: &str, url: Option<&str>) -> Result<(), String> {
    let backend = WebBackend::discover("chromium").ok_or("Playwright not found")?;
    let app_dir = crate::root()
        .join("examples/apps/advanced/project-workspace")
        .join(framework);
    let (url, server) = match url {
        Some(url) => (url.to_owned(), None),
        None if framework == "html" => {
            let (url, stop) = crate::support::serve_dir(app_dir.clone());
            (url, Some(stop))
        }
        None => return Err("provide the URL of the running React app".into()),
    };
    let mut environment = Environment::new(VIEWPORT);
    environment.warmup_ms = WARMUP_MS;
    let result = Walkthrough {
        target: Target {
            command: url,
            args: vec![],
        },
        environment,
        actions: vec![
            UiAction::Click {
                target_id: "[data-page=projects]".into(),
            },
            UiAction::WaitMs { ms: STEP_PAUSE_MS },
            UiAction::Click {
                target_id: "#new-project".into(),
            },
            UiAction::WaitMs {
                ms: NEW_PROJECT_PAUSE_MS,
            },
        ],
        hold_ms: HOLD_MS,
        annotations: vec![],
        out: app_dir.join("media"),
    }
    .record(&backend);
    if let Some(stop) = server {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    result
}
