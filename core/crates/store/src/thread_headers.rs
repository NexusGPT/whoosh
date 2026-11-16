//! lane reply-headers (SPEC A144, migration 0023): each mail's threading headers kept on this Mac, so a reply is threaded from the store
//! and never waits on the mail server. Two columns of `message`:
//!   `rfc_message_id`: the Message-ID with its angle brackets; NULL = not known yet; '' = the server was asked and gave none (not asked again).
//!   `rfc_references_json`: the mail's chain, oldest first (References, else In-Reply-To), a JSON array; NULL = not known yet.
//! `NewMessage` does not carry them: it is built field by field in crates this lane does not own (import, sync-engine, smart-inbox,
//! actions, search-text and the engine's tests), so a new field would stop them compiling. The arrival writes them beside the insert, in
//! the same transaction (`WriteCtx::set_thread_headers`); the fill-in pass after a folder sync writes the rest (`set_thread_headers_quiet`).
use crate::error::StoreError;
use crate::ops::json_strings;
use crate::Store;
use rusqlite::{params, Connection, OptionalExtension};

/// A mail's threading headers: its own Message-ID and its chain (oldest first), each id with its angle brackets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadHeaders {
    pub message_id: String,
    pub references: Vec<String>,
}

/// One id as the store keeps it: trimmed, with its angle brackets (mail-parser hands the Message-ID without them). None for an empty id.
pub fn bracketed(id: &str) -> Option<String> {
    let t = id.trim();
    let bare = t.trim_start_matches('<').trim_end_matches('>').trim();
    if bare.is_empty() { None } else { Some(format!("<{bare}>")) }
}

