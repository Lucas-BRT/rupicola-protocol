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
    /// The Android view system.
    Android,
    Other(String),
}

/// Where a snapshot came from, so a reader knows how much to trust its shape.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotSource {
    /// A web page's DOM.
    Dom,
    /// The desktop accessibility bus (AT-SPI).
    Atspi,
    /// Android's UI Automator.
    Uiautomator,
    /// An adapter inside the application, speaking this protocol.
    #[default]
    Adapter,
}

/// What a widget is, in one small vocabulary that every platform maps onto, so "all the
/// buttons" means the same on the web, on Android and on a desktop. The platform's own name
/// for the widget stays in [`WidgetNode::role`].
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetKind {
    Button,
    Link,
    Text,
    Heading,
    Image,
    TextBox,
    Checkbox,
    Radio,
    Switch,
    Slider,
    ComboBox,
    List,
    ListItem,
    Tab,
    Menu,
    MenuItem,
    Dialog,
    Window,
    /// A container that only holds other widgets.
    Group,
    /// Anything the vocabulary has no word for, and any word a newer sender adds.
    #[default]
    #[serde(other)]
    Other,
}

impl WidgetKind {
    /// The kind of a DOM element, from its ARIA role or, failing that, its tag name.
    #[must_use]
    pub fn from_dom(role: &str) -> Self {
        match role.to_ascii_lowercase().as_str() {
            "button" | "submit" | "reset" => Self::Button,
            "a" | "link" => Self::Link,
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "heading" => Self::Heading,
            "img" | "image" | "svg" | "canvas" | "video" | "picture" => Self::Image,
            "textbox" | "textarea" | "searchbox" | "input" => Self::TextBox,
            "checkbox" => Self::Checkbox,
            "radio" => Self::Radio,
            "switch" => Self::Switch,
            "slider" | "range" => Self::Slider,
            "select" | "combobox" | "listbox" => Self::ComboBox,
            "ul" | "ol" | "list" | "menubar" => Self::List,
            "li" | "listitem" | "option" => Self::ListItem,
            "tab" => Self::Tab,
            "menu" => Self::Menu,
            "menuitem" => Self::MenuItem,
            "dialog" | "alertdialog" => Self::Dialog,
            "p" | "span" | "label" | "text" | "strong" | "em" | "small" | "code" => Self::Text,
            "div" | "section" | "article" | "nav" | "main" | "header" | "footer" | "aside"
            | "form" | "group" | "toolbar" | "tablist" | "body" | "html" => Self::Group,
            _ => Self::Other,
        }
    }

    /// The kind of an Android view, from its class name (`android.widget.Button`, or a
    /// library's subclass such as `com.google.android.material.button.MaterialButton`).
    #[must_use]
    pub fn from_android_class(class: &str) -> Self {
        let name = class.rsplit('.').next().unwrap_or(class);
        // Most specific first: a RadioButton and a CheckBox are also buttons of a kind.
        let has = |part: &str| name.contains(part);
        if has("RadioButton") {
            Self::Radio
        } else if has("CheckBox") || has("CheckedTextView") {
            Self::Checkbox
        } else if has("Switch") || has("ToggleButton") {
            Self::Switch
        } else if has("Button") {
            Self::Button
        } else if has("EditText") || has("TextInput") || has("AutoComplete") {
            Self::TextBox
        } else if has("SeekBar") || has("Slider") {
            Self::Slider
        } else if has("Spinner") {
            Self::ComboBox
        } else if has("ImageView") || has("Image") {
            Self::Image
        } else if has("TextView") {
            Self::Text
        } else if has("TabLayout") || has("TabWidget") {
            Self::Tab
        } else if has("RecyclerView") || has("ListView") || has("GridView") {
            Self::List
        } else if has("Dialog") {
            Self::Dialog
        } else if has("Layout") || has("ViewGroup") || name == "View" || has("ScrollView") {
            Self::Group
        } else {
            Self::Other
        }
    }
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

/// Computed style properties reported for a DOM widget when available.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ComputedStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size_px: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_x: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overflow_y: Option<String>,
}

