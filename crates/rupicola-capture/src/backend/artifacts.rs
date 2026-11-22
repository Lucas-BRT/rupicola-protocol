use crate::session::{self, CaptureError};
use std::path::{Path, PathBuf};

/// Where a session writes its files, named by checked ids.
#[derive(Clone, Debug)]
pub(crate) struct Artifacts {
    out: PathBuf,
}

impl Artifacts {
    pub(crate) fn new(out: &Path) -> Self {
        Self {
            out: out.to_path_buf(),
        }
    }

    /// The output directory.
    pub(crate) fn dir(&self) -> &Path {
        &self.out
    }

    /// The file for a screenshot called `id`.
    ///
    /// # Errors
    ///
    /// When `id` is not a safe file name.
    pub(crate) fn png(&self, id: &str) -> Result<PathBuf, CaptureError> {
        Ok(self.out.join(format!("{}.png", session::checked_id(id)?)))
    }

    /// The file for a clip called `id`.
    ///
    /// # Errors
    ///
    /// When `id` is not a safe file name.
    pub(crate) fn mp4(&self, id: &str) -> Result<PathBuf, CaptureError> {
        Ok(self.out.join(format!("{}.mp4", session::checked_id(id)?)))
    }

    /// The file inside the output directory called `name`.
    pub(crate) fn file(&self, name: &str) -> PathBuf {
        self.out.join(name)
    }
}
