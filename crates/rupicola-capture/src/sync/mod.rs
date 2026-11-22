//! Blocking capture: backends that drive their processes with `std::process` and wait
//! with `std::thread::sleep`.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once. [`run_plan`] drives
//! a session from a declarative [`crate::session::Plan`], refusing up front a plan the backend cannot
//! honour unless the plan allows a degraded run.

use crate::session::{
    Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use rupicola_protocol::UiAction;
use std::path::Path;

mod android_adb;
pub mod annotate;
mod cdp;
#[cfg(unix)]
mod gtk_broadway;
#[cfg(unix)]
mod linux_wayland;
#[cfg(unix)]
mod linux_x11;
mod runner;
#[cfg(test)]
mod tests;
mod tool;
mod web;
#[cfg(unix)]
mod xvfb;

pub use crate::backend::{AndroidAdbBackend, CdpBackend, WebBackend};
#[cfg(unix)]
pub use crate::backend::{GtkBroadwayBackend, LinuxWaylandBackend, LinuxX11Backend};
pub use annotate::{annotate_png, annotate_video, mp4_to_gif};
pub use cdp::CdpDiscovery;
pub use runner::run_plan;
pub use web::WebDiscovery;
#[cfg(unix)]
pub use xvfb::XvfbSession;

/// A running capture of one target.
pub trait CaptureSession {
    /// # Errors
    ///
    /// Fails when the target does not accept the action.
    fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the condition is not met in time.
    fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the frame cannot be grabbed.
    fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError>;

    /// # Errors
    ///
    /// Fails when recording cannot start.
    fn start_recording(&mut self) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when no recording is running or it cannot be finished.
    fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError>;

    /// Stops the target and everything started for it. Called exactly once, even after
    /// a failed step.
    ///
    /// # Errors
    ///
    /// Fails when cleanup does not complete.
    fn finish(self: Box<Self>) -> Result<(), CaptureError>;
}

/// A way of capturing: knows what it can do and how to start a [`CaptureSession`].
pub trait CaptureBackend {
    fn id(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    /// Launches `target` in `environment`; files go under `out`.
    ///
    /// # Errors
    ///
    /// [`ErrorClass::Environment`] when tools are missing, [`ErrorClass::Target`] when
    /// the target does not come up.
    fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError>;
}
