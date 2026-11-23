//! What the shared code needs from the runtime, blocking flavour. `rt_async` has the same
//! names; the shared code is written once, async-first, and this is where it blocks.

use crate::backend::tool::Tool;
use crate::session::CaptureError;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus, Output};
use std::thread;
use std::time::Duration;

/// A process started for a session.
pub(crate) struct Child(std::process::Child);

impl Child {
    /// Stops the process; there is nothing to do when it already ended.
    pub(crate) fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }

    pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
        self.0.wait()
    }
}

/// Runs, waits and touches files the blocking way.
pub(crate) struct Runtime;

impl Runtime {
    pub(crate) fn spawn(mut command: Command) -> io::Result<Child> {
        command.spawn().map(Child)
    }

    pub(crate) fn status(mut command: Command) -> io::Result<ExitStatus> {
        command.status()
    }

    pub(crate) fn output(mut command: Command) -> io::Result<Output> {
        command.output()
    }

    pub(crate) fn sleep(ms: u64) {
        thread::sleep(Duration::from_millis(ms));
    }

    /// # Errors
    ///
    /// An environment error naming the program when it is not on `PATH`.
    pub(crate) fn require(tool: Tool) -> Result<(), CaptureError> {
        if tool.probe().status().is_ok_and(|status| status.success()) {
            Ok(())
        } else {
            Err(tool.missing())
        }
    }

    pub(crate) fn create_dir_all(path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
        std::fs::write(path, contents)
    }

    pub(crate) fn exists(path: &Path) -> bool {
        path.exists()
    }
}
