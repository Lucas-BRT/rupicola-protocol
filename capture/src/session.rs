//! Session-based capture contract.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once; the session then
//! performs actions, waits, takes screenshots and records clips for as long as the plan
//! needs, so the target launches once however many checkpoints are taken.
//! [`run_plan`] drives a session from a declarative [`Plan`], refusing up front a plan the
//! backend cannot honour unless the plan allows a degraded run.

use rupicola_protocol::{UiAction, UiSnapshot};
use serde::{Deserialize, Serialize};
use std::{fmt, path::Path, path::PathBuf};

/// Pixel ratio used when a plan does not set one.
const DEFAULT_SCALE: f64 = 1.0;
/// Milliseconds a target gets to settle after launch unless the plan says otherwise.
const DEFAULT_WARMUP_MS: u64 = 1200;

const fn default_scale() -> f64 {
    DEFAULT_SCALE
}

const fn default_warmup_ms() -> u64 {
    DEFAULT_WARMUP_MS
}

/// Whether `id` is safe as a file name: ASCII letters, digits, `-` and `_`.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// `id` if it is a safe file name.
///
/// # Errors
///
/// An [`ErrorClass::Environment`] error naming the bad id; plans are checked with this
/// before anything starts.
pub fn checked_id(id: &str) -> Result<&str, CaptureError> {
    if valid_id(id) {
        Ok(id)
    } else {
        Err(CaptureError::environment(format!(
            "`{id}` is not a valid id: use letters, digits, - and _"
        )))
    }
}

/// What is captured: an executable (desktop backends) or a URL (web backends).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Target {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorScheme {
    Light,
    Dark,
}

/// How the target is shown.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Environment {
    pub viewport: [u32; 2],
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default)]
    pub color_scheme: Option<ColorScheme>,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default = "default_warmup_ms")]
    pub warmup_ms: u64,
}

impl Environment {
    /// A default environment with the given viewport.
    #[must_use]
    pub const fn new(viewport: [u32; 2]) -> Self {
        Self {
            viewport,
            scale: DEFAULT_SCALE,
            color_scheme: None,
            reduced_motion: false,
            warmup_ms: DEFAULT_WARMUP_MS,
        }
    }
}

/// One thing a backend may or may not be able to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    /// Performs [`UiAction`]s.
    Actions,
    /// Reports a UI tree with each screenshot.
    UiTree,
    /// Records walkthrough clips.
    Video,
    /// Honours the environment's color scheme.
    ColorScheme,
    /// Honours the environment's scale.
    Scale,
}

impl Capability {
    const fn bit(self) -> u8 {
        1 << self as u8
    }

    /// Name used in messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Actions => "actions",
            Self::UiTree => "ui-tree",
            Self::Video => "video",
            Self::ColorScheme => "color-scheme",
            Self::Scale => "scale",
        }
    }

    const ALL: [Self; 5] = [
        Self::Actions,
        Self::UiTree,
        Self::Video,
        Self::ColorScheme,
        Self::Scale,
    ];
}

/// What a backend can do, or what a plan needs; checked before anything starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities(u8);

impl Capabilities {
    /// A set holding exactly `items`.
    #[must_use]
    pub const fn of(items: &[Capability]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < items.len() {
            bits |= items[index].bit();
            index += 1;
        }
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, item: Capability) -> bool {
        self.0 & item.bit() != 0
    }

    /// `self` with `item` added.
    #[must_use]
    pub const fn with(self, item: Capability) -> Self {
        Self(self.0 | item.bit())
    }

    /// The capabilities in `needed` that `self` lacks.
    #[must_use]
    pub fn missing(self, needed: Self) -> Vec<Capability> {
        Capability::ALL
            .into_iter()
            .filter(|item| needed.contains(*item) && !self.contains(*item))
            .collect()
    }
}

impl Serialize for Capabilities {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(
            Capability::ALL
                .into_iter()
                .filter(|item| self.contains(*item)),
        )
    }
}

/// Who is to blame for a failure, so callers can tell a broken machine from a broken app.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorClass {
    /// Missing tools, display servers or browsers; the backend could not run.
    Environment,
    /// The target failed to launch, crashed, or did not answer.
    Target,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureError {
    pub class: ErrorClass,
    pub message: String,
}