/// The stored chain of one row, oldest first (the JSON array read by SQLite itself, in its order).
fn chain_of(c: &Connection, row_id: &str) -> Result<Vec<String>, StoreError> {
    let mut st = c.prepare_cached(
        "SELECT j.value FROM message m, json_each(COALESCE(m.rfc_references_json, '[]')) j WHERE m.id = ?1 AND j.type = 'text' ORDER BY j.key",
    )?;
    let rows = st.query_map(params![row_id], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn headers_of(c: &Connection, row_id: &str) -> Result<Option<ThreadHeaders>, StoreError> {
    let mid: Option<Option<String>> = c
        .prepare_cached("SELECT rfc_message_id FROM message WHERE id = ?1")?
        .query_row(params![row_id], |r| r.get::<_, Option<String>>(0))
        .optional()?;
    match mid.flatten() {
        Some(m) if !m.is_empty() => Ok(Some(ThreadHeaders { message_id: m, references: chain_of(c, row_id)? })),
        _ => Ok(None),
    }
}

/// The newest message of a thread whose headers are stored (a non-empty Message-ID), whether or not it still has a place on the server.
fn newest_in_thread(c: &Connection, thread_id: &str) -> Result<Option<(String, ThreadHeaders)>, StoreError> {
    let id: Option<String> = c
        .prepare_cached(
            "SELECT id FROM message WHERE thread_id = ?1 AND rfc_message_id IS NOT NULL AND rfc_message_id <> '' \
             ORDER BY received_at_ms DESC, id DESC LIMIT 1",
        )?
        .query_row(params![thread_id], |r| r.get(0))
        .optional()?;
    let Some(id) = id else { return Ok(None) };
    Ok(headers_of(c, &id)?.map(|h| (id, h)))
}

impl Store {
    /// The threading headers of one stored message. None when the message is not stored, its headers are not known yet, or the server
    /// gave none (the '' mark).
    pub fn thread_headers(&self, message_id: &str) -> Result<Option<ThreadHeaders>, StoreError> {
        self.read(|c| headers_of(c, message_id))
    }

    /// The reply's parent as the store knows it: the newest message of `thread_id` whose headers are stored (its row id and headers).
    /// None when no message of the thread has stored headers (the caller then asks the server, as before 0023).
    pub fn newest_thread_headers(&self, thread_id: &str) -> Result<Option<(String, ThreadHeaders)>, StoreError> {
        self.read(|c| newest_in_thread(c, thread_id))
    }

    /// The fill-in pass's work: (UID, message id) of the messages of one account and folder whose headers are not known yet
    /// (`rfc_message_id` NULL) and that have a place on the server (a UID as their remote id), newest first, at most `limit`.
    /// The partial index `message_headers_unknown` (0023) holds only the unknown rows, so the planner need not walk the known ones.
    pub fn thread_headers_unknown(&self, account_id: &str, folder: &str, limit: usize) -> Result<Vec<(u32, String)>, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT remote_id, id FROM message \
                 WHERE account_id = ?1 AND folder = ?2 AND rfc_message_id IS NULL AND remote_id IS NOT NULL AND remote_id <> '' AND remote_id NOT GLOB '*[^0-9]*' \
                 ORDER BY received_at_ms DESC, id LIMIT ?3",
            )?;
            let rows = st.query_map(params![a, f, limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for row in rows {
                let (uid, id) = row?;
                if let Ok(u) = uid.parse::<u32>() { out.push((u, id)); }
            }
            Ok(out)
        })
    }

    /// The folders of one account that hold rows whose headers are not known yet (stored before 0023, or a fetch that read no Message-ID)
    /// and that have a place on the server: the fill-in pass covers them when the account's adapter is registered, not only the folders a
    /// later sync names.
    pub fn thread_headers_unknown_folders(&self, account_id: &str) -> Result<Vec<String>, StoreError> {
        let a = account_id.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT DISTINCT folder FROM message \
                 WHERE account_id = ?1 AND rfc_message_id IS NULL AND remote_id IS NOT NULL AND remote_id <> '' AND remote_id NOT GLOB '*[^0-9]*' \
                 ORDER BY folder",
            )?;
            let rows = st.query_map(params![a], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// The fill-in pass's write: `Some` sets a message's headers; `None` marks one the server did not answer ('' in `rfc_message_id`, only
    /// where it is still unknown), so the next pass does not ask again. One transaction; nothing is published (no list, count or thread
    /// shows these columns: the update hook's notes are dropped, as `set_bodies_quiet` does). Answers how many rows changed.
    pub fn set_thread_headers_quiet(&self, rows: Vec<(String, Option<ThreadHeaders>)>) -> Result<usize, StoreError> {
        if rows.is_empty() { return Ok(0); }
        self.submit(move |st| {
            let out = (|| -> Result<usize, StoreError> {
                let tx = st.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let changed = write_headers(&tx, &rows)?;
                tx.commit()?;
                Ok(changed)
            })();
            st.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out
        })?
    }
}

/// `Some`: the known headers replace what the row held (a row gains them when they are known). `None`: '' where still unknown.
fn write_headers(tx: &rusqlite::Transaction<'_>, rows: &[(String, Option<ThreadHeaders>)]) -> Result<usize, StoreError> {
    let mut set = tx.prepare_cached(
        "UPDATE message SET rfc_message_id = ?2, rfc_references_json = ?3 \
         WHERE id = ?1 AND (rfc_message_id IS NOT ?2 OR rfc_references_json IS NOT ?3)",
    )?;
    let mut mark = tx.prepare_cached("UPDATE message SET rfc_message_id = '' WHERE id = ?1 AND rfc_message_id IS NULL")?;
    let mut changed = 0;
    for (id, headers) in rows {
        changed += match headers {
            Some(h) => set.execute(params![id, h.message_id, json_strings(&h.references)])?,
            None => mark.execute(params![id])?,
        };
    }
    Ok(changed)
}

impl crate::WriteCtx<'_> {
    /// The arrival's write, inside the insert's own transaction (`junk_filter::store_arrivals`): each message's known headers. A message
    /// whose fetch read no Message-ID is left out by the caller (its row keeps what it held; the fill-in pass asks the server).
    pub fn set_thread_headers(&mut self, rows: &[(String, ThreadHeaders)]) -> Result<usize, StoreError> {
        let known: Vec<(String, Option<ThreadHeaders>)> = rows.iter().map(|(id, h)| (id.clone(), Some(h.clone()))).collect();
        write_headers(self.tx, &known)
    }
}
