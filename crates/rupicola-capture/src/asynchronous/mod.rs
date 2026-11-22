//! Asynchronous capture: backends that drive their processes with `tokio::process` and
//! wait with `tokio::time`.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once. [`run_plan`] drives
//! a session from a declarative [`crate::session::Plan`], refusing up front a plan the
//! backend cannot honour unless the plan allows a degraded run. Every process is started
//! with `kill_on_drop`, so a session that is dropped without `finish` still cleans up.

use crate::session::{
    Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
};
use async_trait::async_trait;
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

/// A std command, ready to run on tokio and to be killed when dropped.
pub(crate) trait IntoTokio {
    fn into_tokio(self) -> tokio::process::Command;
}

impl IntoTokio for std::process::Command {
    fn into_tokio(self) -> tokio::process::Command {
        let mut command = tokio::process::Command::from(self);
        command.kill_on_drop(true);
        command
    }
}

/// A running capture of one target.
#[async_trait]
pub trait CaptureSession: Send {
    /// # Errors
    ///
    /// Fails when the target does not accept the action.
    async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the condition is not met in time.
    async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when the frame cannot be grabbed.
    async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError>;

    /// # Errors
    ///
    /// Fails when recording cannot start.
    async fn start_recording(&mut self) -> Result<(), CaptureError>;

    /// # Errors
    ///
    /// Fails when no recording is running or it cannot be finished.
    async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError>;

    /// Stops the target and everything started for it. Called exactly once, even after
    /// a failed step.
    ///
    /// # Errors
    ///
    /// Fails when cleanup does not complete.
    async fn finish(self: Box<Self>) -> Result<(), CaptureError>;
}

/// A way of capturing: knows what it can do and how to start a [`CaptureSession`].
#[async_trait]
pub trait CaptureBackend: Send + Sync {
    fn id(&self) -> &'static str;

    fn capabilities(&self) -> Capabilities;

    /// Launches `target` in `environment`; files go under `out`.
    ///
    /// # Errors
    ///
    /// [`crate::session::ErrorClass::Environment`] when tools are missing,
    /// [`crate::session::ErrorClass::Target`] when the target does not come up.
    async fn start(
        &self,
        target: &Target,
        environment: &Environment,
        out: &Path,
    ) -> Result<Box<dyn CaptureSession>, CaptureError>;
}
