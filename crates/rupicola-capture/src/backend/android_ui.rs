//! Reads the view hierarchy that Android's UI Automator dumps (`uiautomator dump`) into a
//! [`UiSnapshot`].
//!
//! The dump is a small, fixed XML dialect, so it is read here by hand and no XML library is
//! needed. Android reports pixels; the snapshot's bounds are logical, so every length is
//! divided by the screen's density (160 dpi is one logical unit per pixel).

use rupicola_protocol::{
    Rect, SnapshotSource, ToolkitKind, UiSnapshot, WidgetKind, WidgetNode, WidgetState,
};
use std::collections::HashMap;

/// The density at which one logical unit is one pixel.
const BASELINE_DPI: u32 = 160;
/// The protocol version a snapshot declares.
const SNAPSHOT_PROTOCOL_VERSION: u32 = 1;
/// The package of a tree whose nodes name none.
const UNKNOWN_APP: &str = "android";
/// The id of the node that holds several top-level windows.
const SYNTHETIC_ROOT: &str = "hierarchy";
/// The first eight bytes of every PNG.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Where a PNG states its size: the IHDR width and height, each four big-endian bytes.
const PNG_WIDTH_AT: usize = 16;
const PNG_HEIGHT_AT: usize = 20;
const PNG_HEADER_LEN: usize = 24;

const HIERARCHY_OPEN: &str = "<hierarchy";
const HIERARCHY_CLOSE: &str = "</hierarchy>";

/// Why a dump could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DumpError(pub(crate) String);

impl std::fmt::Display for DumpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl DumpError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// A screen's density in dots per inch, which turns Android's pixels into logical units.
#[derive(Clone, Copy)]
struct Density(u32);

impl Density {
    /// Pixels per logical unit; a screen that states no density counts one to one.
    fn scale(self) -> f64 {
        if self.0 == 0 {
            1.0
        } else {
            f64::from(self.0) / f64::from(BASELINE_DPI)
        }
    }

    /// `pixels` in logical units, rounded to the nearest whole one.
    fn logical(self, pixels: u32) -> u32 {
        if self.0 == 0 {
            return pixels;
        }
        let units = (u64::from(pixels) * u64::from(BASELINE_DPI) + u64::from(self.0) / 2)
            / u64::from(self.0);
        u32::try_from(units).unwrap_or(u32::MAX)
    }
}

/// One `<node>` of the dump, before it is turned into a [`WidgetNode`].
#[derive(Debug, Default)]
struct Node {
    attributes: HashMap<String, String>,
    children: Vec<Node>,
}

impl Node {
    fn text(&self, name: &str) -> &str {
        self.attributes.get(name).map_or("", String::as_str)
    }

    fn flag(&self, name: &str) -> bool {
        self.text(name) == "true"
    }

    /// The `bounds="[left,top][right,bottom]"` of the node, in pixels.
    fn pixels(&self) -> [i32; 4] {
        let numbers: Vec<i32> = self
            .text("bounds")
            .split(|c: char| !(c.is_ascii_digit() || c == '-'))
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse().ok())
            .collect();
        match numbers[..] {
            [left, top, right, bottom] => [left, top, right, bottom],
            _ => [0; 4],
        }
    }
}

/// A parsed `uiautomator dump`.
#[derive(Debug)]
pub(crate) struct AndroidDump {
    top: Vec<Node>,
}

impl AndroidDump {
    /// Reads the hierarchy out of the dump's text, ignoring whatever the device prints
    /// around it (a warning before the XML, a sentence after it).
    pub(crate) fn parse(text: &str) -> Result<Self, DumpError> {
        let start = text
            .find(HIERARCHY_OPEN)
            .ok_or_else(|| DumpError::new("the dump holds no view hierarchy"))?;
        let body = &text[start..];
        let end = body
            .find(HIERARCHY_CLOSE)
            .map(|at| at + HIERARCHY_CLOSE.len())
            .ok_or_else(|| DumpError::new("the dump ends before its view hierarchy does"))?;
        let mut tags = Tags::new(&body[..end]);
        let mut stack = vec![Node::default()];
        while let Some(tag) = tags.next_tag()? {
            match tag {
                Tag::Open {
                    name,
                    attributes,
                    empty,
                } => {
                    if name == "node" {
                        let node = Node {
                            attributes,
                            children: Vec::new(),
                        };
                        if empty {
                            stack
                                .last_mut()
                                .expect("the root is always there")
                                .children
                                .push(node);
                        } else {
                            stack.push(node);
                        }
                    }
                }
                Tag::Close { name } => {
                    if name == "node" {
                        let node = stack.pop().filter(|_| !stack.is_empty());
                        let node = node.ok_or_else(|| DumpError::new("a node closes twice"))?;
                        stack.last_mut().expect("checked above").children.push(node);
                    }
                }
            }
        }
        match (stack.pop(), stack.is_empty()) {
            (Some(root), true) => Ok(Self { top: root.children }),
            _ => Err(DumpError::new("a node is never closed")),
        }
    }

