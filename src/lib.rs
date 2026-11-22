//! Lightweight middleware micro-protocol for UI widget trees, bounding boxes,
//! and remote interaction across Web DOM and Desktop toolkits.

pub mod action;
pub mod geometry;
pub mod message;
pub mod transport;
pub mod tree;

pub use action::UiAction;
pub use geometry::Rect;
pub use message::{ProtocolRequest, ProtocolResponse};
pub use transport::{read_ndjson, write_ndjson};
pub use tree::{ToolkitKind, UiSnapshot, WidgetBox, WidgetNode, WidgetState};

/// Current wire protocol version.
pub const PROTOCOL_VERSION: u32 = 1;
