use crate::message::{ProtocolRequest, ProtocolResponse};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(unix)]
use std::path::Path;

/// Writes one JSON-line message and flushes the stream.
///
/// # Errors
///
/// Returns an error if serialization or writing to `writer` fails.
pub fn write_ndjson<W: Write, T: Serialize>(writer: &mut W, msg: &T) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, msg)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    writer.write_all(b"\n")?;
    writer.flush()
}

/// Reads one JSON-line message from `reader`. Returns `Ok(None)` at EOF.
///
/// # Errors
///
/// Returns an error if reading fails or the line is invalid JSON.
pub fn read_ndjson<R: BufRead, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> io::Result<Option<T>> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Ok(None);
    }
    let parsed = serde_json::from_str(line.trim_end())
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    Ok(Some(parsed))
}

/// Synchronous NDJSON client over a Unix domain socket.
#[cfg(unix)]
pub struct UnixProtocolClient {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
}

#[cfg(unix)]
impl UnixProtocolClient {
    /// Connects to a middleware socket listening at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if connecting or cloning the socket handle fails.
    pub fn connect(path: &Path) -> io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            writer: stream,
            reader,
        })
    }

    /// Sends one [`ProtocolRequest`] and waits for the matching [`ProtocolResponse`].
    ///
    /// # Errors
    ///
    /// Returns an error on I/O failure, unexpected EOF, or malformed JSON.
    pub fn call(&mut self, request: &ProtocolRequest) -> io::Result<ProtocolResponse> {
        write_ndjson(&mut self.writer, request)?;
        read_ndjson(&mut self.reader)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "socket closed"))
    }
}

/// Accepts one client connection on `socket_path` and dispatches requests to `handler`.
///
/// # Errors
///
/// Returns an error if binding or handling the stream fails.
#[cfg(unix)]
pub fn serve_once_unix<F>(socket_path: &Path, mut handler: F) -> io::Result<()>
where
    F: FnMut(ProtocolRequest) -> ProtocolResponse,
{
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    let (stream, _) = listener.accept()?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);
    while let Some(req) = read_ndjson::<_, ProtocolRequest>(&mut reader)? {
        let resp = handler(req);
        write_ndjson(&mut writer, &resp)?;
    }
    Ok(())
}
