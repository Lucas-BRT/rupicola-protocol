//! Where a test keeps what it captured.
//!
//! By default a test captures into a temporary directory and removes it. With
//! `RUPICOLA_KEEP_CAPTURES` set to a directory, each test keeps its files in a folder of its
//! own there, so a person can look at what was captured.

use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// The environment variable that turns keeping on.
const KEEP_ENV: &str = "RUPICOLA_KEEP_CAPTURES";

/// The directory captures are kept in, when keeping is on.
fn keep_root() -> Option<PathBuf> {
    std::env::var_os(KEEP_ENV)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// An empty directory for the test called `name` to capture into.
pub fn out_dir(name: &str) -> PathBuf {
    let dir = keep_root().map_or_else(
        || std::env::temp_dir().join(format!("rupicola-capture-{name}")),
        |root| root.join(name),
    );
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Writes `value` as pretty JSON next to the captures when they are kept; otherwise nothing.
pub fn keep_json(dir: &Path, file: &str, value: &impl Serialize) {
    if keep_root().is_some() {
        let _ = fs::create_dir_all(dir);
        if let Ok(text) = serde_json::to_string_pretty(value) {
            let _ = fs::write(dir.join(file), text);
        }
    }
}

/// Removes `dir` unless captures are being kept.
pub fn finish(dir: &Path) {
    if keep_root().is_none() {
        let _ = fs::remove_dir_all(dir);
    }
}
