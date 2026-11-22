use crate::{
    action::UiAction,
    tree::{ToolkitKind, UiSnapshot},
};
use serde::{Deserialize, Serialize};

/// Request sent by a `CaptureBackend` to a Middleware adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProtocolRequest {
    Handshake {
        protocol_version: u32,
        client: String,
    },
    Ping,
    GetSnapshot,
    PerformAction {
        action: UiAction,
    },
}

/// Response returned by a Middleware adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProtocolResponse {
    HandshakeAck {
        protocol_version: u32,
        app_id: String,
        toolkit: ToolkitKind,
        capabilities: Vec<String>,
    },
    Pong,
    Snapshot {
        snapshot: UiSnapshot,
    },
    ActionAck {
        ok: bool,
        #[serde(default)]
        message: Option<String>,
    },
    Error {
        code: String,
        message: String,
    },
}
