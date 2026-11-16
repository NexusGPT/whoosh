//! The engine's rows in the store: `folder_sync_state` (cursor, backfill anchor, full pass) and
//! `account_sync_state` (the contract's SyncState). Every statement runs inside `Store::write`, so the
//! one writer thread applies it; reads that need the newest truth run there too.
use contracts::{ActionFailed, SyncCursor, SyncState};
use rusqlite::{params, Transaction};
use std::collections::HashMap;
use store::{Store, StoreError};

use crate::port::Page;
use crate::words;

/// What the sync is doing, as a coded status. The words carry the account's stored message count, read inside the
/// transaction that stores the page, so the count and the status never disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// `sync.connecting`: opening the connection and listing the folders.
    Connecting,
    /// `sync.fetching`: a page of `folder` is stored.
    Fetching { folder: String },
    /// `sync.messages`: the history is stored.
    Messages,
}

/// Where one folder's backfill stands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FolderRow {
    /// A row exists: at least one page of this folder is stored.
    pub seen: bool,
    /// Position of the oldest stored message; the next page starts below it.
    pub anchor: Option<i64>,
    /// The folder's full history is stored.
    pub done: bool,
}

/// A change to an account's SyncState. `None` leaves a field as it is; `Some(None)` clears it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatePatch {
    pub history_fetched: Option<bool>,
    pub add_indexed: u64,
    pub last_error: Option<Option<ActionFailed>>,
    pub provider_status_words: Option<Option<String>>,
    /// A coded progress status; it wins over `provider_status_words` when both are set.
    pub progress: Option<Progress>,
    pub last_synced_ms: Option<i64>,
}

pub fn load_folders(store: &Store, account_id: &str) -> Result<HashMap<String, FolderRow>, StoreError> {
    let id = account_id.to_string();
    store.write(move |ctx| {
        let mut st = ctx.tx.prepare("SELECT folder, last_uid, last_full_pass_ms FROM folder_sync_state WHERE account_id = ?1")?;
        let rows = st.query_map([&id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?, r.get::<_, Option<i64>>(2)?)))?;
        let mut out = HashMap::new();
        for row in rows {
            let (folder, anchor, full) = row?;
            out.insert(folder, FolderRow { seen: true, anchor, done: full.is_some() });
        }
        Ok(out)
    })
}

/// Store a page: the messages first (the store's insert), then, in one transaction, the folder's anchor,
/// cursor and full-pass time with the account's indexed count. The anchor never moves past a message that
/// is not stored: a stop between the two writes repeats one page, and the insert ignores repeated ids.
/// `show_progress`: the page also sets the coded `sync.fetching` status (not while the provider has the account slowed).
pub fn persist_page(store: &Store, account_id: &str, folder: &str, page: Page, now_ms: i64, show_progress: bool) -> Result<SyncState, StoreError> {
    let inserted = if page.messages.is_empty() { 0 } else { store.insert_messages(page.messages)? };
    let (aid, f) = (account_id.to_string(), folder.to_string());
    let (oldest, done, cursor) = (page.oldest, page.done, page.cursor);
    store.write(move |ctx| {
        let tx = ctx.tx;
        tx.execute("INSERT INTO folder_sync_state (account_id, folder) VALUES (?1, ?2) ON CONFLICT(account_id, folder) DO NOTHING", params![aid, f])?;
        tx.execute("UPDATE folder_sync_state SET last_fetched_ms = ?3 WHERE account_id = ?1 AND folder = ?2", params![aid, f, now_ms])?;
        if let Some(o) = oldest {
            tx.execute("UPDATE folder_sync_state SET last_uid = ?3 WHERE account_id = ?1 AND folder = ?2", params![aid, f, o])?;
        }
        if done {
            tx.execute("UPDATE folder_sync_state SET last_full_pass_ms = ?3 WHERE account_id = ?1 AND folder = ?2", params![aid, f, now_ms])?;
        }
        if let Some(c) = cursor {
            write_cursor(tx, &aid, &f, &c)?;
        }
        let patch = StatePatch { add_indexed: inserted as u64, last_synced_ms: Some(now_ms), progress: show_progress.then(|| Progress::Fetching { folder: f.clone() }), ..Default::default() };
        apply(tx, &aid, &patch)
    })
}

pub fn write_state(store: &Store, account_id: &str, patch: StatePatch) -> Result<SyncState, StoreError> {
    let aid = account_id.to_string();
    store.write(move |ctx| apply(ctx.tx, &aid, &patch))
}

