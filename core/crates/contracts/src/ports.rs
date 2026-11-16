//! Rust-side ports: traits one core crate declares for another to implement. Not exported to Swift.
//! (The async MailAdapter port still lives in core/crates/sync-imap/src/port.rs; moving it here is
//! a later delta, because it needs async_trait in the contract crate.)
use crate::{CoreError, SyncCursor};

/// What the Sent copy needs: the account and the exact bytes that were delivered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendSentRequest {
    pub account_id: String,
    pub rfc822: Vec<u8>,
}

/// IMAP APPEND of the Sent copy after a delivery. Called by core/crates/send; implemented by the
/// IMAP adapter (core/crates/sync-imap). One method, named `execute`, as every port.
pub trait AppendSentPort: Send + Sync {
    fn execute(&self, request: &AppendSentRequest) -> Result<(), CoreError>;
}

// ---- Moved verbatim from core/crates/actions/src/port.rs (root delta 4, 2026-11-07): the contract
// names the MailAdapter port (docs/CAPABILITY-CONTRACT.md), so it lives here; actions re-exports it.
// Not the same trait as the async `MailAdapter` in core/crates/sync-imap/src/port.rs (a fetch port).

/// One message as the server knows it: enough for an adapter to find it. `folder` is where it was BEFORE the action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRef {
    pub message_id: String,
    pub thread_id: String,
    pub remote_id: Option<String>,
    pub folder: String,
}

/// What the server is asked to do after the core has already committed. One request per account.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteBack {
    SetArchived { account_id: String, messages: Vec<MessageRef>, archived: bool },
    SetRead { account_id: String, messages: Vec<MessageRef>, read: bool },
    /// None = the snooze is lifted.
    Snooze { account_id: String, messages: Vec<MessageRef>, return_at_ms: Option<i64> },
    Pin { account_id: String, message: MessageRef, pinned: bool },
    Label { account_id: String, messages: Vec<MessageRef>, label_name: String, applied: bool },
    /// Copy into the target account, delete from the source only after the target accepted. The adapter owns that order.
    Move { from_account_id: String, to_account_id: String, messages: Vec<MessageRef> },
    MoveToFolder { account_id: String, messages: Vec<MessageRef>, folder: String },
    EmptyFolders { account_id: String, folders: Vec<String> },
    /// RFC 8058 one-click: the adapter reads the List-Unsubscribe headers and posts `List-Unsubscribe=One-Click`.
    Unsubscribe { account_id: String, message: MessageRef },
}

/// A server said no. `account_id` is the account whose server refused; `reason_words` is what the owner reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub account_id: String,
    pub reason_words: String,
    pub retryable: bool,
}

pub trait MailAdapter: Send + Sync {
    fn execute(&self, request: WriteBack) -> Result<(), Refusal>;
}

/// Is there a network path now? Moved from core/crates/outbox/src/ports.rs (root delta 4). Swift answers
/// it from the system's path monitor and calls the outbox's tick() when the path comes back.
#[uniffi::export(with_foreign)]
pub trait OnlinePort: Send + Sync {
    fn execute(&self) -> bool;
}

// ---- The shared sync port (root delta 4, 2026-11-07; from core/crates/sync-engine/src/port.rs, with the
// two changes the root decided: the backfill position is TEXT, and a throttle names its provider).
// Generic over the message the caller stores (the engine's store::NewMessage): the contract crate cannot
// name a store type, because the store depends on the contract.

/// Messages of one folder, newest first. `oldest` is the position of the oldest message in the page, as
/// text: an IMAP UID written as its digits, a JMAP or Graph id as it comes; the next page starts below it.
#[derive(Debug, Clone)]
pub struct SyncPage<M> {
    pub messages: Vec<M>,
    pub oldest: Option<String>,
    /// True when nothing lies below `oldest`.
    pub done: bool,
    /// The folder's change cursor, when the provider hands one out with this page.
    pub cursor: Option<SyncCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    /// The provider asked us to slow down. Every adapter reports a provider limit this one way.
    Throttled { provider: String, retry_after_ms: Option<u64>, reason_words: String },
    /// Anything else, in the server's own words.
    Failed { reason_words: String, retryable: bool },
}

impl From<CoreError> for SyncError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Throttled { provider, retry_after_ms } => {
                let reason_words = CoreError::Throttled { provider: provider.clone(), retry_after_ms }.to_string();
                SyncError::Throttled { provider, retry_after_ms, reason_words }
            }
            CoreError::Action { failure } => SyncError::Failed { reason_words: failure.reason_words, retryable: failure.retryable },
            other => SyncError::Failed { reason_words: other.to_string(), retryable: false },
        }
    }
}

/// One mail account as a list of folders and pages of messages. The engine drives it; the IMAP, Graph,
/// JMAP and Gmail API adapters implement it in their own crates.
#[async_trait::async_trait]
pub trait SyncAdapter: Send + Sync {
    /// What one page carries per message (the engine's store::NewMessage).
    type Message: Send;
    /// The folders to sync. A refused login surfaces here.
    async fn list_folders(&self) -> Result<Vec<String>, SyncError>;
    /// Up to `limit` messages of `folder` strictly below `before` (None: from the newest), newest first.
    async fn page(&self, folder: &str, before: Option<String>, limit: u32) -> Result<SyncPage<Self::Message>, SyncError>;
}
