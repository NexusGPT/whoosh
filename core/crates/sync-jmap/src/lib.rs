//! JMAP adapter for Fastmail and other JMAP hosts (fn-sync-jmap).
//!
//! Open-source start: `jmap-client` 0.4 (Stalwart Labs; licence "Apache-2.0 OR MIT", both licence
//! files in the crate) is the protocol client. This crate adds what it does not have: the
//! capability probe that answers `FallbackToImap`, the first-screen-before-history order, the
//! Email-state cursor that travels with each batch, batch sizes taken from the server's declared
//! limit, and the EventSource watch with catch-up after every reconnect.
mod adapter;
mod config;
mod errors;
mod map;
mod port;
mod push;

pub use adapter::JmapAdapter;
pub use config::{discovery_base, JmapAuth, JmapConfig};
pub use port::*;
pub use push::{PushConfig, WatchEnd};
