use super::*;
use crate::session::{Capability, ErrorClass, Plan, Step};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

type Calls = Arc<Mutex<Vec<String>>>;

/// Records every call so tests can check order and cleanup.
#[derive(Default)]
struct Journal(Calls);

struct Fake {
    capabilities: Capabilities,
    journal: Calls,
    fail_on: Option<&'static str>,
}

struct FakeSession {
    journal: Calls,
    fail_on: Option<&'static str>,
}

impl FakeSession {
    fn note(&self, entry: String) -> Result<(), CaptureError> {
        let fails = self.fail_on.is_some_and(|prefix| entry.starts_with(prefix));
        self.journal.lock().unwrap().push(entry);
        if fails {
            Err(CaptureError::target("boom"))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl CaptureSession for FakeSession {
    async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        self.note(format!("perform {action:?}"))
    }
    async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        self.note(format!("wait {condition:?}"))
    }
    async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        self.note(format!("screenshot {id}"))?;
        Ok(Checkpoint {
            id: id.to_owned(),
            png: PathBuf::from(format!("{id}.png")),
            snapshot: None,
        })
    }
    async fn start_recording(&mut self) -> Result<(), CaptureError> {
        self.note("start_recording".to_owned())
    }
    async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        self.note(format!("stop_recording {id}"))?;
        Ok(Clip {
            id: id.to_owned(),
            mp4: PathBuf::from(format!("{id}.mp4")),
        })
    }
    async fn finish(self: Box<Self>) -> Result<(), CaptureError> {
        self.note("finish".to_owned())
    }
}

#[async_trait]
impl CaptureBackend for Fake {
    fn id(&self) -> &'static str {
        "fake"
    }
    fn capabilities(&self) -> Capabilities {
        self.capabilities
    }
    async fn start(
        &self,
        _: &Target,
        _: &Environment,
        _: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError> {
        self.journal.lock().unwrap().push("start".to_owned());
        Ok(Box::new(FakeSession {
            journal: Arc::clone(&self.journal),
            fail_on: self.fail_on,
        }))
    }
}

const FULL: Capabilities = Capabilities::of(&Capability::ALL);
const SCREENSHOT_ONLY: Capabilities = Capabilities::of(&[]);
const VIEWPORT: [u32; 2] = [800, 600];

fn plan(steps: Vec<Step>, allow_degraded: bool) -> Plan {
    Plan {
        target: Target {
            command: "app".to_owned(),
            args: Vec::new(),
        },
        environment: Environment::new(VIEWPORT),
        steps,
        allow_degraded,
    }
}

fn flow() -> Vec<Step> {
    vec![
        Step::Checkpoint {
            id: "before".to_owned(),
        },
        Step::StartRecording,
        Step::Action {
            action: UiAction::Click {
                target_id: "go".to_owned(),
            },
        },
        Step::Checkpoint {
            id: "after".to_owned(),
        },
        Step::StopRecording {
            id: "walk".to_owned(),
        },
    ]
}

fn fake(capabilities: Capabilities, fail_on: Option<&'static str>) -> (Fake, Journal) {
    let journal = Journal::default();
    let backend = Fake {
        capabilities,
        journal: Arc::clone(&journal.0),
        fail_on,
    };
    (backend, journal)
}

#[tokio::test]
async fn one_session_serves_every_checkpoint() {
    let (backend, journal) = fake(FULL, None);
    let bundle = run_plan(&backend, &plan(flow(), false), Path::new("out"))
        .await
        .unwrap();
    assert_eq!(bundle.checkpoints.len(), 2);
    assert_eq!(bundle.clips.len(), 1);
    let calls = journal.0.lock().unwrap();
    assert_eq!(calls.iter().filter(|call| *call == "start").count(), 1);
    assert_eq!(calls.last().map(String::as_str), Some("finish"));
}

#[tokio::test]
async fn a_plan_the_backend_cannot_honour_is_refused_before_starting() {
    let (backend, journal) = fake(SCREENSHOT_ONLY, None);
    let error = run_plan(&backend, &plan(flow(), false), Path::new("out"))
        .await
        .unwrap_err();
    assert_eq!(error.class, ErrorClass::Environment);
    assert!(error.message.contains("actions") && error.message.contains("video"));
    assert!(journal.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_degraded_plan_skips_what_is_missing_and_says_so() {
    let (backend, _) = fake(SCREENSHOT_ONLY, None);
    let bundle = run_plan(&backend, &plan(flow(), true), Path::new("out"))
        .await
        .unwrap();
    assert_eq!(bundle.checkpoints.len(), 2);
    assert_eq!(bundle.clips.len(), 0);
    assert_eq!(bundle.warnings.len(), 2);
}

#[tokio::test]
async fn the_session_is_finished_after_a_failed_step() {
    let (backend, journal) = fake(FULL, Some("perform"));
    let error = run_plan(&backend, &plan(flow(), false), Path::new("out"))
        .await
        .unwrap_err();
    assert_eq!(error.class, ErrorClass::Target);
    assert_eq!(
        journal.0.lock().unwrap().last().map(String::as_str),
        Some("finish")
    );
}

#[test]
fn coordinate_actions_need_only_the_pointer() {
    let click = Step::Action {
        action: UiAction::ClickAt { x: 1.0, y: 2.0 },
    };
    let named = Step::Action {
        action: UiAction::Click {
            target_id: "go".to_owned(),
        },
    };
    let pointer = Capabilities::of(&[Capability::Pointer]);
    assert_eq!(pointer.missing(click.needs()), []);
    assert_eq!(pointer.missing(named.needs()), [Capability::Actions]);
    assert_eq!(FULL.missing(click.needs()), []);
    let full_actions = Capabilities::of(&[Capability::Actions]);
    assert_eq!(full_actions.missing(click.needs()), []);
}

#[test]
fn plans_round_trip_through_json() {
    let original = plan(flow(), true);
    let json = serde_json::to_string(&original).unwrap();
    assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), original);
}
