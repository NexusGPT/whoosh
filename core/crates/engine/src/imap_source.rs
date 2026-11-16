//! One IMAP account as the sync engine's `AccountSource`: the folders to sync (with their SPECIAL-USE roles kept for the
//! mailbox lists) and pages of messages newest first, over sync-imap's adapter. The engine owns order, retries, the
//! first-screen-then-history policy and every store write; this file only translates.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use store::NewMessage;
use sync_engine::{words, AccountSource, Page, SourceError};
use sync_imap::{FetchOptions, FetchedMessage, FolderRole, ImapAdapter, ImapMailAdapter};

use crate::consts::IMAP_PREVIEW_BYTES;

/// The folder roles each account's server named at its last LIST, by account then folder.
pub type Roles = Arc<Mutex<HashMap<String, HashMap<String, FolderRole>>>>;

pub const OPTS: FetchOptions = FetchOptions { preview_bytes: IMAP_PREVIEW_BYTES };

/// The newest page of a folder is a history page below a UID no server hands out (RFC 9051: UIDs are 32-bit, nonzero).
const ABOVE_EVERY_UID: u32 = u32::MAX;

pub struct ImapSource {
    pub account_id: String,
    pub adapter: Arc<ImapAdapter>,
    pub roles: Roles,
    /// Ids of messages the server flagged (\Flagged): written to Starred once the engine stored them.
    pub flagged: Arc<Mutex<Vec<String>>>,
    /// The hidden test copy's mutant `sync-never-stores`: every page comes back empty.
    pub never_store: bool,
    /// Delta 17: the store (to tell a message the sync is about to store for the first time from one it already holds) and the
    /// label flow, which is told the new ids before the engine stores the page and kicked after it (`Forward`, sync.rs).
    pub(crate) store: Arc<store::Store>,
    pub(crate) filing: Arc<crate::filing_worker::Filing>,
    /// body-r1: told which folders had their newest page fetched, so their newest bodies are downloaded once the page is stored.
    pub(crate) bodies: Arc<crate::bodies::Bodies>,
}

/// A fetched IMAP message as the store's insert takes it: the UID is the remote id, the decoded preview stays the only body text
/// until the body is downloaded (body-r1: in the background, the newest first), no tab decided yet (it sits in the first tab).
pub fn new_message(f: FetchedMessage) -> NewMessage {
    let has_list_header = !f.message.list_unsubscribe.is_empty();
    NewMessage {
        remote_id: Some(f.uid.to_string()),
        body_text: None,
        body_html: None,
        has_list_header,
        size_bytes: f.size.map(i64::from),
        tab_id: None,
        message: f.message,
    }
}

/// lane reply-headers (SPEC A144): a fetched message's threading headers as the store keeps them (each id with its angle brackets; mail-parser
/// hands the Message-ID without them), keyed by its row id. `NewMessage` does not carry them (it keeps its shape): the caller writes them
/// beside the row. None when the fetch read no Message-ID: the row stays unknown and the fill-in pass after the folder sync asks the server.
pub fn arrival_headers(f: &FetchedMessage) -> Option<(String, store::ThreadHeaders)> {
    let message_id = f.message_id_header.as_deref().and_then(store::bracketed)?;
    let references = f.references.iter().filter_map(|r| store::bracketed(r)).collect();
    Some((f.message.id.clone(), store::ThreadHeaders { message_id, references }))
}

/// real-replyto (migration 0027): a fetched message's Reply-To entries as the store keeps them, keyed by its row id. Every arrival has an
/// entry (an empty list = the mail has no Reply-To, which the store records as read, not as unknown).
pub fn arrival_reply_to(f: &FetchedMessage) -> (String, Vec<store::ReplyTo>) {
    (f.message.id.clone(), f.reply_to_named.iter().map(|(name, address)| store::ReplyTo { name: name.clone(), address: address.clone() }).collect())
}

/// A server's answer as the engine's retry policy reads it: a "slow down" keeps its own wait, the rest its words.
pub fn source_error(e: contracts::CoreError) -> SourceError {
    match e {
        contracts::CoreError::Throttled { provider, retry_after_ms } => SourceError::Throttled { retry_after_ms, reason_words: format!("{provider} asked the app to slow down") },
        other => SourceError::from(other),
    }
}

