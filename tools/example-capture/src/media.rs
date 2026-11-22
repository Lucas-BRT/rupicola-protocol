//! Runs a capture plan and turns its screenshot and clip into the example media:
//! `preview.png`, `walkthrough.gif` and, when there are annotations, their annotated twins.

use rupicola_capture::session::{
    CaptureBackend, Environment, Plan, Step, Target, WaitCondition, run_plan,
};
use rupicola_capture::{annotate_png, annotate_video, mp4_to_gif};
use rupicola_protocol::{AnnotationBox, UiAction};
use std::fs;
use std::path::PathBuf;

const PREVIEW_ID: &str = "preview";
const WALKTHROUGH_ID: &str = "walkthrough";
const ANNOTATED_PREVIEW: &str = "preview-annotated.png";
const WALKTHROUGH_GIF: &str = "walkthrough.gif";
const ANNOTATED_WALKTHROUGH_MP4: &str = "walkthrough-annotated.mp4";
const ANNOTATED_WALKTHROUGH_GIF: &str = "walkthrough-annotated.gif";
/// Pixel ratio the annotation coordinates are written in.
const ANNOTATION_SCALE: f64 = 1.0;
/// Widest GIF, in pixels.
const GIF_MAX_WIDTH: u32 = 640;

/// One example capture: what to run, what to do, and where the media goes.
pub struct Walkthrough {
    pub target: Target,
    pub environment: Environment,
    pub actions: Vec<UiAction>,
    /// Pause after the last action, so the clip ends on a settled screen.
    pub hold_ms: u64,
    pub annotations: Vec<AnnotationBox>,
    pub out: PathBuf,
}

impl Walkthrough {
    fn plan(&self) -> Plan {
        let mut steps = vec![Step::StartRecording];
        steps.extend(self.actions.iter().map(|action| Step::Action {
            action: action.clone(),
        }));
        steps.push(Step::Wait {
            condition: WaitCondition::Delay { ms: self.hold_ms },
        });
        steps.push(Step::Checkpoint {
            id: PREVIEW_ID.to_owned(),
        });
        steps.push(Step::StopRecording {
            id: WALKTHROUGH_ID.to_owned(),
        });
        Plan {
            target: self.target.clone(),
            environment: self.environment.clone(),
            steps,
            allow_degraded: false,
        }
    }

    /// Captures with `backend` and writes the media under [`Self::out`], leaving no MP4.
    ///
    /// # Errors
    ///
    /// The capture error, or the ffmpeg step that failed.
    pub fn record(&self, backend: &dyn CaptureBackend) -> Result<(), String> {
        let bundle = run_plan(backend, &self.plan(), &self.out).map_err(|e| e.to_string())?;
        let (Some(preview), Some(clip)) = (bundle.checkpoints.first(), bundle.clips.first()) else {
            return Err("the capture produced no screenshot or clip".to_owned());
        };

        let gif = self.out.join(WALKTHROUGH_GIF);
        mp4_to_gif(&clip.mp4, &gif, GIF_MAX_WIDTH)?;
        let mut intermediates = vec![clip.mp4.clone()];
        if !self.annotations.is_empty() {
            annotate_png(
                &preview.png,
                &self.out.join(ANNOTATED_PREVIEW),
                &self.annotations,
                ANNOTATION_SCALE,
            )?;
            let annotated_mp4 = self.out.join(ANNOTATED_WALKTHROUGH_MP4);
            annotate_video(
                &clip.mp4,
                &annotated_mp4,
                &self.annotations,
                ANNOTATION_SCALE,
            )?;
            mp4_to_gif(
                &annotated_mp4,
                &self.out.join(ANNOTATED_WALKTHROUGH_GIF),
                GIF_MAX_WIDTH,
            )?;
            intermediates.push(annotated_mp4);
        }
        for mp4 in intermediates {
            fs::remove_file(&mp4).map_err(|e| format!("remove {}: {e}", mp4.display()))?;
        }
        Ok(())
    }
}
