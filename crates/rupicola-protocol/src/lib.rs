//! Lightweight middleware micro-protocol for UI widget trees, bounding boxes,
//! temporal video annotations, and remote interaction across Web DOM and Desktop toolkits.

pub mod action;
pub mod annotation;
pub mod geometry;
pub mod message;
pub mod transport;
pub mod tree;

pub use action::UiAction;
pub use annotation::{
    AnnotationBox, AnnotationKeyframe, AnnotationShape, AnnotationStyle, Easing, LabelPosition,
    TimeRange,
};
pub use geometry::Rect;
pub use message::{ProtocolRequest, ProtocolResponse};
#[cfg(unix)]
pub use transport::{UnixProtocolClient, serve_once_unix};
pub use transport::{read_ndjson, write_ndjson};
pub use tree::{
    ComputedStyle, SnapshotSource, ToolkitKind, UiSnapshot, WidgetBox, WidgetKind, WidgetNode,
    WidgetState,
};

/// Current wire protocol version.
pub const PROTOCOL_VERSION: u32 = 1;
