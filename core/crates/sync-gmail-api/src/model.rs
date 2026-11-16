//! What the adapter hands to the sync engine. Message bodies stay raw RFC 822 bytes: core-parse reads them.
use contracts::SyncCursor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMessage {
    pub id: String,
    pub thread_id: String,
    pub label_ids: Vec<String>,
    pub history_id: String,
    pub internal_date_ms: i64,
    pub size_estimate: u64,
    pub rfc822: Vec<u8>,
}

/// One change to the local mirror. Applying the same change twice leaves the same state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Added { message_id: String, thread_id: String, label_ids: Vec<String> },
    Deleted { message_id: String },
    LabelsChanged { message_id: String, added: Vec<String>, removed: Vec<String> },
}

/// A group of changes with the bodies of the messages it adds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Batch {
    pub changes: Vec<Change>,
    pub messages: Vec<RawMessage>,
}

/// The caller stores each batch. `Err` stops the sync and the cursor is not advanced.
pub trait ChangeSink {
    fn accept(&mut self, batch: Batch) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Incremental,
    FullResync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResyncReason {
    /// No cursor yet: the first sync of the account.
    NoCursor,
    /// Google answered 404 to the history request: the start point is too old.
    HistoryExpired,
}

/// The result of one `sync`. Store `cursor` only after every batch was stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSummary {
    pub mode: SyncMode,
    pub reason: Option<ResyncReason>,
    pub cursor: SyncCursor,
    pub changes: u64,
    pub messages: u64,
    /// Messages Google listed or reported added but that were gone when fetched.
    pub gone: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    System,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub kind: LabelKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    pub history_id: String,
    pub expires_at_ms: i64,
    /// When this adapter should call watch again (GMAIL_WATCH_RENEW_H after it started, never after expiry).
    pub renew_at_ms: i64,
}
