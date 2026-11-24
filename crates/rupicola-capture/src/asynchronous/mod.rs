//! Asynchronous capture: backends that drive their processes with `tokio::process` and
//! wait with `tokio::time`.
//!
//! A [`CaptureBackend`] starts a [`CaptureSession`] for a target once. [`run_plan`] drives
//! a session from a declarative [`crate::session::Plan`], refusing up front a plan the
//! backend cannot honour unless the plan allows a degraded run. Every process is started
//! with `kill_on_drop`, so a session that is dropped without `finish` still cleans up.

pub use crate::backend::{AndroidAdbBackend, CdpBackend, WebBackend};
#[cfg(unix)]
pub use crate::backend::{GtkBroadwayBackend, LinuxWaylandBackend, LinuxX11Backend};
pub use crate::common::annotate::asynchronous::{annotate_png, annotate_video, mp4_to_gif};
pub use crate::common::cdp::asynchronous::CdpDiscovery;
pub use crate::common::probe::asynchronous::probe_environment;
pub use crate::common::runner::asynchronous::run_plan;
pub use crate::common::traits::asynchronous::{CaptureBackend, CaptureSession};
pub use crate::common::web::asynchronous::WebDiscovery;
