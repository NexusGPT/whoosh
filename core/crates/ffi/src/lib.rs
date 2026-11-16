//! The library Swift links (libwhoosh_ffi.a). It re-exports the contract crate so every
//! UniFFI item lands in one static library. The real core (store, sync, actions) is wired
//! here by its lanes through `contracts::WhooshCore`; slice 1 (add an account) is `engine::RustCore`.
pub use contracts::*;
// The real core (slice 1): `open_rust_core` lands in the same static library, in the Swift module `engine`.
pub use engine::open_rust_core;

pub mod readers;
