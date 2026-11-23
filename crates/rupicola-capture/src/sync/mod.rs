//! Blocking capture: backends that drive their processes with `std::process` and wait
//! with `std::thread::sleep`.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once. [`run_plan`] drives
//! a session from a declarative [`crate::session::Plan`], refusing up front a plan the backend cannot
//! honour unless the plan allows a degraded run.

pub mod annotate;
mod cdp;
#[cfg(unix)]
mod gtk_broadway;
#[cfg(unix)]
mod linux_wayland;
#[cfg(unix)]
mod linux_x11;
#[cfg(test)]
mod tests;
mod tool;
mod web;
#[cfg(unix)]
mod xvfb;

pub use crate::backend::{AndroidAdbBackend, CdpBackend, WebBackend};
#[cfg(unix)]
pub use crate::backend::{GtkBroadwayBackend, LinuxWaylandBackend, LinuxX11Backend};
pub use crate::common::runner::sync::run_plan;
pub use crate::common::traits::sync::{CaptureBackend, CaptureSession};
pub use annotate::{annotate_png, annotate_video, mp4_to_gif};
pub use cdp::CdpDiscovery;
pub use web::WebDiscovery;
#[cfg(unix)]
pub use xvfb::XvfbSession;
