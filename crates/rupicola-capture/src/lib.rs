//! Headless capture backends and visual annotation engine built on `rupicola-protocol`.
//!
//! The capture contract ([`session`]) and what backends do (`backend`) are shared.
//! Backends come in the flavour a feature selects, and none is on by default, so a user
//! must choose: `sync` builds [`sync`], which blocks on its processes, and `async` builds
//! `asynchronous`, which runs them on tokio. Both may be on.

#[cfg(not(any(feature = "sync", feature = "async")))]
compile_error!(
    "enable the `sync` or the `async` feature of rupicola-capture to choose how captures run"
);

#[cfg(feature = "async")]
pub mod asynchronous;

mod backend;
pub mod session;
#[cfg(feature = "sync")]
pub mod sync;
