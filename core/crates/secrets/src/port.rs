//! Ports: one method each, named `execute`. Use cases depend on these, never on an adapter.
use contracts::{CoreError, KeychainRef};

use crate::{Secret, SecretKind};

/// Writes a secret (creating or replacing it) and returns the only thing the database keeps.
pub trait StoreSecret: Send + Sync {
    fn execute(&self, kind: SecretKind, name: &str, secret: &Secret) -> Result<KeychainRef, CoreError>;
}

/// Reads the secret a reference points at. `NotFound` when the item is gone.
pub trait ReadSecret: Send + Sync {
    fn execute(&self, reference: &KeychainRef) -> Result<Secret, CoreError>;
}

/// Removes the item. Removing an item that is already gone is not an error (use and expiry
/// both delete a device link, in either order).
pub trait DeleteSecret: Send + Sync {
    fn execute(&self, reference: &KeychainRef) -> Result<(), CoreError>;
}
