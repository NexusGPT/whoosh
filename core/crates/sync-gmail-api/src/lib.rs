//! Gmail API sync adapter: history.list incremental sync, full resync on a 404 history, labels, watch.
//! Built only when the `gmail_api_verified` flag is on (`GmailApi::open`). It owns no credential and opens no
//! socket: the HTTP transport, the token source and the clock are handed in.
mod adapter;
mod error;
pub mod model;
mod quota;
pub mod transport;
mod wire;

#[cfg(feature = "live")]
pub mod live;

/// Numbers read from docs/CONSTANTS.md at build time (see build.rs).
pub mod limits {
    include!(concat!(env!("OUT_DIR"), "/constants.rs"));
}

pub use adapter::{GmailApi, Parts, FLAG_NAME};
pub use error::GmailError;
pub use model::*;
pub use quota::QuotaGate;