    /// The tree as a snapshot of a screen `screen` pixels wide and high, at `density_dpi`.
    pub(crate) fn into_snapshot(
        self,
        screen: [u32; 2],
        density_dpi: u32,
        timestamp_ms: u64,
    ) -> UiSnapshot {
        let density = Density(density_dpi);
        let scale = density.scale();
        let logical = |pixels: u32| f64::from(pixels) / scale;
        let app_id = self
            .top
            .iter()
            .find_map(Self::package)
            .unwrap_or_else(|| UNKNOWN_APP.to_owned());
        let mut converted: Vec<WidgetNode> = self
            .top
            .iter()
            .enumerate()
            .map(|(index, node)| Self::widget(node, &index.to_string(), scale))
            .collect();
        let root = if converted.len() == 1 {
            converted.remove(0)
        } else {
            WidgetNode {
                id: SYNTHETIC_ROOT.to_owned(),
                role: SYNTHETIC_ROOT.to_owned(),
                kind: WidgetKind::Window,
                label: None,
                value: None,
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: logical(screen[0]),
                    height: logical(screen[1]),
                },
                state: WidgetState {
                    visible: true,
                    enabled: true,
                    ..WidgetState::default()
                },
                style_hint: None,
                style: None,
                children: converted,
            }
        };
        UiSnapshot {
            protocol_version: SNAPSHOT_PROTOCOL_VERSION,
            app_id,
            toolkit: ToolkitKind::Android,
            title: None,
            viewport: [density.logical(screen[0]), density.logical(screen[1])],
            scale_factor: scale,
            timestamp_ms,
            source: SnapshotSource::Uiautomator,
            root,
        }
    }

    /// The first package named in `node` or below it.
    fn package(node: &Node) -> Option<String> {
        let own = node.text("package");
        if own.is_empty() {
            node.children.iter().find_map(Self::package)
        } else {
            Some(own.to_owned())
        }
    }

    /// `node` as a widget. `path` is where it sits in the tree, which names it when Android
    /// gave it no resource id.
    fn widget(node: &Node, path: &str, scale: f64) -> WidgetNode {
        let class = node.text("class");
        let kind = WidgetKind::from_android_class(class);
        let simple = class.rsplit('.').next().unwrap_or(class);
        let resource = node.text("resource-id");
        let text = node.text("text");
        let description = node.text("content-desc");
        let secret = node.flag("password");
        // A text box's text is what was typed, so it is the value; elsewhere it is the label.
        let (label, value) = if secret {
            (None, None)
        } else if kind == WidgetKind::TextBox {
            (non_empty(description), non_empty(text))
        } else if text.is_empty() {
            (non_empty(description), None)
        } else {
            (non_empty(text), None)
        };
        let [left, top, right, bottom] = node.pixels();
        let to_logical = |pixels: i32| f64::from(pixels) / scale;
        let visible = right > left && bottom > top;
        let checkable = node.flag("checkable");
        WidgetNode {
            id: if resource.is_empty() {
                format!("{simple}-{path}")
            } else {
                resource.to_owned()
            },
            role: class.to_owned(),
            kind,
            label,
            value,
            bounds: Rect {
                x: to_logical(left),
                y: to_logical(top),
                width: to_logical(right - left),
                height: to_logical(bottom - top),
            },
            state: WidgetState {
                visible,
                enabled: node.flag("enabled"),
                focused: node.flag("focused"),
                hovered: false,
                pressed: false,
                checked: checkable.then(|| node.flag("checked")),
                selected: Some(node.flag("selected")),
            },
            style_hint: None,
            style: None,
            children: node
                .children
                .iter()
                .enumerate()
                .map(|(index, child)| Self::widget(child, &format!("{path}-{index}"), scale))
                .collect(),
        }
    }
}

