//! Headless capture backends and visual annotation engine built on `rupicola-protocol`.

pub mod annotate;
pub mod backend;
#[cfg(unix)]
pub mod linux_x11;

pub use annotate::annotate_png;
pub use backend::{CaptureBackend, CaptureOutput, CaptureRequest};
#[cfg(unix)]
pub use linux_x11::LinuxX11Backend;
