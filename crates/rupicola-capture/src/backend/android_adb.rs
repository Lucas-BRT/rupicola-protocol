use std::path::Path;
use std::process::{Command, Stdio};

pub(crate) const ADB: &str = "adb";
/// Where the device keeps a recording until it is pulled.
const REMOTE_RECORDING: &str = "/sdcard/rupicola-walkthrough.mp4";
/// Longest clip `screenrecord` allows, in seconds.
const MAX_RECORDING_SECONDS: &str = "180";
/// Signal that makes `screenrecord` finish and close its file.
const INTERRUPT_SIGNAL: &str = "-2";
/// Program on the device that is asked to stop.
const SCREENRECORD: &str = "screenrecord";
/// Milliseconds the device gets to write the end of a recording after the interrupt.
pub(crate) const RECORDING_FLUSH_MS: u64 = 500;

/// Mobile Android capture backend over `adb` (`screencap` + `screenrecord`).
///
/// It captures whatever the device shows; the session's target is not launched.
#[derive(Clone, Debug, Default)]
pub struct AndroidAdbBackend {
    /// Optional device serial (`adb -s <serial>`).
    pub serial: Option<String>,
}

impl AndroidAdbBackend {
    /// `adb`, aimed at the configured device, with its output discarded.
    fn adb(&self) -> Command {
        let mut command = Command::new(ADB);
        if let Some(ref serial) = self.serial {
            command.args(["-s", serial]);
        }
        command.stdout(Stdio::null()).stderr(Stdio::null());
        command
    }

    /// Asks whether a device is attached; stdout and stderr are left to the caller.
    pub(crate) fn get_state(&self) -> Command {
        let mut command = self.adb();
        command
            .arg("get-state")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Takes a screenshot; the PNG comes on stdout.
    pub(crate) fn screencap(&self) -> Command {
        let mut command = self.adb();
        command
            .args(["exec-out", "screencap", "-p"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Records the screen until interrupted.
    pub(crate) fn record(&self) -> Command {
        let mut command = self.adb();
        command.args([
            "shell",
            SCREENRECORD,
            "--time-limit",
            MAX_RECORDING_SECONDS,
            REMOTE_RECORDING,
        ]);
        command
    }

    /// Interrupts the recording so the device closes its file.
    pub(crate) fn stop_record(&self) -> Command {
        let mut command = self.adb();
        command.args(["shell", "pkill", INTERRUPT_SIGNAL, SCREENRECORD]);
        command
    }

    /// Copies the recording to `local`.
    pub(crate) fn pull(&self, local: &Path) -> Command {
        let mut command = self.adb();
        command.args(["pull", REMOTE_RECORDING]).arg(local);
        command
    }

    /// Deletes the recording from the device.
    pub(crate) fn remove(&self) -> Command {
        let mut command = self.adb();
        command.args(["shell", "rm", "-f", REMOTE_RECORDING]);
        command
    }
}
