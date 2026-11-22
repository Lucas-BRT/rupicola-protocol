use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

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