impl CaptureError {
    #[must_use]
    pub fn environment(message: impl Into<String>) -> Self {
        Self {
            class: ErrorClass::Environment,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn target(message: impl Into<String>) -> Self {
        Self {
            class: ErrorClass::Target,
            message: message.into(),
        }
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let class = match self.class {
            ErrorClass::Environment => "environment",
            ErrorClass::Target => "target",
        };
        write!(f, "{class} error: {}", self.message)
    }
}

impl std::error::Error for CaptureError {}

/// A condition a step waits for.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "wait", rename_all = "kebab-case")]
pub enum WaitCondition {
    /// A fixed pause.
    Delay { ms: u64 },
    /// An element is on screen.
    Visible { target_id: String },
    /// Some text is on screen.
    Text { text: String },
}

/// One screenshot, with the UI tree when the backend has one.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Checkpoint {
    pub id: String,
    pub png: PathBuf,
    pub snapshot: Option<UiSnapshot>,
}

/// One recorded walkthrough.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Clip {
    pub id: String,
    pub mp4: PathBuf,
}

/// A running capture of one target.
pub trait CaptureSession {
    /// # Errors
    ///
    /// Fails when the target does not accept the action.
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the condition is not met in time.
    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the frame cannot be grabbed.
    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError>;

    /// # Errors
    ///
    /// Fails when recording cannot start.
    fn start_recording(&mut self) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when no recording is running or it cannot be finished.
    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError>;

    /// Stops the target and everything started for it. Called exactly once, even after
    /// a failed step.
    ///
    /// # Errors
    ///
    /// Fails when cleanup does not complete.
    fn finish(self: Box<Self>) -> Result<(), CaptureError>;
}

/// A way of capturing: knows what it can do and how to start a [`CaptureSession`].
pub trait CaptureBackend {
    fn id(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    /// Launches `target` in `environment`; files go under `out`.
    ///
    /// # Errors
    ///
    /// [`ErrorClass::Environment`] when tools are missing, [`ErrorClass::Target`] when
    /// the target does not come up.
    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError>;
}

/// One step of a [`Plan`].
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub enum Step {
    Action { action: UiAction },
    Wait { condition: WaitCondition },
    Checkpoint { id: String },
    StartRecording,
    StopRecording { id: String },
}

impl Step {
    /// Capabilities this step needs beyond a screenshot.
    #[must_use]
    pub const fn needs(&self) -> Capabilities {
        match self {
            Self::Action { .. } => Capabilities::of(&[Capability::Actions]),
            Self::StartRecording | Self::StopRecording { .. } => {
                Capabilities::of(&[Capability::Video])
            }
            Self::Wait { .. } | Self::Checkpoint { .. } => Capabilities::of(&[]),
        }
    }
}

/// A declarative capture: where, how it is shown, and what to do.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Plan {
    pub target: Target,
    pub environment: Environment,
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Run with what the backend can do, recording a warning for each skipped request,
    /// instead of refusing the plan.
    #[serde(default)]
    pub allow_degraded: bool,
}

impl Plan {
    /// Everything the plan asks of a backend.
    #[must_use]
    pub fn needs(&self) -> Capabilities {
        let mut needed = Capabilities::default();
        if self.environment.color_scheme.is_some() {
            needed = needed.with(Capability::ColorScheme);
        }
        if (self.environment.scale - DEFAULT_SCALE).abs() > f64::EPSILON {
            needed = needed.with(Capability::Scale);
        }
        for step in &self.steps {
            needed = Capabilities(needed.0 | step.needs().0);
        }
        needed
    }
}

/// What a plan produced.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Bundle {
    pub checkpoints: Vec<Checkpoint>,
    pub clips: Vec<Clip>,
    /// Requests skipped because the plan allowed a degraded run.
    pub warnings: Vec<String>,
}

