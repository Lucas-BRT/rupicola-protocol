//! Code written once, async-first, and generated for each flavour by `maybe-async-cfg`:
//! the blocking copy lands in a `sync` module and the tokio copy in `asynchronous`. What
//! differs between the two is the small `rt_sync` / `rt_async` pair, which both expose the
//! same `Runtime`.

pub(crate) mod android_adb;
#[cfg(unix)]
pub(crate) mod linux_wayland;
#[cfg(unix)]
pub(crate) mod linux_x11;
#[cfg(feature = "async")]
pub(crate) mod rt_async;
#[cfg(feature = "sync")]
pub(crate) mod rt_sync;
pub(crate) mod runner;
pub(crate) mod traits;
#[cfg(unix)]
pub(crate) mod xvfb;
