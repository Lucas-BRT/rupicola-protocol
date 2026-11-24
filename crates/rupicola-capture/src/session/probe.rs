use super::Capabilities;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Category of capture target a backend handles.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Web,
    Desktop,
    Mobile,
}

/// Host availability, detected binaries, missing dependencies, and install guidance for one
/// capture backend.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct BackendReport {
    pub id: String,
    pub label: String,
    pub kind: BackendKind,
    pub available: bool,
    pub capabilities: Capabilities,
    pub detected_binaries: Vec<PathBuf>,
    pub missing_dependencies: Vec<String>,
    pub install_guidance: String,
}
