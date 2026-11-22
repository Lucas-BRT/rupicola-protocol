use crate::session::{
    self, Capabilities, Capability, CaptureError, Checkpoint, Clip, Environment, Target,
    WaitCondition,
};
use crate::tool::Tool;
use rupicola_protocol::UiAction;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

const ADB: &str = "adb";
/// Where the device keeps a recording until it is pulled.
const REMOTE_RECORDING: &str = "/sdcard/rupicola-walkthrough.mp4";
/// Longest clip `screenrecord` allows, in seconds.
const MAX_RECORDING_SECONDS: &str = "180";
/// Signal that makes `screenrecord` finish and close its file.
const INTERRUPT_SIGNAL: &str = "-2";
/// Program on the device that is asked to stop.
const SCREENRECORD: &str = "screenrecord";
/// Time the device gets to write the end of a recording after the interrupt.
const RECORDING_FLUSH: Duration = Duration::from_millis(500);

/// Mobile Android capture backend over `adb` (`screencap` + `screenrecord`).
///
/// It captures whatever the device shows; the session's target is not launched.
#[derive(Clone, Debug, Default)]
pub struct AndroidAdbBackend {
    /// Optional device serial (`adb -s <serial>`).
    pub serial: Option<String>,
}

impl AndroidAdbBackend {
    fn adb_cmd(&self) -> Command {
        let mut cmd = Command::new(ADB);
        if let Some(ref serial) = self.serial {
            cmd.args(["-s", serial]);
        }
        cmd
    }
}

impl session::CaptureBackend for AndroidAdbBackend {
    fn id(&self) -> &'static str {
        "mobile-android-adb"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::of(&[Capability::Video])
    }

    fn start(
        &self,
        _target: &Target,
        _environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn session::CaptureSession>, CaptureError> {
        Tool(ADB).require()?;
        let state = self
            .adb_cmd()
            .arg("get-state")
            .output()
            .map_err(|e| CaptureError::environment(format!("run {ADB}: {e}")))?;
        if !state.status.success() {
            return Err(CaptureError::environment(format!(
                "no Android device: {}",
                String::from_utf8_lossy(&state.stderr).trim()
            )));
        }
        fs::create_dir_all(out)
            .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
        Ok(Box::new(AndroidSession {
            backend: self.clone(),
            out: out.to_path_buf(),
            recording: None,
        }))
    }
}

struct AndroidSession {
    backend: AndroidAdbBackend,
    out: PathBuf,
    recording: Option<Child>,
}

impl AndroidSession {
    fn unsupported() -> CaptureError {
        CaptureError::environment(
            "this backend cannot act on the device: only wait-ms is supported",
        )
    }

    /// Runs an `adb` command, hiding its output.
    fn run(&self, args: &[&str]) -> Result<bool, CaptureError> {
        self.backend
            .adb_cmd()
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .map_err(|e| CaptureError::environment(format!("run {ADB}: {e}")))
    }

    fn discard_recording(&mut self) {
        if let Some(mut child) = self.recording.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = self.run(&["shell", "rm", "-f", REMOTE_RECORDING]);
    }
}

impl session::CaptureSession for AndroidSession {
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
        match action {
            UiAction::WaitMs { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            _ => Err(Self::unsupported()),
        }
    }

    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
        match condition {
            WaitCondition::Delay { ms } => {
                thread::sleep(Duration::from_millis(*ms));
                Ok(())
            }
            WaitCondition::Visible { .. } | WaitCondition::Text { .. } => Err(
                CaptureError::environment("this backend has no UI tree to wait on"),
            ),
        }
    }

    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
        let png = self.out.join(format!("{}.png", session::checked_id(id)?));
        let output = self
            .backend
            .adb_cmd()
            .args(["exec-out", "screencap", "-p"])
            .output()
            .map_err(|e| CaptureError::environment(format!("run {ADB} screencap: {e}")))?;
        if !output.status.success() || output.stdout.is_empty() {
            return Err(CaptureError::target(format!(
                "adb screencap failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        fs::write(&png, &output.stdout)
            .map_err(|e| CaptureError::environment(format!("write {}: {e}", png.display())))?;
        Ok(Checkpoint {
            id: id.to_owned(),
            png,
            snapshot: None,
        })
    }

    fn start_recording(&mut self) -> Result<(), CaptureError> {
        if self.recording.is_some() {
            return Err(CaptureError::environment("a recording is already running"));
        }
        let child = self
            .backend
            .adb_cmd()
            .args([
                "shell",
                SCREENRECORD,
                "--time-limit",
                MAX_RECORDING_SECONDS,
                REMOTE_RECORDING,
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CaptureError::environment(format!("run {ADB} screenrecord: {e}")))?;
        self.recording = Some(child);
        Ok(())
    }

    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
        let mp4 = self.out.join(format!("{}.mp4", session::checked_id(id)?));
        let Some(mut child) = self.recording.take() else {
            return Err(CaptureError::environment("no recording is running"));
        };
        let stopped = self.run(&["shell", "pkill", INTERRUPT_SIGNAL, SCREENRECORD])?;
        let _ = child.wait();
        thread::sleep(RECORDING_FLUSH);
        let pulled = stopped && {
            let remote = REMOTE_RECORDING;
            self.backend
                .adb_cmd()
                .args(["pull", remote])
                .arg(&mp4)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        };
        let _ = self.run(&["shell", "rm", "-f", REMOTE_RECORDING]);
        if pulled && mp4.is_file() {
            Ok(Clip {
                id: id.to_owned(),
                mp4,
            })
        } else {
            Err(CaptureError::target("the recording could not be retrieved"))
        }
    }

    fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
        self.discard_recording();
        Ok(())
    }
}
