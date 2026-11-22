//! Headless capture backends and visual annotation engine built on `rupicola-protocol`.

pub mod android_adb;
pub mod annotate;
pub mod backend;
pub mod cdp;
#[cfg(unix)]
pub mod gtk_broadway;
#[cfg(unix)]
pub mod linux_wayland;
#[cfg(unix)]
pub mod linux_x11;
pub mod session;
pub mod web;

pub use android_adb::AndroidAdbBackend;
pub use annotate::{annotate_png, annotate_video, mp4_to_gif};
pub use backend::{CaptureBackend, CaptureOutput, CaptureRequest};
pub use cdp::CdpBackend;
#[cfg(unix)]
pub use gtk_broadway::GtkBroadwayBackend;
#[cfg(unix)]
pub use linux_wayland::LinuxWaylandBackend;
#[cfg(unix)]
pub use linux_x11::LinuxX11Backend;
pub use web::WebBackend;