fn write_cursor(tx: &Transaction<'_>, aid: &str, f: &str, c: &SyncCursor) -> Result<(), StoreError> {
    let (kind, uv, ms, value): (&str, Option<i64>, Option<i64>, Option<&str>) = match c {
        SyncCursor::Imap { uid_validity, highest_mod_seq } => ("imap", Some(*uid_validity as i64), Some(*highest_mod_seq as i64), None),
        SyncCursor::GraphDelta { token } => ("graph_delta", None, None, Some(token)),
        SyncCursor::GmailHistory { history_id } => ("gmail_history", None, None, Some(history_id)),
        SyncCursor::Jmap { state } => ("jmap", None, None, Some(state)),
    };
    tx.execute(
        "UPDATE folder_sync_state SET cursor_kind = ?3, imap_uid_validity = ?4, imap_highest_mod_seq = ?5, cursor_value = ?6 WHERE account_id = ?1 AND folder = ?2",
        params![aid, f, kind, uv, ms, value],
    )?;
    Ok(())
}

fn apply(tx: &Transaction<'_>, aid: &str, p: &StatePatch) -> Result<SyncState, StoreError> {
    tx.execute("INSERT INTO account_sync_state (account_id) VALUES (?1) ON CONFLICT(account_id) DO NOTHING", params![aid])?;
    if p.add_indexed > 0 {
        tx.execute("UPDATE account_sync_state SET messages_indexed = messages_indexed + ?2 WHERE account_id = ?1", params![aid, p.add_indexed as i64])?;
    }
    if let Some(h) = p.history_fetched {
        tx.execute("UPDATE account_sync_state SET history_fetched = ?2 WHERE account_id = ?1", params![aid, h as i64])?;
    }
    if let Some(e) = &p.last_error {
        let json = e.as_ref().map(|f| serde_json::json!({"account_name": f.account_name, "reason_words": f.reason_words, "retryable": f.retryable, "code": f.code}).to_string());
        tx.execute("UPDATE account_sync_state SET last_error_json = ?2 WHERE account_id = ?1", params![aid, json])?;
    }
    if let Some(w) = &p.provider_status_words {
        tx.execute("UPDATE account_sync_state SET provider_status_words = ?2 WHERE account_id = ?1", params![aid, w])?;
    }
    if let Some(t) = p.last_synced_ms {
        tx.execute("UPDATE account_sync_state SET last_synced_ms = ?2 WHERE account_id = ?1", params![aid, t])?;
    }
    if let Some(progress) = &p.progress {
        let n: i64 = tx.query_row("SELECT messages_indexed FROM account_sync_state WHERE account_id = ?1", params![aid], |r| r.get(0))?;
        tx.execute("UPDATE account_sync_state SET provider_status_words = ?2 WHERE account_id = ?1", params![aid, progress_status(progress, n)])?;
    }
    read_state(tx, aid)
}

fn messages_words(n: i64) -> String {
    if n == 1 { "1 message".to_string() } else { format!("{n} messages") }
}

/// The coded status of a progress, with `n` messages stored.
pub fn progress_status(progress: &Progress, n: i64) -> String {
    let count = n.to_string();
    match progress {
        Progress::Connecting => words::status("Connecting…", words::CONNECTING, &[]),
        Progress::Fetching { folder } => words::status(&format!("Fetching {folder}: {}", messages_words(n)), words::FETCHING, &[("folder", folder), ("count", &count)]),
        Progress::Messages => words::status(&messages_words(n), words::MESSAGES, &[("count", &count)]),
    }
}

fn read_state(tx: &Transaction<'_>, aid: &str) -> Result<SyncState, StoreError> {
    let row = tx.query_row(
        "SELECT history_fetched, messages_indexed, last_error_json, provider_status_words, last_synced_ms FROM account_sync_state WHERE account_id = ?1",
        params![aid],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<i64>>(4)?)),
    )?;
    let last_error = match row.2 {
        None => None,
        Some(text) => Some(parse_failure(&text).ok_or_else(|| StoreError::Invalid(format!("account_sync_state.last_error_json is not an ActionFailed: {text}")))?),
    };
    Ok(SyncState { account_id: aid.to_string(), history_fetched: row.0 != 0, messages_indexed: row.1 as u64, last_error, provider_status_words: row.3, last_synced_ms: row.4 })
}

/// Parse a stored `last_error_json` back into the contract record.
pub fn parse_failure(text: &str) -> Option<ActionFailed> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    Some(ActionFailed {
        account_name: v.get("account_name")?.as_str()?.to_string(),
        reason_words: v.get("reason_words")?.as_str()?.to_string(),
        retryable: v.get("retryable")?.as_bool()?,
        // delta 12; a row written before it has no code
        code: v.get("code").and_then(|c| c.as_str()).map(str::to_string),
    })
}
