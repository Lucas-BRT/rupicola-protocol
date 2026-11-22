//! Headless capture backends and visual annotation engine built on `rupicola-protocol`.
//!
//! The capture contract ([`session`]) is shared. Backends come in the flavour a feature
//! selects, and none is on by default, so a user must choose: `sync` builds [`sync`],
//! which blocks on its processes.

#[cfg(not(feature = "sync"))]
compile_error!("enable the `sync` feature of rupicola-capture to choose how captures run");

mod backend;
pub mod session;
#[cfg(feature = "sync")]
pub mod sync;
