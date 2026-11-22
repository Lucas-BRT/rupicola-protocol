//! Error messages both flavours of the backends report.

pub(crate) const RECORDING_RUNNING: &str = "a recording is already running";
pub(crate) const NO_RECORDING: &str = "no recording is running";
pub(crate) const NO_FRAMES: &str = "the recording has no frames";
pub(crate) const DRIVER_NO_PIPES: &str = "the driver has no pipes";
pub(crate) const DRIVER_EXITED: &str = "the driver exited unexpectedly";
pub(crate) const NO_UI_TREE_WAIT: &str = "this backend has no UI tree to wait on";
