//! What the shared code needs from the runtime, blocking flavour. `rt_async` has the same
//! names; the shared code is written once, async-first, and this is where it blocks.

use crate::backend::tool::Tool;
use crate::session::CaptureError;
use std::io::{self, BufRead as _, BufReader, Write as _};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::thread;
use std::time::Duration;

/// Maximum sleep slice when polling a blocking child pipe during recording waits.
const SYNC_READ_POLL_MS: u64 = 4;

/// Result of reading a delimited message from a child process with a timeout.
pub(crate) enum ReadMessage {
    #[allow(dead_code)]
    Message(Vec<u8>),
    Timeout,
    #[allow(dead_code)]
    Eof,
}

/// A process started for a session, with the pipes its command asked for. It is killed
/// when dropped.
pub(crate) struct Child {
    inner: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    stdout: Option<BufReader<std::process::ChildStdout>>,
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

    /// Whether both stdin and stdout are piped.
    pub(crate) const fn has_pipes(&self) -> bool {
        self.stdin.is_some() && self.stdout.is_some()
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

    /// Reads what the process wrote up to the next `delimiter`, without it; `None` at the end
    /// of the stream.
    pub(crate) fn read_message(&mut self, delimiter: u8) -> io::Result<Option<Vec<u8>>> {
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        let mut message = Vec::new();
        if stdout.read_until(delimiter, &mut message)? == 0 {
            return Ok(None);
        }
        if message.last() == Some(&delimiter) {
            message.pop();
        }
        Ok(Some(message))
    }

    /// Sleeps briefly and reports [`ReadMessage::Timeout`] so the blocking runtime can poll CDP.
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    pub(crate) fn read_message_timeout(
        &mut self,
        _delimiter: u8,
        timeout: Duration,
    ) -> io::Result<ReadMessage> {
        thread::sleep(timeout.min(Duration::from_millis(SYNC_READ_POLL_MS)));
        Ok(ReadMessage::Timeout)
    }

    /// Reads one line from the process's stdout; `None` at the end of the stream.
    pub(crate) fn read_line(&mut self) -> io::Result<Option<String>> {
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::BrokenPipe))?;
        let mut line = String::new();
        Ok((stdout.read_line(&mut line)? > 0).then_some(line))
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
            stdout: inner.stdout.take().map(BufReader::new),
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

    /// Resolves `tool` on `PATH` when available.
    pub(crate) fn resolve_tool(tool: Tool) -> Option<std::path::PathBuf> {
        let output = tool.locate().output().ok()?;
        if !output.status.success() {
            return None;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!path.is_empty()).then(|| std::path::PathBuf::from(path))
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

    /// Whether something accepts connections at `address`, trying for `timeout_ms`.
    pub(crate) fn connects(address: SocketAddr, timeout_ms: u64) -> bool {
        TcpStream::connect_timeout(&address, Duration::from_millis(timeout_ms)).is_ok()
    }

    /// Resolves `host:port` to its IP addresses.
    pub(crate) fn lookup_ips(host: &str, port: u16) -> Result<Vec<std::net::IpAddr>, CaptureError> {
        use std::net::ToSocketAddrs as _;
        let addrs = (host, port).to_socket_addrs().map_err(|err| {
            CaptureError::target(format!("failed to resolve host `{host}`: {err}"))
        })?;
        Ok(addrs.map(|addr| addr.ip()).collect())
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

    pub(crate) fn is_file(path: &Path) -> bool {
        path.is_file()
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
