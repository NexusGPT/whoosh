//! The ImapMailAdapter port (renamed from MailAdapter by the lead's ruling, 2026-11-07: the contract's MailAdapter, a write-back port, is the one name).
//!
//! CONTRACT DELTA: docs/plan.json calls this "the contract's MailAdapter port", but
//! `core/crates/contracts` holds no such trait (the contract crate is frozen for this lane).
//! The port is declared here, built only from contract record types, so sync-engine and
//! actions can compile against it today. If the root moves it into `contracts`, only the
//! `use` lines of those crates change. See docs/lanes/core-sync-imap.md "## Contract deltas".
use async_trait::async_trait;
use contracts::{CoreError, Message, SyncCursor};

use crate::roles::FolderRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub name: String,
    pub delimiter: Option<String>,
    pub role: FolderRole,
}

/// One message as the server holds it. `message` is the contract record; the rest is what the
/// store needs to key and revisit it.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchedMessage {
    pub uid: u32,
    pub uid_validity: u32,
    pub modseq: Option<u64>,
    /// IMAP flags as written by the server, for example `\Seen`.
    pub flags: Vec<String>,
    /// The Message-ID header, for threading and for the Sent copy.
    pub message_id_header: Option<String>,
    pub size: Option<u32>,
    /// Gmail's own identity and labels, present only when the server advertises X-GM-EXT-1.
    pub gmail: Option<GmailIdentity>,
    pub message: Message,
    /// junk-r2: the values of the Authentication-Results headers, topmost (the receiving server's own) first, each unfolded; empty when
    /// the message carries none. Read from the BODY.PEEK[HEADER] block the fetch already asks for, so no fetch item is added.
    pub authentication_results: Vec<String>,
    /// junk-r2: the Reply-To addresses, lower-case; empty when the header is absent.
    pub reply_to: Vec<String>,
    /// real-replyto: every Reply-To entry as (display name, address), header order, as the header wrote them (the name '' when it gave none); the
    /// store keeps it so a reply goes where the sender asked. Read from the BODY.PEEK[HEADER] block the fetch already asks for.
    pub reply_to_named: Vec<(String, String)>,
    /// junk-r3: the HTML the first body bytes hold (`BODY.PEEK[TEXT]<0.n>`), None when the slice reaches no HTML part: the spam
    /// filter's link rule reads it on an arrival, before the body is downloaded.
    pub html_head: Option<String>,
    /// A144: the mail's own thread chain: the References ids, else the In-Reply-To ids (RFC 5322 3.6.4), each in angle brackets, in
    /// header order; empty when it has neither. Read from the BODY.PEEK[HEADER] block the fetch already asks for (no fetch item added).
    pub references: Vec<String>,
}

/// X-GM-MSGID and X-GM-LABELS (Gmail IMAP extensions). The message id is the same in every
/// folder that holds the message, so `message.id` is built from it and the store keeps ONE row;
/// `labels` is the complete label set the server reports, whichever folder was fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmailIdentity {
    pub msg_id: u64,
    pub labels: Vec<String>,
}

/// Where a folder stands on the server: the two numbers the store persists per folder
/// (`SyncCursor::Imap`), plus what the server can do for the next sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImapFolderState {
    pub folder: String,
    pub uid_validity: u32,
    /// None when the server has no CONDSTORE (it then answers no HIGHESTMODSEQ).
    pub highest_modseq: Option<u64>,
    pub exists: u32,
    pub condstore: bool,
    pub qresync: bool,
}

impl ImapFolderState {
    /// The cursor the store keeps for this folder.
    pub fn cursor(&self) -> SyncCursor {
        SyncCursor::Imap { uid_validity: u64::from(self.uid_validity), highest_mod_seq: self.highest_modseq.unwrap_or(0) }
    }
}

/// Where a folder stands after a fetch. Persist `cursor` as the folder's `SyncCursor`.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderSnapshot {
    pub folder: String,
    pub cursor: SyncCursor,
    /// Messages in the folder at SELECT time.
    pub exists: u32,
    pub messages: Vec<FetchedMessage>,
    /// Oldest UID in `messages`; the next history page starts below it. None for an empty folder.
    pub oldest_uid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryPage {
    pub messages: Vec<FetchedMessage>,
    pub oldest_uid: Option<u32>,
    /// True when no message lies below `oldest_uid`.
    pub done: bool,
}

/// Where the server put one moved message (RFC 4315 COPYUID): the target folder's UIDVALIDITY and the message's UID there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MovedUid { pub from_uid: u32, pub uid_validity: u32, pub to_uid: u32 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagChange { pub uid: u32, pub flags: Vec<String>, pub modseq: Option<u64>, pub gmail_msg_id: Option<u64> }

