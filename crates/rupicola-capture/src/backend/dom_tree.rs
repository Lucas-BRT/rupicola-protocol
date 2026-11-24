//! The tree of a web page, as `dom_snapshot.js` reports it, made ready to keep.

use rupicola_protocol::{SnapshotSource, UiSnapshot, WidgetKind, WidgetNode};

/// What walks the DOM in the page: the one script every web backend runs.
pub(crate) const DOM_SNAPSHOT_JS: &str = include_str!("../dom_snapshot.js");

/// A tree walked out of a page's DOM.
pub(crate) struct DomTree;

impl DomTree {
    /// The tree, marked as a DOM and with every node in the shared vocabulary.
    pub(crate) fn finish(mut tree: UiSnapshot) -> UiSnapshot {
        tree.source = SnapshotSource::Dom;
        Self::classify(&mut tree.root);
        tree
    }

    fn classify(node: &mut WidgetNode) {
        node.kind = WidgetKind::from_dom(&node.role);
        for child in &mut node.children {
            Self::classify(child);
        }
    }
}
