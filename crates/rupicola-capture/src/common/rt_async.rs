//! What the shared code needs from the runtime, tokio flavour. `rt_sync` has the same
//! names; the shared code is written once, async-first, and this is where it awaits.

use crate::backend::tool::Tool;
use crate::session::CaptureError;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::time::Duration;

/// A process started for a session; it is killed when dropped.
pub(crate) struct Child(tokio::process::Child);

impl Child {
    /// Stops the process; there is nothing to do when it already ended.
    pub(crate) async fn kill(&mut self) {
        let _ = self.0.kill().await;
    }

    pub(crate) async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.0.wait().await
    }
}

/// A std command, ready to run on tokio and to be killed when dropped.
fn tokio_command(command: Command) -> tokio::process::Command {
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    command
}

/// Runs, waits and touches files on tokio.
pub(crate) struct Runtime;

impl Runtime {
    pub(crate) fn spawn(command: Command) -> io::Result<Child> {
        tokio_command(command).spawn().map(Child)
    }

    pub(crate) async fn status(command: Command) -> io::Result<ExitStatus> {
        tokio_command(command).status().await
    }

    pub(crate) async fn output(command: Command) -> io::Result<Output> {
        tokio_command(command).output().await
    }

    pub(crate) async fn sleep(ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    /// # Errors
    ///
    /// An environment error naming the program when it is not on `PATH`.
    pub(crate) async fn require(tool: Tool) -> Result<(), CaptureError> {
        if tokio_command(tool.probe())
            .status()
            .await
            .is_ok_and(|status| status.success())
        {
            Ok(())
        } else {
            Err(tool.missing())
        }
    }

    pub(crate) async fn create_dir_all(path: &Path) -> io::Result<()> {
        tokio::fs::create_dir_all(path).await
    }

    pub(crate) async fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
        tokio::fs::write(path, contents).await
    }

    pub(crate) async fn exists(path: &Path) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
    }
}
