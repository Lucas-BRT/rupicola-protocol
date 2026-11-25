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
/// The concat image demuxer defaults to 25 fps; use millisecond precision for frame durations.
const CONCAT_INPUT_FRAMERATE: &str = "1000";
/// Environment variable that names a Playwright module directory.
const PLAYWRIGHT_DIR_ENV: &str = "RUPICOLA_PLAYWRIGHT_DIR";
const SESSION_DRIVER: &str = include_str!("../playwright_session_driver.js");
/// Browser engines supported by `WebBackend` via Playwright.
pub const SUPPORTED_ENGINES: &[&str] = &["chromium", "firefox", "webkit"];

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
    /// Whether `engine` is a Playwright web engine supported by [`WebBackend`].
    #[must_use]
    pub fn is_supported_engine(engine: &str) -> bool {
        SUPPORTED_ENGINES.contains(&engine)
    }

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
                "file '{}'\noption framerate {CONCAT_INPUT_FRAMERATE}\nduration {seconds:.3}\n",
                Self::absolute(&frame.file).display()
            );
        }
        // The concat demuxer ignores the duration of the last entry unless it is repeated.
        if let Some(last) = frames.last() {
            let _ = writeln!(
                script,
                "file '{}'\noption framerate {CONCAT_INPUT_FRAMERATE}",
                Self::absolute(&last.file).display()
            );
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

    #[test]
    fn encoding_preserves_sixteen_millisecond_frame_timestamps() {
        use std::{
            fs,
            process::Command,
            time::{SystemTime, UNIX_EPOCH},
        };

        if Command::new("ffmpeg").arg("-version").output().is_err()
            || Command::new("ffprobe").arg("-version").output().is_err()
        {
            return;
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "rupicola-frame-timing-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();

        let frames: Vec<_> = (0_u8..4)
            .map(|index| {
                let file = dir.join(format!("frame-{index}.ppm"));
                let mut ppm = b"P6\n2 2\n255\n".to_vec();
                for _ in 0..4 {
                    ppm.extend_from_slice(&[index * 60, 0, 0]);
                }
                fs::write(&file, ppm).unwrap();
                Frame {
                    file,
                    ms: f64::from(index) * 16.0,
                }
            })
            .collect();
        let list = dir.join(FRAME_LIST);
        let mp4 = dir.join("clip.mp4");
        fs::write(&list, Encoder::frame_list(&frames)).unwrap();

        let status = Encoder::command(&list, &mp4).status().unwrap();
        assert!(status.success(), "ffmpeg exited with {status}");

        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "frame=best_effort_timestamp_time",
                "-of",
                "csv=p=0",
            ])
            .arg(&mp4)
            .output()
            .unwrap();
        assert!(
            probe.status.success(),
            "ffprobe failed: {}",
            String::from_utf8_lossy(&probe.stderr)
        );
        let timestamps: Vec<f64> = String::from_utf8(probe.stdout)
            .unwrap()
            .lines()
            .map(|line| line.split(',').next().unwrap().parse().unwrap())
            .collect();
        assert!(
            timestamps.len() >= frames.len(),
            "expected all source frames, got {}",
            timestamps.len()
        );
        for pair in timestamps.windows(2).take(frames.len() - 1) {
            let interval = pair[1] - pair[0];
            assert!(
                (interval - 0.016).abs() <= 0.001,
                "expected 16 ms PTS spacing, got {interval:.3} s"
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
