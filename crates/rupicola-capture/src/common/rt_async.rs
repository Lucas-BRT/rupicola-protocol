//! What the shared code needs from the runtime, tokio flavour. `rt_sync` has the same
//! names; the shared code is written once, async-first, and this is where it awaits.

use crate::backend::tool::Tool;
use crate::session::CaptureError;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

/// A process started for a session, with the pipes its command asked for. It is killed
/// when dropped.
pub(crate) struct Child {
    inner: tokio::process::Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout: Option<BufReader<tokio::process::ChildStdout>>,
}

impl Child {
    /// Stops the process; there is nothing to do when it already ended.
    pub(crate) async fn kill(&mut self) {
        let _ = self.inner.kill().await;
    }

    pub(crate) async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.inner.wait().await
    }

    /// Whether both stdin and stdout are piped.
    pub(crate) const fn has_pipes(&self) -> bool {
        self.stdin.is_some() && self.stdout.is_some()
    }

    /// Writes `data` to the process's stdin and flushes it.
    pub(crate) async fn write(&mut self, data: &[u8]) -> io::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        stdin.write_all(data).await?;
        stdin.flush().await
    }

    /// Closes the process's stdin.
    pub(crate) fn close_stdin(&mut self) {
        self.stdin = None;
    }

    /// Reads what the process wrote up to the next `delimiter`, without it; `None` at the end
    /// of the stream.
    pub(crate) async fn read_message(&mut self, delimiter: u8) -> io::Result<Option<Vec<u8>>> {
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        let mut message = Vec::new();
        if stdout.read_until(delimiter, &mut message).await? == 0 {
            return Ok(None);
        }
        if message.last() == Some(&delimiter) {
            message.pop();
        }
        Ok(Some(message))
    }

    /// Reads one line from the process's stdout; `None` at the end of the stream.
    pub(crate) async fn read_line(&mut self) -> io::Result<Option<String>> {
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        let mut line = String::new();
        Ok((stdout.read_line(&mut line).await? > 0).then_some(line))
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
        let mut inner = tokio_command(command).spawn()?;
        Ok(Child {
            stdin: inner.stdin.take(),
            stdout: inner.stdout.take().map(BufReader::new),
            inner,
        })
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

    /// Whether `tool` is on `PATH`.
    pub(crate) async fn is_available(tool: Tool) -> bool {
        Self::status(tool.probe())
            .await
            .is_ok_and(|status| status.success())
    }

    /// # Errors
    ///
    /// An environment error naming the program when it is not on `PATH`.
    pub(crate) async fn require(tool: Tool) -> Result<(), CaptureError> {
        if Self::is_available(tool).await {
            Ok(())
        } else {
            Err(tool.missing())
        }
    }

    /// Whether something accepts connections at `address`, trying for `timeout_ms`.
    pub(crate) async fn connects(address: SocketAddr, timeout_ms: u64) -> bool {
        matches!(
            tokio::time::timeout(
                Duration::from_millis(timeout_ms),
                tokio::net::TcpStream::connect(address)
            )
            .await,
            Ok(Ok(_))
        )
    }

    pub(crate) async fn create_dir_all(path: &Path) -> io::Result<()> {
        tokio::fs::create_dir_all(path).await
    }

    pub(crate) async fn remove_dir_all(path: &Path) -> io::Result<()> {
        tokio::fs::remove_dir_all(path).await
    }

    pub(crate) async fn rename(from: &Path, to: &Path) -> io::Result<()> {
        tokio::fs::rename(from, to).await
    }

    pub(crate) async fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
        tokio::fs::write(path, contents).await
    }

    pub(crate) async fn exists(path: &Path) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
    }

    pub(crate) async fn is_file(path: &Path) -> bool {
        tokio::fs::metadata(path)
            .await
            .is_ok_and(|metadata| metadata.is_file())
    }

    /// The names of the entries of `dir`; empty when it cannot be read.
    pub(crate) async fn file_names(dir: &Path) -> Vec<String> {
        let mut names = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(name) = entry.file_name().into_string() {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Restricts `path` to its owner (mode 0700 on Unix).
    pub(crate) async fn set_private(path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok(())
        }
    }

    /// A new file at `path`, as the stdout or stderr of a process.
    pub(crate) async fn log_file(path: &Path) -> io::Result<Stdio> {
        let file = tokio::fs::File::create(path).await?;
        Ok(Stdio::from(file.into_std().await))
    }
}
