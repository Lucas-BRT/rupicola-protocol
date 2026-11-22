//! What every flavour of the backends shares: their configuration, the commands they
//! start and the logic around them. Only the waiting differs, and it lives in
//! [`crate::sync`] and [`crate::asynchronous`].

pub(crate) mod android_adb;
pub(crate) mod annotate;
pub(crate) mod artifacts;
pub(crate) mod cdp;
#[cfg(unix)]
pub(crate) mod gtk_broadway;
#[cfg(unix)]
pub(crate) mod linux_wayland;
#[cfg(unix)]
pub(crate) mod linux_x11;
pub(crate) mod messages;
pub(crate) mod tool;
pub(crate) mod web;
pub(crate) mod web_driver;
#[cfg(unix)]
pub(crate) mod xvfb;

pub use android_adb::AndroidAdbBackend;
pub use cdp::CdpBackend;
#[cfg(unix)]
pub use gtk_broadway::GtkBroadwayBackend;
#[cfg(unix)]
pub use linux_wayland::LinuxWaylandBackend;
#[cfg(unix)]
pub use linux_x11::LinuxX11Backend;
pub use web::WebBackend;