/// Node in a captured UI tree.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WidgetNode {
    pub id: String,
    /// The platform's own name for the widget: an ARIA role or tag, an Android class.
    pub role: String,
    /// What the widget is, in the vocabulary every platform shares.
    #[serde(default)]
    pub kind: WidgetKind,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    pub bounds: Rect,
    #[serde(default)]
    pub state: WidgetState,
    #[serde(default)]
    pub style_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<ComputedStyle>,
    #[serde(default)]
    pub children: Vec<WidgetNode>,
}

/// Flattened bounding-box entry extracted from a [`UiSnapshot`].
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetBox {
    pub id: String,
    pub role: String,
    pub kind: WidgetKind,
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
    /// Size of the captured surface, in logical units.
    pub viewport: [u32; 2],
    /// Physical pixels per logical unit. Every `bounds` in the tree is logical, so the
    /// pixels of the screenshot taken at the same moment are `bounds * scale_factor`
    /// (see [`crate::Rect::to_pixel_box`]).
    pub scale_factor: f64,
    pub timestamp_ms: u64,
    /// How the tree was obtained.
    #[serde(default)]
    pub source: SnapshotSource,
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

    /// The same tree without any text a person could have typed or read: the window title and
    /// every label and value. Roles, kinds, bounds and states stay, so the layout can still be
    /// checked.
    #[must_use]
    pub fn without_text(&self) -> Self {
        let mut stripped = self.clone();
        stripped.title = None;
        strip_text(&mut stripped.root);
        stripped
    }
}

fn strip_text(node: &mut WidgetNode) {
    node.label = None;
    node.value = None;
    for child in &mut node.children {
        strip_text(child);
    }
}

