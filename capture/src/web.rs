use crate::annotate::finalize_media;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use rupicola_protocol::UiSnapshot;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// Headless Web capture backend powered by Playwright (`chromium`, `firefox`, or `webkit`).
#[derive(Clone, Debug)]
pub struct WebBackend {
    pub browser_engine: String,
    pub node_bin: PathBuf,
    pub playwright_module: PathBuf,
}

impl WebBackend {
    /// Auto-discovers `node` and a local `playwright` module installation.
    #[must_use]
    pub fn discover(browser_engine: &str) -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let node_candidates = [
            PathBuf::from("node"),
            home.join(".local/share/mise/installs/node/lts/bin/node"),
        ];
        let node_bin = node_candidates.into_iter().find(|p| {
            (p.as_os_str() == "node" && Command::new(p).arg("--version").output().is_ok())
                || p.is_file()
        })?;
        let pw_candidates = [
            std::env::var_os("RUPICOLA_PLAYWRIGHT_DIR")
                .map(PathBuf::from)
                .unwrap_or_default(),
            home.join("Projects/pokemon-survivors/node_modules/playwright"),
        ];
        let playwright_module = pw_candidates
            .into_iter()
            .find(|p| !p.as_os_str().is_empty() && p.exists())?;
        Some(Self {
            browser_engine: browser_engine.to_owned(),
            node_bin,
            playwright_module,
        })
    }
}

const PLAYWRIGHT_DRIVER: &str = include_str!("playwright_driver.js");

impl CaptureBackend for WebBackend {
    fn name(&self) -> &'static str {
        "web-playwright"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;

        let screenshot_png = request.output_dir.join("preview.png");
        let record_video = request.record_video_ms.is_some_and(|m| m > 0);
        let tmp_video_dir = request.output_dir.join(".tmp-video");
        if record_video {
            let _ = fs::remove_dir_all(&tmp_video_dir);
            fs::create_dir_all(&tmp_video_dir)
                .map_err(|e| format!("failed to create temp video dir: {e}"))?;
        }

        let actions_json = serde_json::to_string(&request.actions)
            .map_err(|e| format!("failed to serialize actions: {e}"))?;
        let out = Command::new(&self.node_bin)
            .arg("-e")
            .arg(PLAYWRIGHT_DRIVER)
            .env("PW_DIR", &self.playwright_module)
            .env("PW_ENGINE", &self.browser_engine)
            .env("TARGET_URL", &request.command)
            .env("VP_W", request.viewport[0].to_string())
            .env("VP_H", request.viewport[1].to_string())
            .env("OUT_PNG", &screenshot_png)
            .env("ACTIONS_JSON", actions_json)
            .env(
                "VIDEO_DIR",
                if record_video {
                    tmp_video_dir.to_string_lossy().into_owned()
                } else {
                    String::new()
                },
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("failed to execute playwright driver: {e}"))?;

        if !out.status.success() {
            return Err(format!(
                "playwright driver failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }

        let parsed: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| format!("invalid driver JSON output: {e}"))?;
        let snapshot: Option<UiSnapshot> = parsed
            .get("snapshot")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok());

        let video_mp4 = if let Some(webm) = parsed.get("video_path").and_then(|v| v.as_str()) {
            let mp4_path = request.output_dir.join("walkthrough.mp4");
            let conv = Command::new("ffmpeg")
                .args([
                    "-y",
                    "-i",
                    webm,
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv420p",
                    "-movflags",
                    "+faststart",
                ])
                .arg(&mp4_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = fs::remove_dir_all(&tmp_video_dir);
            match conv {
                Ok(s) if s.success() => Some(mp4_path),
                _ => None,
            }
        } else {
            None
        };

        finalize_media(
            &request.output_dir,
            screenshot_png,
            video_mp4,
            &request.annotations,
            snapshot,
        )
    }
}