fn non_empty(text: &str) -> Option<String> {
    (!text.is_empty()).then(|| text.to_owned())
}

/// The density `wm density` reports, in dots per inch. An override, when the user changed it,
/// is what the screen really uses.
pub(crate) fn density_dpi(wm_density: &str) -> Option<u32> {
    let after = |label: &str| {
        wm_density
            .lines()
            .find_map(|line| line.trim().strip_prefix(label))
            .and_then(|rest| rest.trim().parse::<u32>().ok())
    };
    after("Override density:").or_else(|| after("Physical density:"))
}

/// The width and height a PNG states in its header.
pub(crate) fn png_size(png: &[u8]) -> Option<[u32; 2]> {
    if png.len() < PNG_HEADER_LEN || png[..PNG_SIGNATURE.len()] != PNG_SIGNATURE {
        return None;
    }
    let read = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    Some([read(PNG_WIDTH_AT), read(PNG_HEIGHT_AT)])
}

/// A tag of the XML, with its attributes decoded.
enum Tag {
    Open {
        name: String,
        attributes: HashMap<String, String>,
        /// `<node .../>`: it has no children and no closing tag.
        empty: bool,
    },
    Close {
        name: String,
    },
}

/// Walks the tags of a piece of XML.
struct Tags<'a> {
    rest: &'a str,
}

impl<'a> Tags<'a> {
    const fn new(text: &'a str) -> Self {
        Self { rest: text }
    }

    fn next_tag(&mut self) -> Result<Option<Tag>, DumpError> {
        loop {
            let Some(open) = self.rest.find('<') else {
                return Ok(None);
            };
            self.rest = &self.rest[open + 1..];
            // A declaration or a comment is skipped, up to its end.
            if self.rest.starts_with('?') || self.rest.starts_with('!') {
                let end = self
                    .rest
                    .find('>')
                    .ok_or_else(|| DumpError::new("a declaration never ends"))?;
                self.rest = &self.rest[end + 1..];
                continue;
            }
            if let Some(after) = self.rest.strip_prefix('/') {
                let end = after
                    .find('>')
                    .ok_or_else(|| DumpError::new("a closing tag never ends"))?;
                let name = after[..end].trim().to_owned();
                self.rest = &after[end + 1..];
                return Ok(Some(Tag::Close { name }));
            }
            return self.open_tag().map(Some);
        }
    }

    fn open_tag(&mut self) -> Result<Tag, DumpError> {
        let name_end = self
            .rest
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .ok_or_else(|| DumpError::new("a tag never ends"))?;
        let name = self.rest[..name_end].to_owned();
        self.rest = &self.rest[name_end..];
        let mut attributes = HashMap::new();
        loop {
            self.rest = self.rest.trim_start();
            if let Some(after) = self.rest.strip_prefix("/>") {
                self.rest = after;
                return Ok(Tag::Open {
                    name,
                    attributes,
                    empty: true,
                });
            }
            if let Some(after) = self.rest.strip_prefix('>') {
                self.rest = after;
                return Ok(Tag::Open {
                    name,
                    attributes,
                    empty: false,
                });
            }
            let equals = self
                .rest
                .find('=')
                .ok_or_else(|| DumpError::new(format!("an attribute of <{name}> has no value")))?;
            let key = self.rest[..equals].trim().to_owned();
            let after = self.rest[equals + 1..].trim_start();
            let quote = after
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or_else(|| DumpError::new(format!("`{key}` is not quoted")))?;
            let value_end = after[1..]
                .find(quote)
                .ok_or_else(|| DumpError::new(format!("`{key}` is never closed")))?;
            attributes.insert(key, decode(&after[1..=value_end]));
            self.rest = &after[value_end + 2..];
        }
    }
}

