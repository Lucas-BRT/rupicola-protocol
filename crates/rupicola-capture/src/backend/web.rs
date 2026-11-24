use super::dom_tree::DOM_SNAPSHOT_JS;
use super::web_driver::Frame;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Folder, inside the output directory, where a recording's frames wait for encoding.
pub(crate) const FRAMES_DIR: &str = ".recording-frames";
/// Name of the list that tells `ffmpeg` which frames make a clip.
pub(crate) const FRAME_LIST: &str = "frames.txt";
/// Display time given to the last frame of a clip, in seconds.
const LAST_FRAME_SECONDS: f64 = 0.1;
/// Milliseconds per second.
const MS_PER_SECOND: f64 = 1000.0;
/// Environment variable that names a Playwright module directory.
const PLAYWRIGHT_DIR_ENV: &str = "RUPICOLA_PLAYWRIGHT_DIR";
const SESSION_DRIVER: &str = include_str!("../playwright_session_driver.js");

/// Headless Web capture backend powered by Playwright (`chromium`, `firefox`, or `webkit`).
#[derive(Clone, Debug)]
pub struct WebBackend {
    pub browser_engine: String,
    pub node_bin: PathBuf,
    pub playwright_module: PathBuf,
}

/// Where to look for `node` and a Playwright module.
pub(crate) struct Candidates {
    pub(crate) nodes: Vec<PathBuf>,
    pub(crate) modules: Vec<PathBuf>,
}

impl WebBackend {
    /// The places `node` and Playwright are looked for; `None` without a home directory.
    pub(crate) fn candidates() -> Option<Candidates> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        Some(Candidates {
            nodes: vec![
                PathBuf::from("node"),
                home.join(".local/share/mise/installs/node/lts/bin/node"),
            ],
            modules: std::env::var_os(PLAYWRIGHT_DIR_ENV)
                .map(PathBuf::from)
                .into_iter()
                .collect(),
        })
    }

    /// The command that asks `node` for its version, to tell it works.
    pub(crate) fn node_probe(node: &Path) -> Command {
        let mut command = Command::new(node);
        command
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// The command that runs the session driver, speaking JSON lines on stdin and stdout.
    pub(crate) fn driver(&self) -> Command {
        let mut command = Command::new(&self.node_bin);
        command
            .arg("-e")
            // The driver hands the DOM walker to the page, so the walker comes first.
            .arg(format!("{DOM_SNAPSHOT_JS}\n{SESSION_DRIVER}"))
            .env("PW_DIR", &self.playwright_module)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        command
    }
}

/// Turns the frames of a recording into a clip.
pub(crate) struct Encoder;

impl Encoder {
    /// The `ffmpeg` concat list that holds each frame until the next was taken.
    pub(crate) fn frame_list(frames: &[Frame]) -> String {
        let mut script = String::new();
        for (index, frame) in frames.iter().enumerate() {
            let seconds = frames.get(index + 1).map_or(LAST_FRAME_SECONDS, |next| {
                (next.ms - frame.ms) / MS_PER_SECOND
            });
            let _ = write!(
                script,
                "file '{}'\nduration {seconds:.3}\n",
                Self::absolute(&frame.file).display()
            );
        }
        // The concat demuxer ignores the duration of the last entry unless it is repeated.
        if let Some(last) = frames.last() {
            let _ = writeln!(script, "file '{}'", Self::absolute(&last.file).display());
        }
        script
    }

    /// `path` made absolute, because `ffmpeg` reads the paths in a list relative to the
    /// list's own folder, not to the working directory.
    fn absolute(path: &Path) -> PathBuf {
        std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
    }

    /// The `ffmpeg` command that encodes the frames listed in `list` into `mp4`.
    pub(crate) fn command(list: &Path, mp4: &Path) -> Command {
        let mut command = Command::new("ffmpeg");
        command
            .args(["-y", "-f", "concat", "-safe", "0", "-i"])
            .arg(list)
            .args([
                "-vf",
                "scale=trunc(iw/2)*2:trunc(ih/2)*2",
                "-fps_mode",
                "vfr",
            ])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .args(["-movflags", "+faststart"])
            .arg(mp4)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELATIVE_FRAME: &str = "out/.recording-frames/frame000000.jpg";

    #[test]
    fn the_frame_list_names_frames_by_absolute_path() {
        let list = Encoder::frame_list(&[Frame {
            file: PathBuf::from(RELATIVE_FRAME),
            ms: 0.0,
        }]);
        let expected =
            std::path::absolute(std::env::current_dir().unwrap().join(RELATIVE_FRAME)).unwrap();
        assert!(list.contains(&format!("file '{}'", expected.display())));
    }
}
