use serde::{Deserialize, Serialize};

/// Action dispatched to the target application through the middleware adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum UiAction {
    Click {
        target_id: String,
    },
    ClickAt {
        x: f64,
        y: f64,
    },
    Hover {
        target_id: String,
    },
    Focus {
        target_id: String,
    },
    TypeText {
        target_id: String,
        text: String,
    },
    PressKey {
        key: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    Scroll {
        target_id: Option<String>,
        dx: f64,
        dy: f64,
    },
    WaitMs {
        ms: u64,
    },
}
