//! The plan the README shows as JSON must stay readable as one.

use rupicola_capture::session::{Plan, Step};

const README_PLAN: &str = r#"{
  "target": { "command": "https://example.com" },
  "environment": { "viewport": [480, 240], "warmup_ms": 0 },
  "steps": [
    { "step": "checkpoint", "id": "before" },
    { "step": "start-recording" },
    { "step": "action", "action": { "action": "click", "target_id": "go" } },
    { "step": "wait", "condition": { "wait": "text", "text": "Done" } },
    { "step": "checkpoint", "id": "after" },
    { "step": "stop-recording", "id": "click" }
  ]
}"#;

const STEPS: usize = 6;

#[test]
fn the_json_plan_in_the_readme_parses_with_defaults_for_what_it_omits() {
    let plan: Plan = serde_json::from_str(README_PLAN).unwrap();
    assert_eq!(plan.steps.len(), STEPS);
    assert_eq!(plan.steps[1], Step::StartRecording);
    assert!(!plan.allow_degraded);
    assert_eq!(plan.target.args, Vec::<String>::new());
}
