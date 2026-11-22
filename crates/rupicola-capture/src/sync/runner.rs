use super::{CaptureBackend, CaptureSession};
use crate::session::{Bundle, Capability, CaptureError, Plan, Step, checked_id};
use std::{collections::HashSet, path::Path};

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
    let mut checkpoints = HashSet::new();
    let mut recordings = HashSet::new();
    for step in &plan.steps {
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
