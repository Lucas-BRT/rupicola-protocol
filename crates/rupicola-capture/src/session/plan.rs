use super::{
    Capabilities, Capability, Checkpoint, Clip, DEFAULT_SCALE, Environment, Target, WaitCondition,
};
use super::{CaptureError, checked_id};
use rupicola_protocol::UiAction;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    /// Whether the backend lacks something this step needs.
    #[must_use]
    pub fn is_skipped(&self, missing: &[Capability]) -> bool {
        missing.iter().any(|item| self.needs().contains(*item))
    }

    /// Capabilities this step needs beyond a screenshot.
    #[must_use]
    pub const fn needs(&self) -> Capabilities {
        match self {
            Self::Action {
                action:
                    UiAction::ClickAt { .. }
                    | UiAction::Scroll {
                        target_id: None, ..
                    }
                    | UiAction::WaitMs { .. },
            } => Capabilities::of(&[Capability::Pointer]),
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
            needed = needed.union(step.needs());
        }
        needed
    }
}

/// What running a plan on a backend will leave out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preflight {
    /// What the plan asks for and the backend cannot do.
    pub missing: Vec<Capability>,
    /// One message per skipped request, for the bundle.
    pub warnings: Vec<String>,
}

impl Plan {
    /// Checks the plan against what a backend can do, before anything starts.
    ///
    /// # Errors
    ///
    /// An [`super::ErrorClass::Environment`] error when an id is not a safe file name, an
    /// id repeats, or the backend lacks a capability the plan needs and the plan is not
    /// degradable.
    pub fn preflight(
        &self,
        backend: &str,
        capabilities: Capabilities,
    ) -> Result<Preflight, CaptureError> {
        let mut checkpoints = HashSet::new();
        let mut recordings = HashSet::new();
        for step in &self.steps {
            let (id, ids) = match step {
                Step::Checkpoint { id } => (id, &mut checkpoints),
                Step::StopRecording { id } => (id, &mut recordings),
                _ => continue,
            };
            checked_id(id)?;
            if !ids.insert(id) {
                return Err(CaptureError::environment(format!(
                    "duplicate artifact id: {id}"
                )));
            }
        }
        let missing = capabilities.missing(self.needs());
        if !missing.is_empty() && !self.allow_degraded {
            return Err(CaptureError::environment(format!(
                "backend {backend} lacks: {}",
                missing
                    .iter()
                    .map(|item| item.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let warnings = missing
            .iter()
            .map(|item| format!("backend {backend} lacks {}; skipped", item.name()))
            .collect();
        Ok(Preflight { missing, warnings })
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
