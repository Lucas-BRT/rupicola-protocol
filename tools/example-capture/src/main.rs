mod media;
#[cfg(unix)]
mod native;
mod refresh;
mod support;
mod web;

use std::path::{Path, PathBuf};

const USAGE: &str = "Usage:
  example-capture refresh-medium
  example-capture workspace --framework html
  example-capture workspace --framework react --url URL
  example-capture workspace --framework iced|cosmic --binary PATH";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let args: Vec<_> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["--help" | "-h"] => {
            println!("{USAGE}");
            Ok(())
        }
        ["refresh-medium"] => {
            refresh::run();
            Ok(())
        }
        ["workspace", "--framework", "html"] => web::capture("html", None),
        [
            "workspace",
            "--framework",
            framework @ ("html" | "react"),
            "--url",
            url,
        ] => web::capture(framework, Some(url)),
        [
            "workspace",
            "--framework",
            framework @ ("iced" | "cosmic"),
            "--binary",
            binary,
        ] => capture_native(framework, binary),
        _ => Err(USAGE.into()),
    }
}

#[cfg(unix)]
fn capture_native(framework: &str, binary: &str) -> Result<(), String> {
    native::capture(framework, binary)
}

#[cfg(not(unix))]
fn capture_native(_: &str, _: &str) -> Result<(), String> {
    Err("native example capture requires Xvfb on Unix".into())
}
