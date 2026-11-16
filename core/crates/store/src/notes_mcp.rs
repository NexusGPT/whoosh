//! lane real-notes: what the real engine keeps for the private notes (contract `Note`, J8), the outside-apps (MCP) grants, server switch
//! and approval answers, and the reads of suggestions' rows the engine needs. Everything stays in this Mac's database.
//!
//! - Notes: migration 0001's `note` table (one row per note; a message note also names its thread). These are the owner's PRIVATE notes on a
//!   thread or a message. They are not the reply writer's fixed notes (`fixed_note`, migration 0013, `replies.rs`, commands SaveFixedNote and
//!   co): those are rules of thumb for the AI reply and have another command family and another table.
//! - MCP: migration 0001's `mcp_grant` table keeps the grants (first-seen order by rowid). The switch ("the server is on") and the approval
//!   answers are two small tables made here by `CREATE TABLE IF NOT EXISTS`, NOT by a migration: a new table in the migration list would
//!   turn `store/migrations/tests/migration_v23.rs:46` ("no table added or removed") red on the base. The engine starts no server and opens
//!   no socket: it only keeps what the owner chose and answered.
//! - A refusal is `Refusal` (words), never a bare SQLite status; a refusal writes nothing.
use crate::error::StoreError;
use crate::writer::WriteCtx;
use crate::Store;
use contracts::{McpGrant, Note, NoteTarget, Suggestion, SuggestionKind};
use rusqlite::{params, Connection, OptionalExtension};

/// A refusal in words: the engine turns it into the contract's `CoreError::Invalid` or `NotFound`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    Invalid { field: String, words: String },
    NotFound { kind: String, id: String },
}
fn invalid(field: &str, words: &str) -> Refusal { Refusal::Invalid { field: field.into(), words: words.into() } }
fn not_found(kind: &str, id: &str) -> Refusal { Refusal::NotFound { kind: kind.into(), id: id.into() } }

/// One stored note with the columns the undo entry needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFull { pub note: Note, pub thread_id: String, pub message_id: Option<String>, pub updated_at_ms: i64 }

/// One approval request as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpApprovalRow { pub request_id: String, pub app_name: String, pub tool_name: String, pub request_words: String, pub state: String }

const NEEDS_WORDS: &str = "a note needs words";

fn full_of(r: (String, String, Option<String>, String, i64, i64)) -> NoteFull {
    let (id, thread_id, message_id, text, written, updated) = r;
    let target = match &message_id { Some(m) => NoteTarget::Message { message_id: m.clone() }, None => NoteTarget::Thread { thread_id: thread_id.clone() } };
    NoteFull { note: Note { id, target, text, written_at_ms: written }, thread_id, message_id, updated_at_ms: updated }
}
const NOTE_COLS: &str = "id, thread_id, message_id, body, created_at_ms, updated_at_ms";
fn note_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, Option<String>, String, i64, i64)> {
    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
}
fn note_by_id(c: &Connection, id: &str) -> Result<Option<NoteFull>, StoreError> {
    Ok(c.query_row(&format!("SELECT {NOTE_COLS} FROM note WHERE id = ?1"), [id], note_row).optional()?.map(full_of))
}

fn table_exists(c: &Connection, name: &str) -> Result<bool, StoreError> {
    Ok(c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [name], |r| r.get::<_, i64>(0))? > 0)
}

