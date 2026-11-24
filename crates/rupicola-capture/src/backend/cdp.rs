use crate::session::Environment;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What the live session evaluates in the page to find elements and ask about them.
#[cfg(unix)]
pub(crate) const PAGE_HELPERS_JS: &str = include_str!("../cdp_page.js");
/// The shell that gives the browser its pipes.
#[cfg(unix)]
const SHELL: &str = "sh";
/// `--remote-debugging-pipe` reads commands from descriptor 3 and writes replies to
/// descriptor 4, and a plain pipe has only stdin and stdout. The shell moves the session's two
/// pipes onto 3 and 4 and gives the browser nothing on 0 and 1.
#[cfg(unix)]
const PIPE_SCRIPT: &str = r#"exec "$0" "$@" 3<&0 4>&1 0</dev/null 1>/dev/null"#;

/// Chromium-family programs tried, in order, when looking for a browser.
pub(crate) const BROWSER_CANDIDATES: [&str; 9] = [
    "chromium",
    "chromium-browser",
    "google-chrome",
    "google-chrome-stable",
    "brave",
    "brave-browser",
    "microsoft-edge",
    "microsoft-edge-stable",
    "vivaldi",
];

/// Lightweight Web capture backend using a local Chromium/Chrome binary directly, requiring no
/// Node.js or Playwright installation.
///
/// On Unix it keeps one headless browser for the whole session and talks to it over the Chrome
/// `DevTools` Protocol, so the page loads once, waits can look at it, and a checkpoint can come
/// with the page's UI tree. Elsewhere it runs the browser again for each screenshot, so the
/// page cannot be driven between checkpoints.
#[derive(Clone, Debug)]
pub struct CdpBackend {
    pub browser_bin: PathBuf,
}

impl CdpBackend {
    /// The command that starts the browser with its protocol on the session's pipes, a fresh
    /// profile in `profile`, and a blank page.
    #[cfg(unix)]
    pub(crate) fn launch(&self, environment: &Environment, profile: &Path) -> Command {
        let [width, height] = environment.viewport;
        let mut command = Command::new(SHELL);
        command
            .arg("-c")
            .arg(PIPE_SCRIPT)
            .arg(&self.browser_bin)
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
                "--no-first-run",
                "--no-default-browser-check",
                "--force-color-profile=srgb",
                "--remote-debugging-pipe",
            ])
            .arg(format!("--window-size={width},{height}"))
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("about:blank")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command
    }

    /// The command that loads `url` and saves a screenshot of it to `png`.
    #[cfg(not(unix))]
    pub(crate) fn screenshot(&self, url: &str, environment: &Environment, png: &Path) -> Command {
        let [width, height] = environment.viewport;
        let mut command = Command::new(&self.browser_bin);
        command
            .args([
                "--headless=new",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
            ])
            .arg(format!("--window-size={width},{height}"))
            .arg(format!("--screenshot={}", png.display()))
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}
