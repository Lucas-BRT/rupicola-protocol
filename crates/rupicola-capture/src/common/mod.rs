//! Code written once, async-first, and generated for each flavour by `maybe-async-cfg`:
//! the blocking copy lands in a `sync` module and the tokio copy in `asynchronous`. What
//! differs between the two is the small `rt_sync` / `rt_async` pair, which both expose the
//! same `Runtime`.

pub(crate) mod android_adb;
pub(crate) mod annotate;
pub(crate) mod cdp;
#[cfg(unix)]
pub(crate) mod gtk_broadway;
#[cfg(unix)]
pub(crate) mod linux_wayland;
#[cfg(unix)]
pub(crate) mod linux_x11;
pub(crate) mod probe;
#[cfg(feature = "async")]
pub(crate) mod rt_async;
#[cfg(feature = "sync")]
pub(crate) mod rt_sync;
pub(crate) mod runner;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod traits;
pub(crate) mod web;
#[cfg(unix)]
pub(crate) mod xvfb;
