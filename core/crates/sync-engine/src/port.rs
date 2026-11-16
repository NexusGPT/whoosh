//! The port the engine drives: one mail account as a list of folders and pages of messages.
//!
//! CONTRACT DELTA: `core/crates/contracts` holds no shared MailAdapter trait; each adapter crate declares
//! its own (sync-imap and sync-jmap `port.rs`, sync-graph and sync-gmail-api concrete methods). This
//! port is the engine's side of the seam. Each adapter gets a thin impl of it when the root decides
//! where the shared trait lives (docs/lanes/core-sync-engine.md "## Contract deltas").
use async_trait::async_trait;
use contracts::{CoreError, SyncCursor};
use store::NewMessage;

/// Messages of one folder, newest first. `oldest` is the position of the oldest message in the page
/// (an IMAP UID; an ordinal for providers without one); the next page starts below it.
#[derive(Debug, Clone)]
pub struct Page {
    pub messages: Vec<NewMessage>,
    pub oldest: Option<i64>,
    /// True when nothing lies below `oldest`.
    pub done: bool,
    /// The folder's change cursor, when the provider hands one out with this page.
    pub cursor: Option<SyncCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// The provider asked us to slow down. `retry_after_ms` is its own wait when it named one.
    Throttled { retry_after_ms: Option<u64>, reason_words: String },
    /// Anything else, in the server's own words. `code` is the status code the failure carries beside its words
    /// (`words::failed_code`, or an engine or sign-in code); None: the engine codes it as a failed sync.
    Failed { reason_words: String, retryable: bool, code: Option<String> },
}

impl From<CoreError> for SourceError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Action { failure } => SourceError::Failed { reason_words: failure.reason_words, retryable: failure.retryable, code: failure.code },
            other => SourceError::Failed { reason_words: other.to_string(), retryable: false, code: None },
        }
    }
}

/// How an account is reached: what its worker cap is measured against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Every provider's IMAP server, Microsoft 365 over XOAUTH2 included.
    Imap,
    /// Microsoft Graph, where a Graph adapter runs.
    Graph,
}

#[async_trait]
pub trait AccountSource: Send + Sync {
    /// The way this source reaches the server. IMAP unless the adapter says otherwise.
    fn transport(&self) -> Transport {
        Transport::Imap
    }

    /// The folders to sync. A refused login surfaces here.
    async fn list_folders(&self) -> Result<Vec<String>, SourceError>;

    /// Up to `limit` messages of `folder` strictly below `before` (None: from the newest), newest first.
    async fn page(&self, folder: &str, before: Option<i64>, limit: u32) -> Result<Page, SourceError>;
}