/// Runs `plan` on `backend`, writing files under `out`.
///
/// # Errors
///
/// An [`ErrorClass::Environment`] error before anything starts when an id is not a safe
/// file name, or when the backend lacks a capability the plan needs and the plan is not degradable; otherwise the first failing
/// step's error, after the session has been finished.
pub fn run_plan(
    backend: &dyn CaptureBackend,
    plan: &Plan,
    out: &Path,
) -> Result<Bundle, CaptureError> {
    for step in &plan.steps {
        if let Step::Checkpoint { id } | Step::StopRecording { id } = step {
            checked_id(id)?;
        }
    }
    let missing = backend.capabilities().missing(plan.needs());
    if !missing.is_empty() && !plan.allow_degraded {
        return Err(CaptureError::environment(format!(
            "backend {} lacks: {}",
            backend.id(),
            missing
                .iter()
                .map(|item| item.name())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let mut bundle = Bundle::default();
    for item in &missing {
        bundle.warnings.push(format!(
            "backend {} lacks {}; skipped",
            backend.id(),
            item.name()
        ));
    }
    let mut session = backend.start(&plan.target, &plan.environment, out)?;
    let outcome = play(session.as_mut(), plan, &missing, &mut bundle);
    let finished = session.finish();
    outcome?;
    finished?;
    Ok(bundle)
}

/// Runs the plan's steps on `session`, skipping those the backend lacks the capability for.
fn play(
    session: &mut dyn CaptureSession,
    plan: &Plan,
    missing: &[Capability],
    bundle: &mut Bundle,
) -> Result<(), CaptureError> {
    let skipped = |step: &Step| missing.iter().any(|item| step.needs().contains(*item));
    for step in &plan.steps {
        if skipped(step) {
            continue;
        }
        match step {
            Step::Action { action } => session.perform(action)?,
            Step::Wait { condition } => session.wait_for(condition)?,
            Step::Checkpoint { id } => bundle.checkpoints.push(session.screenshot(id)?),
            Step::StartRecording => session.start_recording()?,
            Step::StopRecording { id } => bundle.clips.push(session.stop_recording(id)?),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    /// Records every call so tests can check order and cleanup.
    #[derive(Default)]
    struct Journal(Rc<RefCell<Vec<String>>>);

    struct Fake {
        capabilities: Capabilities,
        journal: Rc<RefCell<Vec<String>>>,
        fail_on: Option<&'static str>,
    }

    struct FakeSession {
        journal: Rc<RefCell<Vec<String>>>,
        fail_on: Option<&'static str>,
    }

    impl FakeSession {
        fn note(&self, entry: String) -> Result<(), CaptureError> {
            let fails = self.fail_on.is_some_and(|prefix| entry.starts_with(prefix));
            self.journal.borrow_mut().push(entry);
            if fails {
                Err(CaptureError::target("boom"))
            } else {
                Ok(())
            }
        }
    }

    impl CaptureSession for FakeSession {
        fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            self.note(format!("perform {action:?}"))
        }
        fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            self.note(format!("wait {condition:?}"))
        }
        fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            self.note(format!("screenshot {id}"))?;
            Ok(Checkpoint {
                id: id.to_owned(),
                png: PathBuf::from(format!("{id}.png")),
                snapshot: None,
            })
        }
        fn start_recording(&mut self) -> Result<(), CaptureError> {
            self.note("start_recording".to_owned())
        }
        fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
            self.note(format!("stop_recording {id}"))?;
            Ok(Clip {
                id: id.to_owned(),
                mp4: PathBuf::from(format!("{id}.mp4")),
            })
        }
        fn finish(self: Box<Self>) -> Result<(), CaptureError> {
            self.note("finish".to_owned())
        }
    }

    impl CaptureBackend for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn capabilities(&self) -> Capabilities {
            self.capabilities
        }
        fn start(
            &self,
            _: &Target,
            _: &Environment,
            _: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            self.journal.borrow_mut().push("start".to_owned());
            Ok(Box::new(FakeSession {
                journal: self.journal.clone(),
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
            journal: journal.0.clone(),
            fail_on,
        };
        (backend, journal)
    }

    #[test]
    fn one_session_serves_every_checkpoint() {
        let (backend, journal) = fake(FULL, None);
        let bundle = run_plan(&backend, &plan(flow(), false), Path::new("out")).unwrap();
        assert_eq!(bundle.checkpoints.len(), 2);
        assert_eq!(bundle.clips.len(), 1);
        let calls = journal.0.borrow();
        assert_eq!(calls.iter().filter(|call| *call == "start").count(), 1);
        assert_eq!(calls.last().map(String::as_str), Some("finish"));
    }

    #[test]
    fn a_plan_the_backend_cannot_honour_is_refused_before_starting() {
        let (backend, journal) = fake(SCREENSHOT_ONLY, None);
        let error = run_plan(&backend, &plan(flow(), false), Path::new("out")).unwrap_err();
        assert_eq!(error.class, ErrorClass::Environment);
        assert!(error.message.contains("actions") && error.message.contains("video"));
        assert!(journal.0.borrow().is_empty());
    }

    #[test]
    fn a_degraded_plan_skips_what_is_missing_and_says_so() {
        let (backend, _) = fake(SCREENSHOT_ONLY, None);
        let bundle = run_plan(&backend, &plan(flow(), true), Path::new("out")).unwrap();
        assert_eq!(bundle.checkpoints.len(), 2);
        assert_eq!(bundle.clips.len(), 0);
        assert_eq!(bundle.warnings.len(), 2);
    }

    #[test]
    fn the_session_is_finished_after_a_failed_step() {
        let (backend, journal) = fake(FULL, Some("perform"));
        let error = run_plan(&backend, &plan(flow(), false), Path::new("out")).unwrap_err();
        assert_eq!(error.class, ErrorClass::Target);
        assert_eq!(
            journal.0.borrow().last().map(String::as_str),
            Some("finish")
        );
    }

    #[test]
    fn plans_round_trip_through_json() {
        let original = plan(flow(), true);
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), original);
    }
}
