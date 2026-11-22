//! Headless capture backends and visual annotation engine built on `rupicola-protocol`.

pub mod annotate;
pub mod backend;
#[cfg(unix)]
pub mod linux_wayland;
#[cfg(unix)]
pub mod linux_x11;
pub mod web;

pub use annotate::{annotate_png, annotate_video, mp4_to_gif};
pub use backend::{CaptureBackend, CaptureOutput, CaptureRequest};
#[cfg(unix)]
pub use linux_wayland::LinuxWaylandBackend;
#[cfg(unix)]
pub use linux_x11::LinuxX11Backend;
pub use web::WebBackend;
