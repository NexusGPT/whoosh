//! The ports. One method each, named `execute`. The adapters (IMAP, Graph, JMAP, Gmail API) live in the sync crates.

// MessageRef, WriteBack, Refusal and MailAdapter moved verbatim to the contract crate (root delta 4,
// 2026-11-07); re-exported so every `crate::port::…` path is unchanged.
pub use contracts::{MailAdapter, MessageRef, Refusal, WriteBack};

/// Undo of a quick send cancels the send while its undo window is open. The outbox crate implements this.
pub trait SendCanceller: Send + Sync {
    fn execute(&self, outbox_id: &str) -> Result<(), Refusal>;
}

/// engine-act: the server write-back recorded as a durable row in the SAME transaction as the local write, so a crash between
/// the two can never keep the local change and lose the server's. With a ledger set, `core_first` asks it inside the transaction
/// and does not call the `MailAdapter` itself: a worker applies the recorded rows later (optimistic, the local state stays).
pub trait WriteBackLedger: Send + Sync {
    fn execute(&self, ctx: &mut store::WriteCtx<'_>, request: &WriteBack, now_ms: i64) -> Result<(), store::StoreError>;
}

/// One person from the device's Contacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceContact {
    pub name: String,
    pub addresses: Vec<String>,
}

/// The owner has not shared the device's Contacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenied;

pub trait ContactsDirectory: Send + Sync {
    fn execute(&self) -> Result<Vec<DeviceContact>, PermissionDenied>;
}
