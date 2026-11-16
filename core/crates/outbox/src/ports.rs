//! Ports: one method each, named `execute`. The use cases know only these.
use crate::domain::Record;
use contracts::{ActionFailed, CoreError, Draft, Signature};
use send::{ReplyContext, SendAccount, ThreadKind};

/// What the draft table does in the same transaction as an outbox write.
#[derive(Debug, Clone, PartialEq)]
pub enum DraftEffect {
    Keep,
    /// The draft moved into the outbox.
    Remove { draft_id: String },
    /// The entry went back to editing (undo, cancel): the composer reopens with the same text.
    Restore(Draft),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveRequest {
    pub record: Record,
    pub draft: DraftEffect,
}

/// Insert or update one entry (and its draft effect) in ONE transaction.
pub trait SaveEntryPort: Send + Sync {
    fn execute(&self, request: &SaveRequest) -> Result<(), CoreError>;
}

pub trait GetEntryPort: Send + Sync {
    fn execute(&self, id: &str) -> Result<Option<Record>, CoreError>;
}

/// Entries that are Undoable, Scheduled, Sending or Failed, oldest first.
pub trait ListOpenEntriesPort: Send + Sync {
    fn execute(&self) -> Result<Vec<Record>, CoreError>;
}

/// Every entry that carries this idempotency key, whatever its state. The key is minted once per queued message;
/// an entry stored before keys existed answers to its own id.
pub trait FindByKeyPort: Send + Sync {
    fn execute(&self, key: &str) -> Result<Vec<Record>, CoreError>;
}

/// Sent entries whose Sent-folder copy is still to be saved (the message itself was delivered).
pub trait ListPendingCopiesPort: Send + Sync {
    fn execute(&self) -> Result<Vec<Record>, CoreError>;
}

/// The headers a reply or a forward needs from the message it answers: its Message-ID and its References.
/// The store keeps neither, so the adapter reads them from the mail server.
pub trait ReplyContextPort: Send + Sync {
    fn execute(&self, thread_id: &str, kind: ThreadKind) -> Result<ReplyContext, CoreError>;
}

pub trait LoadDraftPort: Send + Sync {
    fn execute(&self, draft_id: &str) -> Result<Draft, CoreError>;
}

/// The words the owner reads for an account ("Nordlicht").
pub trait AccountNamePort: Send + Sync {
    fn execute(&self, account_id: &str) -> String;
}

// OnlinePort moved to the contract crate (root delta 4, 2026-11-07) so Swift can implement it;
// re-exported so every `crate::ports::OnlinePort` path is unchanged.
pub use contracts::OnlinePort;

/// Where the Sent copy stands after a delivery.
#[derive(Debug, Clone, PartialEq)]
pub enum SentCopy {
    Stored,
    /// Delivered, but the Sent folder did not take its copy. Only the copy is retried, never the delivery.
    Pending(PendingCopy),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingCopy {
    pub account_id: String,
    pub rfc822: Vec<u8>,
    pub failure: ActionFailed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Delivered {
    pub message_id: String,
    pub sent_at_ms: i64,
    pub copy: SentCopy,
}

/// Hand one draft to its account's mail server, once. Err = nothing was delivered.
pub trait SendEntryPort: Send + Sync {
    fn execute(&self, draft: &Draft, reply: Option<&ReplyContext>) -> Result<Delivered, CoreError>;
}

/// Contracts delta 22: the files of a draft about to be sent, read NOW (a file of this Mac as it is at this moment, a forwarded file from the
/// store or fetched from its server), in the order of `draft.attachment_ids`. Err = the message is not sent (a file gone, a fetch that failed),
/// in words: never a message without one of its files.
pub trait AttachmentFilesPort: Send + Sync {
    fn execute(&self, draft: &Draft) -> Result<Vec<send::OutgoingFile>, CoreError>;
}

/// What the sender needs for one account: its SMTP settings (secret resolved for this call only) and signature.
pub trait SendAccountPort: Send + Sync {
    fn execute(&self, account_id: &str) -> Result<(SendAccount, Option<Signature>), CoreError>;
}

/// Send one record to the other devices (encrypted sync).
pub trait PublishChangePort: Send + Sync {
    fn execute(&self, record: &Record) -> Result<(), CoreError>;
}

/// Fetch the records the other devices changed since the last pull.
pub trait PullChangesPort: Send + Sync {
    fn execute(&self) -> Result<Vec<Record>, CoreError>;
}
