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
    /// Turns the mouse wheel by `dx` and `dy` pixels, over `target_id` when given.
    Scroll {
        target_id: Option<String>,
        dx: f64,
        dy: f64,
    },
    /// Brings the element on screen, without moving the pointer.
    ScrollIntoView {
        target_id: String,
    },
    /// Scrolls the element, or the page when `target_id` is `None`, to an absolute offset in
    /// pixels.
    ScrollTo {
        target_id: Option<String>,
        x: f64,
        y: f64,
    },
    /// Scrolls the element, or the page when `target_id` is `None`, down to its end.
    ScrollToEnd {
        target_id: Option<String>,
    },
    WaitMs {
        ms: u64,
    },
}