/// Replaces the XML entities an attribute value may hold.
fn decode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else {
            break;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        if let Some(c) = decoded {
            out.push(c);
            rest = &rest[end + 1..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DENSITY: u32 = 480;
    const SCREEN: [u32; 2] = [1080, 2400];

    /// A made-up dump in the shape the device prints it: noise before, a sentence after.
    const DUMP: &str = "java.io.FileNotFoundException: /data/theme.xml: open failed\n\tat libcore.io.IoBridge.open(IoBridge.java:1)\n\
<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>\
<hierarchy rotation=\"0\">\
<node index=\"0\" text=\"\" resource-id=\"\" class=\"android.widget.FrameLayout\" package=\"org.example.mail\" content-desc=\"\" checkable=\"false\" checked=\"false\" clickable=\"false\" enabled=\"true\" focusable=\"false\" focused=\"false\" scrollable=\"false\" long-clickable=\"false\" password=\"false\" selected=\"false\" bounds=\"[0,0][1080,2400]\">\
<node index=\"0\" text=\"Send &amp; close\" resource-id=\"org.example.mail:id/send\" class=\"android.widget.Button\" package=\"org.example.mail\" content-desc=\"\" checkable=\"false\" checked=\"false\" clickable=\"true\" enabled=\"true\" focusable=\"true\" focused=\"true\" scrollable=\"false\" long-clickable=\"false\" password=\"false\" selected=\"false\" bounds=\"[48,96][528,240]\" />\
<node index=\"1\" text=\"ana@example.com\" resource-id=\"\" class=\"android.widget.EditText\" package=\"org.example.mail\" content-desc=\"Recipient\" checkable=\"false\" checked=\"false\" clickable=\"true\" enabled=\"true\" focusable=\"true\" focused=\"false\" scrollable=\"false\" long-clickable=\"false\" password=\"false\" selected=\"false\" bounds=\"[48,300][1032,444]\" />\
<node index=\"2\" text=\"hunter2\" resource-id=\"\" class=\"android.widget.EditText\" package=\"org.example.mail\" content-desc=\"\" checkable=\"false\" checked=\"false\" clickable=\"true\" enabled=\"true\" focusable=\"true\" focused=\"false\" scrollable=\"false\" long-clickable=\"false\" password=\"true\" selected=\"false\" bounds=\"[48,480][1032,624]\" />\
<node index=\"3\" text=\"Remember me\" resource-id=\"\" class=\"android.widget.CheckBox\" package=\"org.example.mail\" content-desc=\"\" checkable=\"true\" checked=\"true\" clickable=\"true\" enabled=\"false\" focusable=\"true\" focused=\"false\" scrollable=\"false\" long-clickable=\"false\" password=\"false\" selected=\"false\" bounds=\"[48,660][528,804]\" />\
<node index=\"4\" text=\"\" resource-id=\"\" class=\"android.widget.ImageView\" package=\"org.example.mail\" content-desc=\"Logo\" checkable=\"false\" checked=\"false\" clickable=\"false\" enabled=\"true\" focusable=\"false\" focused=\"false\" scrollable=\"false\" long-clickable=\"false\" password=\"false\" selected=\"false\" bounds=\"[0,0][0,0]\" />\
</node></hierarchy>UI hierchary dumped to: /dev/tty\n";

    fn snapshot() -> UiSnapshot {
        AndroidDump::parse(DUMP)
            .unwrap()
            .into_snapshot(SCREEN, DENSITY, 7)
    }

    fn find<'a>(snapshot: &'a UiSnapshot, id: &str) -> &'a WidgetNode {
        snapshot
            .find_by_id(id)
            .unwrap_or_else(|| panic!("no node {id}"))
    }

    #[test]
    fn the_dump_is_read_through_the_noise_around_it() {
        let snapshot = snapshot();
        assert_eq!(snapshot.toolkit, ToolkitKind::Android);
        assert_eq!(snapshot.source, SnapshotSource::Uiautomator);
        assert_eq!(snapshot.app_id, "org.example.mail");
        assert_eq!(snapshot.timestamp_ms, 7);
        assert_eq!(snapshot.root.children.len(), 5);
        assert_eq!(snapshot.root.kind, WidgetKind::Group);
    }

    #[test]
    fn lengths_are_logical_so_the_scale_brings_them_back_to_the_screenshots_pixels() {
        let snapshot = snapshot();
        assert!((snapshot.scale_factor - 3.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.viewport, [360, 800]);
        let send = find(&snapshot, "org.example.mail:id/send");
        assert_eq!(
            send.bounds.to_pixel_box(snapshot.scale_factor),
            [48, 96, 480, 144]
        );
        assert_eq!(
            snapshot.root.bounds.to_pixel_box(snapshot.scale_factor),
            [0, 0, 1080, 2400]
        );
    }

    #[test]
    fn widgets_get_a_kind_a_label_and_the_states_android_reports() {
        let snapshot = snapshot();
        let send = find(&snapshot, "org.example.mail:id/send");
        assert_eq!(send.kind, WidgetKind::Button);
        assert_eq!(
            send.label.as_deref(),
            Some("Send & close"),
            "entities are decoded"
        );
        assert!(send.state.focused && send.state.enabled && send.state.visible);
        assert_eq!(send.state.checked, None, "a button is not checkable");
        let recipient = find(&snapshot, "EditText-0-1");
        assert_eq!(recipient.kind, WidgetKind::TextBox);
        assert_eq!(
            recipient.value.as_deref(),
            Some("ana@example.com"),
            "typed text is the value"
        );
        assert_eq!(recipient.label.as_deref(), Some("Recipient"));
        let remember = find(&snapshot, "CheckBox-0-3");
        assert_eq!(remember.kind, WidgetKind::Checkbox);
        assert_eq!(remember.state.checked, Some(true));
        assert!(!remember.state.enabled);
        let logo = find(&snapshot, "ImageView-0-4");
        assert_eq!(
            logo.label.as_deref(),
            Some("Logo"),
            "a content description labels an image"
        );
        assert!(!logo.state.visible, "an empty rectangle is not on screen");
    }

    #[test]
    fn a_password_is_never_kept() {
        let snapshot = snapshot();
        let secret = find(&snapshot, "EditText-0-2");
        assert_eq!(
            (secret.label.as_deref(), secret.value.as_deref()),
            (None, None)
        );
        assert!(
            !serde_json::to_string(&snapshot)
                .unwrap()
                .contains("hunter2")
        );
    }

    #[test]
    fn several_top_level_windows_share_a_synthetic_root() {
        let two = "<hierarchy rotation=\"0\">\
            <node index=\"0\" class=\"android.widget.FrameLayout\" package=\"a\" bounds=\"[0,0][10,10]\" />\
            <node index=\"1\" class=\"android.widget.FrameLayout\" package=\"b\" bounds=\"[0,0][10,10]\" />\
            </hierarchy>";
        let snapshot = AndroidDump::parse(two)
            .unwrap()
            .into_snapshot([10, 10], 160, 0);
        assert_eq!(snapshot.root.id, SYNTHETIC_ROOT);
        assert_eq!(snapshot.root.children.len(), 2);
        assert_eq!(snapshot.app_id, "a");
    }

    #[test]
    fn a_dump_that_is_not_a_hierarchy_is_refused_with_a_reason() {
        for (text, reason) in [
            (
                "ERROR: null root node returned by UiTestAutomationBridge.",
                "no view hierarchy",
            ),
            (
                "<hierarchy rotation=\"0\"><node index=\"0\" ",
                "ends before",
            ),
            ("<hierarchy><node class=\"a\"></hierarchy>", "never closed"),
            (
                "<hierarchy><node class=\"a\" bounds=[0,0]></node></hierarchy>",
                "not quoted",
            ),
        ] {
            let error = AndroidDump::parse(text).unwrap_err();
            assert!(error.0.contains(reason), "{text}: {error}");
        }
    }

    #[test]
    fn entities_decode_and_an_unknown_one_stays_as_written() {
        assert_eq!(
            decode("a &amp; b &lt;c&gt; &quot;d&quot; &apos;e&apos;"),
            "a & b <c> \"d\" 'e'"
        );
        assert_eq!(decode("&#65;&#x42;"), "AB");
        assert_eq!(decode("fish &chips; &"), "fish &chips; &");
    }

    #[test]
    fn the_density_is_the_override_when_there_is_one() {
        assert_eq!(density_dpi("Physical density: 440\n"), Some(440));
        assert_eq!(
            density_dpi("Physical density: 440\nOverride density: 420\n"),
            Some(420)
        );
        assert_eq!(density_dpi("nothing here"), None);
    }

    #[test]
    fn a_png_states_its_size_in_its_header() {
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend([0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        png.extend(1080_u32.to_be_bytes());
        png.extend(2400_u32.to_be_bytes());
        assert_eq!(png_size(&png), Some([1080, 2400]));
        assert_eq!(png_size(&png[..10]), None);
        assert_eq!(png_size(b"not a png at all, not at all, no"), None);
    }
}
