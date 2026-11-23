//! Asynchronous capture: backends that drive their processes with `tokio::process` and
//! wait with `tokio::time`.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once. [`run_plan`] drives
//! a session from a declarative [`crate::session::Plan`], refusing up front a plan the
//! backend cannot honour unless the plan allows a degraded run. Every process is started
//! with `kill_on_drop`, so a session that is dropped without `finish` still cleans up.

pub mod annotate;
mod cdp;
#[cfg(unix)]
mod gtk_broadway;
#[cfg(test)]
mod tests;
mod tool;
mod web;

pub use crate::backend::{AndroidAdbBackend, CdpBackend, WebBackend};
#[cfg(unix)]
pub use crate::backend::{GtkBroadwayBackend, LinuxWaylandBackend, LinuxX11Backend};
pub use crate::common::runner::asynchronous::run_plan;
pub use crate::common::traits::asynchronous::{CaptureBackend, CaptureSession};
pub use annotate::{annotate_png, annotate_video, mp4_to_gif};
pub use cdp::CdpDiscovery;
pub use web::WebDiscovery;

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
