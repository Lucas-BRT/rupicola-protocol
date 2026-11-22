use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

/// UI toolkit or rendering middleware running inside the target application.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToolkitKind {
    WebDom,
    Iced,
    Cosmic,
    Egui,
    Gtk4,
    Tauri,
    Qt,
    Other(String),
}

/// Interactive state flags reported by a widget or DOM node.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire flags representing independent widget states"
)]
pub struct WidgetState {
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub hovered: bool,
    #[serde(default)]
    pub pressed: bool,
    #[serde(default)]
    pub checked: Option<bool>,
    #[serde(default)]
    pub selected: Option<bool>,
}

/// Node in a captured UI tree.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WidgetNode {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    pub bounds: Rect,
    #[serde(default)]
    pub state: WidgetState,
    #[serde(default)]
    pub style_hint: Option<String>,
    #[serde(default)]
    pub children: Vec<WidgetNode>,
}

/// Flattened bounding-box entry extracted from a [`UiSnapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetBox {
    pub id: String,
    pub role: String,
    pub label: Option<String>,
    pub bounds: Rect,
    pub state: WidgetState,
}

/// Complete UI tree snapshot at a single point in time.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UiSnapshot {
    pub protocol_version: u32,
    pub app_id: String,
    pub toolkit: ToolkitKind,
    #[serde(default)]
    pub title: Option<String>,
    pub viewport: [u32; 2],
    pub scale_factor: f64,
    pub timestamp_ms: u64,
    pub root: WidgetNode,
}

impl UiSnapshot {
    /// Collects all visible nodes with valid bounding boxes in depth-first order.
    #[must_use]
    pub fn flatten_boxes(&self) -> Vec<WidgetBox> {
        let mut out = Vec::new();
        collect_boxes(&self.root, &mut out);
        out
    }

    /// Finds the first node matching `id` in depth-first order.
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<&WidgetNode> {
        find_node(&self.root, id)
    }
}

fn collect_boxes(node: &WidgetNode, out: &mut Vec<WidgetBox>) {
    if node.state.visible && node.bounds.is_valid() {
        out.push(WidgetBox {
            id: node.id.clone(),
            role: node.role.clone(),
            label: node.label.clone(),
            bounds: node.bounds,
            state: node.state,
        });
    }
    for child in &node.children {
        collect_boxes(child, out);
    }
}

fn find_node<'a>(node: &'a WidgetNode, id: &str) -> Option<&'a WidgetNode> {
    if node.id == id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_node(child, id) {
            return Some(found);
        }
    }
    None
}
