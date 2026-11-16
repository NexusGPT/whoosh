//! The secrets lane: Keychain adapter and ports. The database never holds a secret; it holds
//! the `KeychainRef` this crate returns (docs/DATA-MAP.md, CREDENTIAL records).
mod kind;
mod memory;
mod port;
// rig-core: the hidden test copy's keyring, kept in one file of its run folder (test data only)
mod rig_file;
mod secret;

#[cfg(target_os = "macos")]
mod keychain;

pub use kind::{Namespace, SecretKind};
pub use memory::MemoryKeychain;
pub use port::{DeleteSecret, ReadSecret, StoreSecret};
pub use rig_file::RigFileKeychain;
pub use secret::Secret;

#[cfg(target_os = "macos")]
pub use keychain::{ItemAttributes, Keychain, SyncMode};