#[derive(Debug, Clone, PartialEq)]
pub struct Delta {
    pub new: Vec<FetchedMessage>,
    pub flag_changes: Vec<FlagChange>,
    pub vanished: Vec<u32>,
    pub cursor: SyncCursor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Resync {
    Delta(Delta),
    /// The cursor cannot be used (UIDVALIDITY changed, or the server has no CONDSTORE and the store knows no UID of the folder).
    /// The caller drops the folder's messages and runs `first_screen` again.
    Reset { reason_words: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagOp { Add, Remove, Replace }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FetchOptions {
    /// Bytes of body text kept as the row preview.
    pub preview_bytes: u32,
}

/// Receives batches in the order the adapter fetched them. A returned error stops the sync.
#[async_trait]
pub trait MessageSink: Send {
    async fn deliver(&mut self, batch: Vec<FetchedMessage>) -> Result<(), CoreError>;
}

#[async_trait]
pub trait ImapMailAdapter: Send + Sync {
    async fn list_folders(&self) -> Result<Vec<Folder>, CoreError>;

    /// UIDVALIDITY and HIGHESTMODSEQ of `folder` right now (SELECT with CONDSTORE where offered).
    async fn folder_state(&self, folder: &str) -> Result<ImapFolderState, CoreError>;

    /// The newest `first_screen` messages of `folder`, handed to `sink` BEFORE any older message
    /// is requested, then history pages of `history_page` messages, newest to oldest, until
    /// the folder is exhausted.
    async fn initial_sync(
        &self,
        folder: &str,
        first_screen: u32,
        history_page: u32,
        opts: FetchOptions,
        sink: &mut dyn MessageSink,
    ) -> Result<FolderSnapshot, CoreError>;

    /// One page of history below `before_uid` (for a resumed backfill).
    async fn history_page(&self, folder: &str, before_uid: u32, limit: u32, opts: FetchOptions) -> Result<HistoryPage, CoreError>;

    /// What changed since `cursor`. `known_uids` (sorted, the store's UIDs for this folder) is
    /// read only when the server has no QRESYNC (CONDSTORE alone, or neither: A144, Microsoft 365).
    async fn resync(&self, folder: &str, cursor: &SyncCursor, known_uids: &[u32], opts: FetchOptions) -> Result<Resync, CoreError>;

    async fn set_flags(&self, folder: &str, uids: &[u32], op: FlagOp, flags: &[&str]) -> Result<(), CoreError>;

    /// Move inside the account (MOVE, or COPY + delete + expunge when the server lacks MOVE).
    async fn move_within(&self, folder: &str, uids: &[u32], dest: &str) -> Result<(), CoreError> {
        self.move_placed(folder, uids, dest).await.map(|_| ())
    }

    /// `move_within`, answering where the server put each message (RFC 4315 COPYUID). An empty answer = the server named no place
    /// (UIDPLUS is optional): the caller keeps what it knew and the sync's own re-read of the target folder fills the rest in.
    async fn move_placed(&self, folder: &str, uids: &[u32], dest: &str) -> Result<Vec<MovedUid>, CoreError>;

    /// Store a message in `folder` (APPEND).
    async fn append(&self, folder: &str, raw: &[u8], flags: &[&str], internal_date_ms: Option<i64>) -> Result<(), CoreError>;

    /// APPEND the copy of a sent message to the folder whose role is Sent.
    async fn append_sent(&self, raw: &[u8]) -> Result<(), CoreError>;

    /// The complete RFC 822 bytes of one message, without setting \Seen.
    async fn fetch_raw(&self, folder: &str, uid: u32) -> Result<(Vec<u8>, Vec<String>, Option<i64>), CoreError>;

    /// body-r1: the complete RFC 822 bytes of several messages of one folder, without setting \Seen, by UID. A UID the server no
    /// longer holds is absent from the answer (the others still come back). The default asks one by one; `ImapAdapter` sends one
    /// `UID FETCH u1,u2,... (UID BODY.PEEK[])` on one session.
    async fn fetch_raw_many(&self, folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        let mut out = Vec::with_capacity(uids.len());
        for &uid in uids { out.push((uid, self.fetch_raw(folder, uid).await?.0)); }
        Ok(out)
    }

    /// body-r3: one numbered part of one message (`BODY.PEEK[<part>]`, e.g. `2` or `1.2`, as `parse::Attachment::part` numbers it), without setting
    /// \Seen: the bytes still in the part's transfer encoding (`parse::decode_transfer` reads them). The default refuses.
    async fn fetch_part(&self, _folder: &str, _uid: u32, _part: &str) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::Unexpected { reason_words: "this mail adapter cannot fetch one part of a message".to_string() })
    }

    /// Remove messages for good (flag \Deleted + UID EXPUNGE).
    async fn delete(&self, folder: &str, uids: &[u32]) -> Result<(), CoreError>;
}
