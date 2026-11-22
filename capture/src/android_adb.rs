use crate::annotate::finalize_media;
use crate::backend::{CaptureBackend, CaptureOutput, CaptureRequest};
use std::fs;
use std::process::{Command, Stdio};

/// Mobile Android capture backend over `adb` (`screencap` + `screenrecord`).
#[derive(Clone, Debug, Default)]
pub struct AndroidAdbBackend {
    /// Optional device serial (`adb -s <serial>`).
    pub serial: Option<String>,
}

impl AndroidAdbBackend {
    fn adb_cmd(&self) -> Command {
        let mut cmd = Command::new("adb");
        if let Some(ref serial) = self.serial {
            cmd.args(["-s", serial]);
        }
        cmd
    }
}

impl CaptureBackend for AndroidAdbBackend {
    fn name(&self) -> &'static str {
        "mobile-android-adb"
    }

    fn capture(&self, request: &CaptureRequest) -> Result<CaptureOutput, String> {
        let has_adb = Command::new("which")
            .arg("adb")
            .output()
            .is_ok_and(|o| o.status.success());
        if !has_adb {
            return Err("adb not found in PATH; install Android Platform-Tools".to_owned());
        }

        fs::create_dir_all(&request.output_dir)
            .map_err(|e| format!("failed to create output dir: {e}"))?;

        let screenshot_png = request.output_dir.join("preview.png");
        let out = self
            .adb_cmd()
            .args(["exec-out", "screencap", "-p"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| format!("failed to execute adb screencap: {e}"))?;
        if !out.status.success() || out.stdout.is_empty() {
            return Err(format!(
                "adb screencap failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        fs::write(&screenshot_png, &out.stdout)
            .map_err(|e| format!("failed to write android screenshot: {e}"))?;

        let video_mp4 = if let Some(ms) = request.record_video_ms.filter(|&m| m > 0) {
            let secs = (ms / 1000).clamp(1, 180).to_string();
            let remote = "/sdcard/rupicola-walkthrough.mp4";
            let local = request.output_dir.join("walkthrough.mp4");
            let rec = self
                .adb_cmd()
                .args(["shell", "screenrecord", "--time-limit", &secs, remote])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let pulled = rec.is_ok_and(|s| s.success())
                && self
                    .adb_cmd()
                    .args(["pull", remote])
                    .arg(&local)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
            let _ = self
                .adb_cmd()
                .args(["shell", "rm", "-f", remote])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            pulled.then_some(local)
        } else {
            None
        };

        finalize_media(
            &request.output_dir,
            screenshot_png,
            video_mp4,
            &request.annotations,
            None,
        )
    }
}