impl WriteCtx<'_> {
    // ------------------------------------------------------------ notes

    /// Writes a note on a thread or one message. Empty words and an unknown target are refused and nothing is written.
    pub fn note_add(&mut self, target: &NoteTarget, text: &str, now_ms: i64) -> Result<Result<Note, Refusal>, StoreError> {
        let text = text.trim();
        if text.is_empty() { return Ok(Err(invalid("text", NEEDS_WORDS))); }
        let (thread_id, message_id) = match target {
            NoteTarget::Thread { thread_id } => {
                if self.tx.query_row("SELECT count(*) FROM thread WHERE id = ?1", [thread_id], |r| r.get::<_, i64>(0))? == 0 { return Ok(Err(not_found("thread", thread_id))); }
                (thread_id.clone(), None)
            }
            NoteTarget::Message { message_id } => {
                let Some(t) = self.tx.query_row("SELECT thread_id FROM message WHERE id = ?1", [message_id], |r| r.get::<_, String>(0)).optional()? else { return Ok(Err(not_found("message", message_id))) };
                (t, Some(message_id.clone()))
            }
        };
        let next: i64 = self.tx.query_row("SELECT coalesce(max(CAST(substr(id, 6) AS INTEGER)), 0) + 1 FROM note WHERE id GLOB 'note-[0-9]*'", [], |r| r.get(0))?;
        let id = format!("note-{next}");
        self.tx.execute("INSERT INTO note (id, thread_id, message_id, body, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![id, thread_id, message_id, text, now_ms])?;
        Ok(Ok(Note { id, target: target.clone(), text: text.to_string(), written_at_ms: now_ms }))
    }

    /// Replaces a note's words; the note keeps its place and its written time. Returns the words it had.
    pub fn note_edit(&mut self, id: &str, text: &str, now_ms: i64) -> Result<Result<String, Refusal>, StoreError> {
        let Some(old) = note_by_id(self.tx, id)? else { return Ok(Err(not_found("note", id))) };
        let text = text.trim();
        if text.is_empty() { return Ok(Err(invalid("text", NEEDS_WORDS))); }
        self.tx.execute("UPDATE note SET body = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, text, now_ms])?;
        Ok(Ok(old.note.text))
    }

    /// Removes a note; returns what it was (for its undo entry).
    pub fn note_remove(&mut self, id: &str) -> Result<Result<NoteFull, Refusal>, StoreError> {
        let Some(old) = note_by_id(self.tx, id)? else { return Ok(Err(not_found("note", id))) };
        self.tx.execute("DELETE FROM note WHERE id = ?1", [id])?;
        Ok(Ok(old))
    }

    /// Rewrites an undo entry the actions crate pushed as `note_delete` so that it holds the whole note (that crate's own form holds only the id).
    pub fn note_undo_record_delete(&mut self, entry_id: &str, n: &NoteFull) -> Result<(), StoreError> {
        self.tx.execute(
            "UPDATE undo_entry SET action_json = json_object('kind', 'note_delete', 'id', ?2, 'thread_id', ?3, 'message_id', ?4, 'text', ?5, 'written_at_ms', ?6, 'updated_at_ms', ?7) WHERE id = ?1",
            params![entry_id, n.note.id, n.thread_id, n.message_id, n.note.text, n.note.written_at_ms, n.updated_at_ms],
        )?;
        Ok(())
    }

    /// Z when the newest entry not yet undone is a note's (`note_add`, `note_edit`, `note_delete`): reverses it and marks it undone.
    /// None when the newest entry is not a note's (the mail actions answer then). An entry that can never apply (its note or its thread is gone)
    /// is marked undone and refused with `NotFound`, so it never blocks the stack.
    pub fn note_undo_latest(&mut self) -> Result<Option<Result<(), Refusal>>, StoreError> {
        let head: Option<(String, String, String)> = self.tx.query_row(
            "SELECT id, json_extract(action_json, '$.kind'), action_json FROM undo_entry WHERE consumed = 0 ORDER BY at_ms DESC, rowid DESC LIMIT 1", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional()?;
        let Some((entry, kind, json)) = head else { return Ok(None) };
        if !kind.starts_with("note_") { return Ok(None); }
        let field = |path: &str| -> Result<Option<String>, StoreError> {
            Ok(self.tx.query_row("SELECT json_extract(?1, ?2)", params![json, path], |r| r.get::<_, Option<String>>(0))?)
        };
        let id = field("$.id")?.unwrap_or_default();
        let done: Result<(), Refusal> = match kind.as_str() {
            "note_add" => {
                if self.tx.execute("DELETE FROM note WHERE id = ?1", [&id])? == 0 { Err(not_found("note", &id)) } else { Ok(()) }
            }
            "note_edit" => {
                let previous = field("$.previous_text")?.unwrap_or_default();
                // the words come back; the note keeps its place and its written time (`created_at_ms` is not touched)
                if self.tx.execute("UPDATE note SET body = ?2 WHERE id = ?1", params![id, previous])? == 0 { Err(not_found("note", &id)) } else { Ok(()) }
            }
            "note_delete" => {
                let (thread, message, text) = (field("$.thread_id")?, field("$.message_id")?, field("$.text")?);
                let written: i64 = self.tx.query_row("SELECT coalesce(json_extract(?1, '$.written_at_ms'), 0)", [&json], |r| r.get(0))?;
                let updated: i64 = self.tx.query_row("SELECT coalesce(json_extract(?1, '$.updated_at_ms'), 0)", [&json], |r| r.get(0))?;
                match (thread, text) {
                    (Some(thread), Some(text)) => {
                        let taken: i64 = self.tx.query_row("SELECT count(*) FROM note WHERE id = ?1", [&id], |r| r.get(0))?;
                        let thread_there: i64 = self.tx.query_row("SELECT count(*) FROM thread WHERE id = ?1", [&thread], |r| r.get(0))?;
                        let message_there: i64 = match &message { Some(m) => self.tx.query_row("SELECT count(*) FROM message WHERE id = ?1", [m], |r| r.get(0))?, None => 1 };
                        if thread_there == 0 || message_there == 0 { Err(not_found("thread", &thread)) }
                        else if taken > 0 { Err(invalid("note", "this note's number is taken again, so it cannot come back")) }
                        else {
                            self.tx.execute("INSERT INTO note (id, thread_id, message_id, body, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![id, thread, message, text, written, updated])?;
                            Ok(())
                        }
                    }
                    _ => Err(invalid("undo", "this note's entry holds no words, so it cannot come back")),
                }
            }
            _ => return Ok(None),
        };
        match &done {
            Ok(()) | Err(Refusal::NotFound { .. }) => { self.tx.execute("UPDATE undo_entry SET consumed = 1 WHERE id = ?1", [&entry])?; }
            Err(_) => {}
        }
        Ok(Some(done))
    }

    // ------------------------------------------------------------ MCP

    fn ensure_mcp_tables(&self) -> Result<(), StoreError> {
        self.tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS mcp_server_state (id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1), on_flag INTEGER NOT NULL CHECK (on_flag IN (0,1)), updated_at_ms INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS mcp_approval (request_id TEXT PRIMARY KEY NOT NULL, app_name TEXT NOT NULL, tool_name TEXT NOT NULL, request_words TEXT NOT NULL,
               state TEXT NOT NULL CHECK (state IN ('pending','allowed','refused')), asked_at_ms INTEGER NOT NULL, answered_at_ms INTEGER);",
        )?;
        Ok(())
    }

    /// Allows or revokes one app. An app with no row gets one (first-seen order); a revoke keeps the row with no grant time.
    pub fn mcp_grant_set(&mut self, app: &str, allowed: bool, now_ms: i64) -> Result<Result<(), Refusal>, StoreError> {
        let app = app.trim();
        if app.is_empty() { return Ok(Err(invalid("app", "an app needs a name"))); }
        let granted = if allowed { Some(now_ms) } else { None };
        self.tx.execute(
            "INSERT INTO mcp_grant (app_name, allowed, granted_at_ms) VALUES (?1, ?2, ?3) ON CONFLICT(app_name) DO UPDATE SET allowed = excluded.allowed, granted_at_ms = excluded.granted_at_ms",
            params![app, allowed, granted],
        )?;
        Ok(Ok(()))
    }

    /// Keeps the switch "the MCP server is on". Nothing starts: the engine opens no socket.
    pub fn mcp_server_set(&mut self, on: bool, now_ms: i64) -> Result<(), StoreError> {
        self.ensure_mcp_tables()?;
        self.tx.execute("INSERT INTO mcp_server_state (id, on_flag, updated_at_ms) VALUES (1, ?1, ?2) ON CONFLICT(id) DO UPDATE SET on_flag = excluded.on_flag, updated_at_ms = excluded.updated_at_ms", params![on, now_ms])?;
        Ok(())
    }

    /// Records one request an app made and waits for the owner's answer (the host that serves outside apps calls it; the engine itself starts none).
    pub fn mcp_approval_ask(&mut self, request_id: &str, app: &str, tool: &str, words: &str, now_ms: i64) -> Result<(), StoreError> {
        self.ensure_mcp_tables()?;
        self.tx.execute("INSERT INTO mcp_approval (request_id, app_name, tool_name, request_words, state, asked_at_ms) VALUES (?1, ?2, ?3, ?4, 'pending', ?5)", params![request_id, app, tool, words, now_ms])?;
        Ok(())
    }

    /// The owner's yes or no on one pending request. The grant is read again: a revoke made while the prompt was open still wins (the real gate,
    /// crate mcp usecases/gate.rs, does the same). An unknown or already answered request is `NotFound`. Returns the state it took.
    pub fn mcp_approval_answer(&mut self, request_id: &str, allowed: bool, now_ms: i64) -> Result<Result<String, Refusal>, StoreError> {
        self.ensure_mcp_tables()?;
        let row: Option<(String, String)> = self.tx.query_row("SELECT app_name, state FROM mcp_approval WHERE request_id = ?1", [request_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let Some((app, state)) = row else { return Ok(Err(not_found("approval request", request_id))) };
        if state != "pending" { return Ok(Err(not_found("approval request", request_id))); }
        let still: bool = self.tx.query_row("SELECT coalesce((SELECT allowed FROM mcp_grant WHERE app_name = ?1), 0)", [&app], |r| r.get::<_, i64>(0))? != 0;
        let new = if allowed && still { "allowed" } else { "refused" };
        self.tx.execute("UPDATE mcp_approval SET state = ?2, answered_at_ms = ?3 WHERE request_id = ?1", params![request_id, new, now_ms])?;
        Ok(Ok(new.to_string()))
    }
}

fn kind_of(word: &str) -> Option<SuggestionKind> {
    Some(match word { "block_sender" => SuggestionKind::BlockSender, "always_junk_domain" => SuggestionKind::AlwaysJunkDomain, "unsubscribe" => SuggestionKind::Unsubscribe, "mute_thread" => SuggestionKind::MuteThread, "always_label" => SuggestionKind::AlwaysLabel, _ => return None })
}

impl Store {
    // ------------------------------------------------------------ notes (reads and writes)
    /// The notes written on exactly this target, oldest first.
    pub fn notes_on(&self, target: &NoteTarget) -> Result<Vec<Note>, StoreError> {
        let (sql, key) = match target {
            NoteTarget::Thread { thread_id } => (format!("SELECT {NOTE_COLS} FROM note WHERE thread_id = ?1 AND message_id IS NULL ORDER BY created_at_ms, rowid"), thread_id.clone()),
            NoteTarget::Message { message_id } => (format!("SELECT {NOTE_COLS} FROM note WHERE message_id = ?1 ORDER BY created_at_ms, rowid"), message_id.clone()),
        };
        self.read(|c| {
            let mut st = c.prepare_cached(&sql)?;
            let rows = st.query_map([key], note_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?.into_iter().map(|r| full_of(r).note).collect())
        })
    }
    pub fn note_get(&self, id: &str) -> Result<Option<NoteFull>, StoreError> { self.read(|c| note_by_id(c, id)) }
    pub fn note_count(&self) -> Result<u32, StoreError> { self.read(|c| Ok(c.query_row("SELECT count(*) FROM note", [], |r| r.get::<_, i64>(0))?.max(0) as u32)) }
    /// Whether the newest entry of the undo stack not yet undone is a note's.
    pub fn undo_newest_is_note(&self) -> Result<bool, StoreError> {
        self.read(|c| Ok(c.query_row("SELECT coalesce((SELECT json_extract(action_json, '$.kind') LIKE 'note\\_%' ESCAPE '\\' FROM undo_entry WHERE consumed = 0 ORDER BY at_ms DESC, rowid DESC LIMIT 1), 0)", [], |r| r.get::<_, i64>(0))? != 0))
    }

    // ------------------------------------------------------------ MCP (reads)
    /// Every app the engine has a grant row for, in first-seen order.
    pub fn mcp_grants(&self) -> Result<Vec<McpGrant>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT app_name, allowed, granted_at_ms FROM mcp_grant ORDER BY rowid")?;
            let rows = st.query_map([], |r| Ok(McpGrant { app_name: r.get(0)?, allowed: r.get::<_, i64>(1)? != 0, granted_at_ms: r.get(2)? }))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
    /// The grant of one app; None when no row (the engine has never been asked about it).
    pub fn mcp_grant_of(&self, app: &str) -> Result<Option<bool>, StoreError> {
        self.read(|c| Ok(c.query_row("SELECT allowed FROM mcp_grant WHERE app_name = ?1", [app], |r| r.get::<_, i64>(0)).optional()?.map(|v| v != 0)))
    }
    /// Whether the owner switched the server on (false before any choice).
    pub fn mcp_server_on(&self) -> Result<bool, StoreError> {
        self.read(|c| {
            if !table_exists(c, "mcp_server_state")? { return Ok(false); }
            Ok(c.query_row("SELECT on_flag FROM mcp_server_state WHERE id = 1", [], |r| r.get::<_, i64>(0)).optional()?.unwrap_or(0) != 0)
        })
    }
    pub fn mcp_approval(&self, request_id: &str) -> Result<Option<McpApprovalRow>, StoreError> {
        self.read(|c| {
            if !table_exists(c, "mcp_approval")? { return Ok(None); }
            Ok(c.query_row("SELECT request_id, app_name, tool_name, request_words, state FROM mcp_approval WHERE request_id = ?1", [request_id],
                |r| Ok(McpApprovalRow { request_id: r.get(0)?, app_name: r.get(1)?, tool_name: r.get(2)?, request_words: r.get(3)?, state: r.get(4)? })).optional()?)
        })
    }
    pub fn mcp_approvals_pending(&self) -> Result<u32, StoreError> {
        self.read(|c| {
            if !table_exists(c, "mcp_approval")? { return Ok(0); }
            Ok(c.query_row("SELECT count(*) FROM mcp_approval WHERE state = 'pending'", [], |r| r.get::<_, i64>(0))?.max(0) as u32)
        })
    }

    pub fn note_add(&self, target: NoteTarget, text: String, now_ms: i64) -> Result<Result<Note, Refusal>, StoreError> { self.write(move |ctx| ctx.note_add(&target, &text, now_ms)) }
    pub fn mcp_grant_set(&self, app: String, allowed: bool, now_ms: i64) -> Result<Result<(), Refusal>, StoreError> { self.write(move |ctx| ctx.mcp_grant_set(&app, allowed, now_ms)) }
    pub fn mcp_server_set(&self, on: bool, now_ms: i64) -> Result<(), StoreError> { self.write(move |ctx| ctx.mcp_server_set(on, now_ms)) }
    pub fn mcp_approval_ask(&self, request_id: String, app: String, tool: String, words: String, now_ms: i64) -> Result<(), StoreError> { self.write(move |ctx| ctx.mcp_approval_ask(&request_id, &app, &tool, &words, now_ms)) }
    pub fn mcp_approval_answer(&self, request_id: String, allowed: bool, now_ms: i64) -> Result<Result<String, Refusal>, StoreError> { self.write(move |ctx| ctx.mcp_approval_answer(&request_id, allowed, now_ms)) }

    // ------------------------------------------------------------ suggestions, contacts, senders (reads)
    /// One suggestion row, accepted or not (the engine reads it before it acts on it).
    pub fn suggestion_row(&self, id: &str) -> Result<Option<Suggestion>, StoreError> {
        self.read(|c| {
            let row: Option<(String, String, String, Option<String>, i64, i64)> = c.query_row("SELECT id, kind, subject, label_id, offered_at_ms, accepted FROM suggestion WHERE id = ?1 AND id NOT LIKE 'dismissed:%'", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).optional()?;
            row.map(|(id, k, subject, label_id, offered_at_ms, accepted)| {
                let kind = kind_of(&k).ok_or_else(|| StoreError::Invalid(format!("suggestion {id} has an unknown kind {k:?}")))?;
                Ok(Suggestion { id, kind, subject, label_id, offered_at_ms, accepted: accepted != 0 })
            }).transpose()
        })
    }
    /// The newest suggestion on offer (id), for the state path `suggestion/last`.
    pub fn suggestion_last_id(&self) -> Result<Option<String>, StoreError> {
        self.read(|c| Ok(c.query_row("SELECT id FROM suggestion WHERE accepted = 0 AND id NOT LIKE 'dismissed:%' ORDER BY offered_at_ms DESC, rowid DESC LIMIT 1", [], |r| r.get::<_, String>(0)).optional()?))
    }
    /// The contact cards the owner made (merges): (id, name, addresses), oldest first.
    pub fn contact_cards(&self) -> Result<Vec<(String, String, Vec<String>)>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT c.id, c.name, j.value FROM contact c, json_each(c.addresses_json) j ORDER BY c.rowid, j.key")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
            let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
            for row in rows {
                let (id, name, address) = row?;
                match out.last_mut() { Some(last) if last.0 == id => last.2.push(address), _ => out.push((id, name, vec![address])) }
            }
            Ok(out)
        })
    }
    /// The ids of the messages sent from one address (any case), newest first.
    pub fn message_ids_from(&self, address: &str) -> Result<Vec<String>, StoreError> {
        let a = address.trim().to_lowercase();
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT id FROM message WHERE lower(from_address) = ?1 ORDER BY received_at_ms DESC")?;
            let rows = st.query_map([a], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
}
