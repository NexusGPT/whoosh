//! JmapMailAdapter: renamed from MailAdapter by the lead's ruling (2026-11-07) so the contract's MailAdapter, a write-back port, is the one name.
//! The port this adapter implements.
//!
//! CONTRACT DELTA: docs/plan.json calls it "the contract's MailAdapter port", but
//! `core/crates/contracts` holds no such trait (the contract crate is frozen for this lane, and
//! core-sync-imap declares its own, UID-shaped, copy). JMAP names messages by opaque string ids
//! and carries a per-account state string, so this port is shaped for that. It is built only from
//! contract record types (`Message`, `SyncCursor::Jmap`, `CoreError`). Moving it into `contracts`
//! changes only the `use` lines of the crates that consume it. See docs/lanes/core-sync-jmap.md.
use async_trait::async_trait;
use contracts::{CoreError, Message, SyncCursor};

/// The words the owner reads when a host offers no JMAP (docs/CAPABILITY-CONTRACT.md C-J4.5,
/// docs/JOURNEYS.md J4.5: "the host does not offer JMAP: falls back to IMAP").
pub const FALLBACK_WORDS: &str = "no JMAP: falls back to IMAP";

/// What a host answered to the question "do you offer JMAP?".
#[derive(Debug)]
pub enum Discovery<A> {
    /// The host offers JMAP and the sign-in worked; the adapter is ready.
    Jmap(A),
    /// The host does not offer JMAP. The caller sets the account up over IMAP.
    /// `reason_words` always starts with [`FALLBACK_WORDS`] and then says what the host answered.
    FallbackToImap { reason_words: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// The JMAP mailbox id.
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    /// `inbox`, `sent`, `drafts`, `trash`, `junk`, `archive`, `important`, or the server's own
    /// role word; None for a mailbox without a role.
    pub role: Option<String>,
    pub total: u64,
    pub unread: u64,
}

/// One message as the server holds it. `message` is the contract record; the rest is what the
/// store needs to key and revisit it.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedMessage {
    /// The JMAP email id, unchanged. `message.id` is `<account id>:<email id>`.
    pub server_id: String,
    pub mailbox_ids: Vec<String>,
    /// JMAP keywords, for example `$seen`, `$flagged`, `$draft`.
    pub keywords: Vec<String>,
    pub message: Message,
}

/// Bytes the store keeps per message that the server could shorten for us.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchOptions {
    /// Characters of the server's preview kept as the row preview.
    pub preview_chars: usize,
}

/// One batch handed to the core. The cursor travels WITH the data it covers, so the core stores
/// both in one commit: a crash between them cannot lose a message or replay a cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    /// New or changed messages (JMAP reports a change as the message's new full state).
    pub changed: Vec<FetchedMessage>,
    /// Server ids of messages the server destroyed.
    pub vanished: Vec<String>,
    /// Persist as the folder-sync-state cursor together with this batch. None for a history page.
    pub cursor: Option<SyncCursor>,
}

/// What the live connection is doing, for `sync_state.provider_status_words`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStatus {
    Connected,
    Reconnecting { reason_words: String },
}

/// Receives batches in the order the adapter fetched them. A returned error stops the sync and is
/// returned to the caller unchanged, so nothing is dropped silently.
#[async_trait]
pub trait MessageSink: Send {
    async fn deliver(&mut self, batch: Delivery) -> Result<(), CoreError>;
    async fn status(&mut self, status: LinkStatus);
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPage {
    pub messages: Vec<FetchedMessage>,
    /// The id to pass as `after_id` for the next page; None for an empty page.
    pub last_id: Option<String>,
    /// True when no message lies beyond this page.
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resync {
    Delta(Delivery),
    /// The server can no longer say what changed since the cursor (JMAP `cannotCalculateChanges`).
    /// The caller drops the account's messages for this mailbox and runs `first_screen` again.
    Reset { reason_words: String },
}

#[async_trait]
pub trait JmapMailAdapter: Send + Sync {
    async fn list_folders(&self) -> Result<Vec<Folder>, CoreError>;

    /// The newest `first_screen` messages of the mailbox, with the Email state as the cursor.
    async fn first_screen(&self, mailbox_id: &str, first_screen: usize, opts: FetchOptions) -> Result<Delivery, CoreError>;

    /// One page of history after `after_id` (None = start at the newest), newest to oldest.
    async fn history_page(&self, mailbox_id: &str, after_id: Option<&str>, limit: usize, opts: FetchOptions) -> Result<HistoryPage, CoreError>;

    /// The first screen handed to `sink` BEFORE any older message is requested, then history pages
    /// of `history_page` messages until the mailbox is exhausted. Returns the cursor of the
    /// first screen's moment.
    async fn initial_sync(
        &self,
        mailbox_id: &str,
        first_screen: usize,
        history_page: usize,
        opts: FetchOptions,
        sink: &mut dyn MessageSink,
    ) -> Result<SyncCursor, CoreError>;

    /// What changed since `cursor` (an Email state).
    async fn resync(&self, cursor: &SyncCursor, opts: FetchOptions) -> Result<Resync, CoreError>;
}
