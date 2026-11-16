//! AI provider and cloud speech adapters. The crate opens no socket and holds no key: HTTP is the `Transport`
//! seam, and a key is read from the Keychain through the secrets port at the moment of the call, by `KeychainRef`.
//! Provider endpoints and model ids live in `table` only.
mod call;
// ai-settings-core: the pasted key's check (one model listing, the key never kept here)
pub mod check;
mod key;
pub mod ports;
pub mod providers;
pub mod speech;
pub mod table;
pub mod transport;
mod words;

#[cfg(feature = "live")]
pub mod live;

pub use ports::{AudioClip, CompleteText, ListModels, TextRequest, Transcribe};
pub use table::{ProviderId, SpeechId};