fn collect_boxes(node: &WidgetNode, out: &mut Vec<WidgetBox>) {
    if node.state.visible && node.bounds.is_valid() {
        out.push(WidgetBox {
            id: node.id.clone(),
            role: node.role.clone(),
            kind: node.kind,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, kind: WidgetKind, label: Option<&str>) -> WidgetNode {
        WidgetNode {
            id: id.to_owned(),
            role: "x".to_owned(),
            kind,
            label: label.map(str::to_owned),
            value: label.map(|text| format!("value of {text}")),
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            state: WidgetState {
                visible: true,
                ..WidgetState::default()
            },
            style_hint: None,
            style: None,
            children: Vec::new(),
        }
    }

    fn snapshot() -> UiSnapshot {
        let mut root = node("root", WidgetKind::Window, Some("Inbox"));
        root.children = vec![
            node("send", WidgetKind::Button, Some("Send")),
            node("box", WidgetKind::TextBox, Some("secret@example.com")),
        ];
        UiSnapshot {
            protocol_version: 1,
            app_id: "org.example.mail".to_owned(),
            toolkit: ToolkitKind::Android,
            title: Some("Inbox of Ana".to_owned()),
            viewport: [360, 800],
            scale_factor: 3.0,
            timestamp_ms: 7,
            source: SnapshotSource::Uiautomator,
            root,
        }
    }

    #[test]
    fn a_snapshot_from_before_kinds_and_sources_still_reads() {
        let old = json!({
            "protocol_version": 1, "app_id": "a", "toolkit": "gtk4",
            "viewport": [1, 1], "scale_factor": 1.0, "timestamp_ms": 0,
            "root": {"id": "r", "role": "window",
                     "bounds": {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}}
        });
        let read: UiSnapshot = serde_json::from_value(old).unwrap();
        assert_eq!(read.source, SnapshotSource::Adapter);
        assert_eq!(read.root.kind, WidgetKind::Other);
    }

    #[test]
    fn a_kind_a_newer_sender_adds_reads_as_other() {
        let kind: WidgetKind = serde_json::from_value(json!("carousel")).unwrap();
        assert_eq!(kind, WidgetKind::Other);
        assert_eq!(
            serde_json::to_value(WidgetKind::TextBox).unwrap(),
            "text-box"
        );
    }

    #[test]
    fn a_snapshot_survives_a_round_trip() {
        let text = serde_json::to_string(&snapshot()).unwrap();
        assert_eq!(
            serde_json::from_str::<UiSnapshot>(&text).unwrap(),
            snapshot()
        );
    }

    #[test]
    fn without_text_keeps_the_layout_and_drops_every_string_a_person_wrote() {
        let stripped = snapshot().without_text();
        assert_eq!(stripped.title, None);
        let all = serde_json::to_string(&stripped).unwrap();
        for leaked in ["Inbox", "Send", "secret@example.com", "value of"] {
            assert!(!all.contains(leaked), "{leaked} is still there: {all}");
        }
        let boxes = stripped.flatten_boxes();
        assert_eq!(boxes.len(), 3);
        assert_eq!(boxes[1].kind, WidgetKind::Button);
        assert_eq!(boxes[1].bounds, snapshot().root.children[0].bounds);
        assert_eq!(
            snapshot().title.as_deref(),
            Some("Inbox of Ana"),
            "the original is untouched"
        );
    }

    #[test]
    fn dom_roles_and_tags_map_to_kinds() {
        for (raw, kind) in [
            ("button", WidgetKind::Button),
            ("A", WidgetKind::Link),
            ("h3", WidgetKind::Heading),
            ("img", WidgetKind::Image),
            ("textarea", WidgetKind::TextBox),
            ("checkbox", WidgetKind::Checkbox),
            ("range", WidgetKind::Slider),
            ("select", WidgetKind::ComboBox),
            ("li", WidgetKind::ListItem),
            ("dialog", WidgetKind::Dialog),
            ("span", WidgetKind::Text),
            ("div", WidgetKind::Group),
            ("marquee", WidgetKind::Other),
        ] {
            assert_eq!(WidgetKind::from_dom(raw), kind, "{raw}");
        }
    }

    #[test]
    fn android_classes_map_to_kinds_most_specific_first() {
        for (class, kind) in [
            ("android.widget.Button", WidgetKind::Button),
            ("android.widget.ImageButton", WidgetKind::Button),
            (
                "com.google.android.material.button.MaterialButton",
                WidgetKind::Button,
            ),
            ("android.widget.RadioButton", WidgetKind::Radio),
            ("android.widget.CheckBox", WidgetKind::Checkbox),
            ("androidx.appcompat.widget.SwitchCompat", WidgetKind::Switch),
            ("android.widget.EditText", WidgetKind::TextBox),
            ("android.widget.SeekBar", WidgetKind::Slider),
            ("android.widget.Spinner", WidgetKind::ComboBox),
            ("android.widget.ImageView", WidgetKind::Image),
            ("android.widget.TextView", WidgetKind::Text),
            (
                "androidx.recyclerview.widget.RecyclerView",
                WidgetKind::List,
            ),
            ("android.widget.FrameLayout", WidgetKind::Group),
            ("android.view.View", WidgetKind::Group),
            ("android.webkit.WebView", WidgetKind::Other),
        ] {
            assert_eq!(WidgetKind::from_android_class(class), kind, "{class}");
        }
    }
}

#[cfg(test)]
mod computed_style_tests {
    use super::*;

    #[test]
    fn computed_style_and_selected_round_trip() {
        let json = r#"{
            "id": "tab-1",
            "role": "tab",
            "kind": "tab",
            "bounds": {"x": 0.0, "y": 0.0, "width": 80.0, "height": 32.0},
            "state": {"visible": true, "enabled": true, "selected": true},
            "style": {
                "color": "rgb(255, 255, 255)",
                "background_color": "rgb(17, 17, 17)",
                "font_size_px": 14.0,
                "overflow_x": "hidden",
                "overflow_y": "auto"
            }
        }"#;
        let node: WidgetNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.state.selected, Some(true));
        let style = node.style.as_ref().unwrap();
        assert_eq!(style.color.as_deref(), Some("rgb(255, 255, 255)"));
        assert_eq!(style.font_size_px, Some(14.0));
        assert_eq!(style.overflow_y.as_deref(), Some("auto"));
    }
}