/// `source_error` with the step that failed in front of the server's words ("could not fetch INBOX: ..."), so the sync row
/// the owner reads says what the sync was doing, and the failure carries `sync.failed` with the step ("list" or "fetch"), the
/// folder and the server's words as parameters (the app shows its own sentence for the code and these words under Details).
/// A failure that already carries an engine or sign-in code (`engine.`, `oauth.`) keeps it. The words follow the account's
/// name on the status line.
pub fn step_error(step: &str, folder: Option<&str>, e: contracts::CoreError) -> SourceError {
    match source_error(e) {
        SourceError::Failed { reason_words, retryable, code } => {
            let what = match (step, folder) { ("fetch", Some(f)) => format!("fetch {f}"), _ => "list its folders".to_string() };
            let kept = code.filter(|c| c.starts_with("oauth.") || c.starts_with("engine."));
            let code = kept.unwrap_or_else(|| words::failed_code(step, folder.unwrap_or(""), &reason_words));
            SourceError::Failed { reason_words: format!("could not {what}: {reason_words}"), retryable, code: Some(code) }
        }
        throttled => throttled,
    }
}

/// Folders the sync skips: Gmail's All Mail and Starred repeat every other folder's messages.
pub fn skipped(role: FolderRole) -> bool { matches!(role, FolderRole::All | FolderRole::Flagged) }

impl ImapSource {
    fn note_flagged(&self, page: &[FetchedMessage]) {
        let ids: Vec<String> = page.iter().filter(|m| m.flags.iter().any(|f| f.eq_ignore_ascii_case("\\Flagged"))).map(|m| m.message.id.clone()).collect();
        if !ids.is_empty() { self.flagged.lock().unwrap_or_else(|e| e.into_inner()).extend(ids); }
    }
}

#[async_trait]
impl AccountSource for ImapSource {
    async fn list_folders(&self) -> Result<Vec<String>, SourceError> {
        let folders = self.adapter.list_folders().await.map_err(|e| step_error("list", None, e))?;
        let mut roles = HashMap::new();
        let mut names = Vec::new();
        for f in folders {
            roles.insert(f.name.clone(), f.role);
            if !skipped(f.role) { names.push(f.name); }
        }
        crate::replies::register_folders(&self.store, &self.account_id, &roles); // replies-settings: the owner's defaults for the reply sources
        self.roles.lock().unwrap_or_else(|e| e.into_inner()).insert(self.account_id.clone(), roles);
        Ok(names)
    }

    async fn page(&self, folder: &str, before: Option<i64>, limit: u32) -> Result<Page, SourceError> {
        if self.never_store {
            return Ok(Page { messages: vec![], oldest: None, done: true, cursor: None });
        }
        let below = match before {
            None => ABOVE_EVERY_UID,
            Some(b) if b <= 1 => return Ok(Page { messages: vec![], oldest: None, done: true, cursor: None }),
            Some(b) => u32::try_from(b).unwrap_or(ABOVE_EVERY_UID),
        };
        // The newest page also carries the folder's position (UIDVALIDITY, HIGHESTMODSEQ) for the later resync.
        let cursor = if before.is_none() { Some(self.adapter.folder_state(folder).await.map_err(|e| step_error("fetch", Some(folder), e))?.cursor()) } else { None };
        let page = self.adapter.history_page(folder, below, limit, OPTS).await.map_err(|e| step_error("fetch", Some(folder), e))?;
        self.note_flagged(&page.messages);
        // delta 17: the messages this page stores for the first time run the labelling order once the engine has stored them
        self.filing.expect(page.messages.iter().filter(|m| self.store.message(&m.message.id).ok().flatten().is_none()).map(|m| m.message.id.clone()).collect::<Vec<_>>());
        // body-r1: the folder's newest page is about to be stored; its newest bodies download after it (the engine's next event)
        if before.is_none() { self.bodies.folder_synced(&self.account_id, folder); }
        // lane reply-headers: the page's Message-IDs and References, written once the engine has stored the page (`NewMessage` keeps its shape)
        self.bodies.headers.hold(page.messages.iter().filter_map(arrival_headers));
        self.bodies.headers.hold_reply_to(page.messages.iter().map(arrival_reply_to)); // real-replyto: and their Reply-To
        Ok(Page {
            oldest: page.oldest_uid.map(i64::from),
            done: page.done,
            cursor,
            messages: page.messages.into_iter().map(new_message).collect(),
        })
    }
}
