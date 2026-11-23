//! What the shared code needs from the runtime, blocking flavour. `rt_async` has the same
//! names; the shared code is written once, async-first, and this is where it blocks.

use crate::backend::tool::Tool;
use crate::session::CaptureError;
use std::io::{self, Write as _};
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::Duration;

/// A process started for a session, with the pipes its command asked for. It is killed
/// when dropped.
pub(crate) struct Child {
    inner: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
}

impl Child {
    /// Stops the process; there is nothing to do when it already ended.
    pub(crate) fn kill(&mut self) {
        let _ = self.inner.kill();
        let _ = self.inner.wait();
    }

    pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
        self.inner.wait()
    }

    /// Writes `data` to the process's stdin and flushes it.
    pub(crate) fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        stdin.write_all(data)?;
        stdin.flush()
    }

    /// Closes the process's stdin.
    pub(crate) fn close_stdin(&mut self) {
        self.stdin = None;
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Runs, waits and touches files the blocking way.
pub(crate) struct Runtime;

impl Runtime {
    pub(crate) fn spawn(mut command: Command) -> io::Result<Child> {
        let mut inner = command.spawn()?;
        Ok(Child {
            stdin: inner.stdin.take(),
            inner,
        })
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

    /// Whether `tool` is on `PATH`.
    pub(crate) fn is_available(tool: Tool) -> bool {
        tool.probe().status().is_ok_and(|status| status.success())
    }

    /// # Errors
    ///
    /// An environment error naming the program when it is not on `PATH`.
    pub(crate) fn require(tool: Tool) -> Result<(), CaptureError> {
        if Self::is_available(tool) {
            Ok(())
        } else {
            Err(tool.missing())
        }
    }

    pub(crate) fn create_dir_all(path: &Path) -> io::Result<()> {
        std::fs::create_dir_all(path)
    }

    pub(crate) fn remove_dir_all(path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    pub(crate) fn rename(from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    pub(crate) fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
        std::fs::write(path, contents)
    }

    pub(crate) fn exists(path: &Path) -> bool {
        path.exists()
    }

    /// The names of the entries of `dir`; empty when it cannot be read.
    pub(crate) fn file_names(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect()
    }

    /// Restricts `path` to its owner (mode 0700 on Unix).
    pub(crate) fn set_private(path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(())
        }
    }

    /// A new file at `path`, as the stdout or stderr of a process.
    pub(crate) fn log_file(path: &Path) -> io::Result<Stdio> {
        std::fs::File::create(path).map(Stdio::from)
    }
}
