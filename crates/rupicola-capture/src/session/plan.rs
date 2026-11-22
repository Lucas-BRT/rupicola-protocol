use super::{
    Capabilities, Capability, Checkpoint, Clip, DEFAULT_SCALE, Environment, Target, WaitCondition,
};
use rupicola_protocol::UiAction;
use serde::{Deserialize, Serialize};

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

/// What a plan produced.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Bundle {
    pub checkpoints: Vec<Checkpoint>,
    pub clips: Vec<Clip>,
    /// Requests skipped because the plan allowed a degraded run.
    pub warnings: Vec<String>,
}
