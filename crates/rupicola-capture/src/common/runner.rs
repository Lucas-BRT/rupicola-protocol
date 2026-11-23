//! `run_plan`, written once and generated for each flavour.

#[maybe_async_cfg::maybe(
    idents(flavour(sync = "sync", async = "asynchronous")),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::session::{Bundle, Capability, CaptureError, Plan, Step};
    use std::path::Path;

    /// Runs `plan` on `backend`, writing files under `out`.
    ///
    /// # Errors
    ///
    /// An [`crate::session::ErrorClass::Environment`] error before anything starts, from
    /// [`Plan::preflight`]; otherwise the first failing step's error, after the session has
    /// been finished.
    pub async fn run_plan(
        backend: &dyn CaptureBackend,
        plan: &Plan,
        out: &Path,
    ) -> Result<Bundle, CaptureError> {
        let preflight = plan.preflight(backend.id(), backend.capabilities())?;
        let mut bundle = Bundle {
            warnings: preflight.warnings,
            ..Bundle::default()
        };
        let mut session = backend.start(&plan.target, &plan.environment, out).await?;
        let outcome = play(session.as_mut(), plan, &preflight.missing, &mut bundle).await;
        let finished = session.finish().await;
        outcome?;
        finished?;
        Ok(bundle)
    }

    /// Runs the plan's steps on `session`, skipping those the backend lacks the capability for.
    async fn play(
        session: &mut dyn CaptureSession,
        plan: &Plan,
        missing: &[Capability],
        bundle: &mut Bundle,
    ) -> Result<(), CaptureError> {
        for step in plan.steps.iter().filter(|step| !step.is_skipped(missing)) {
            match step {
                Step::Action { action } => session.perform(action).await?,
                Step::Wait { condition } => session.wait_for(condition).await?,
                Step::Checkpoint { id } => bundle.checkpoints.push(session.screenshot(id).await?),
                Step::StartRecording => session.start_recording().await?,
                Step::StopRecording { id } => bundle.clips.push(session.stop_recording(id).await?),
            }
        }
        Ok(())
    }
}
