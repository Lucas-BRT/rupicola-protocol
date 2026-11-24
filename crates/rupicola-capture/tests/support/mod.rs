//! Where a test keeps what it captured.
//!
//! By default a test captures into a temporary directory and removes it. With
//! `RUPICOLA_KEEP_CAPTURES` set to a directory, each test keeps its files in a folder of its
//! own there, so a person can look at what was captured.

#![allow(
    dead_code,
    reason = "every test binary compiles this module and uses only part of it"
)]

use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

/// Pause between accepts while the test server waits for a connection.
const ACCEPT_POLL: Duration = Duration::from_millis(10);

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

/// Serves the CRUD example until the returned flag is set.
pub fn serve_example() -> (String, Arc<AtomicBool>) {
    let page = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/apps/medium/tasks/html/index.html");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/index.html", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    thread::spawn(move || {
        while !flag.load(Ordering::Relaxed) {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                if let Ok(body) = fs::read(&page) {
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&body);
                }
            } else {
                thread::sleep(ACCEPT_POLL);
            }
        }
    });
    (url, stop)
}
