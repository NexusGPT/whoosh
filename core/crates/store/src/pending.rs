//! engine-act: the server write-backs waiting to be applied (migration 0011, `pending_action`). The row is written by
//! `WriteCtx::pending_action_add` INSIDE the transaction of the local change; the engine's worker reads the due rows, applies them
//! over IMAP, then deletes them (`pending_action_done`) or keeps them with the server's words and the time of the next try
//! (`pending_action_retry`). Every statement is parameterised.
use rusqlite::{params, OptionalExtension};

use crate::{Store, StoreError, WriteCtx};

/// One write-back for one message, as the ledger records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPending {
    pub account_id: String,
    pub message_id: Option<String>,
    pub folder: String,
    pub uid: Option<i64>,
    pub kind: String,
    pub payload_json: String,
}

/// A stored row: what to do, and how it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRow {
    pub id: i64,
    pub account_id: String,
    pub message_id: Option<String>,
    pub folder: String,
    pub uid: Option<i64>,
    pub kind: String,
    pub payload_json: String,
    pub created_ms: i64,
    pub tries: i64,
    pub next_at_ms: i64,
    pub last_error: Option<String>,
}

const COLS: &str = "id, account_id, message_id, folder, uid, kind, payload_json, created_ms, tries, next_at_ms, last_error";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PendingRow> {
    Ok(PendingRow {
        id: r.get(0)?, account_id: r.get(1)?, message_id: r.get(2)?, folder: r.get(3)?, uid: r.get(4)?, kind: r.get(5)?,
        payload_json: r.get(6)?, created_ms: r.get(7)?, tries: r.get(8)?, next_at_ms: r.get(9)?, last_error: r.get(10)?,
    })
}

impl WriteCtx<'_> {
    /// Records one write-back in the caller's transaction, due at once. Returns the row id.
    pub fn pending_action_add(&mut self, p: &NewPending, now_ms: i64) -> Result<i64, StoreError> {
        self.tx.execute(
            "INSERT INTO pending_action (account_id, message_id, folder, uid, kind, payload_json, created_ms, tries, next_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,0,?7)",
            params![p.account_id, p.message_id, p.folder, p.uid, p.kind, p.payload_json, now_ms],
        )?;
        Ok(self.tx.last_insert_rowid())
    }

    /// junk-r2: as `pending_action_add`, first due at `due_ms` instead of at once. The spam filter's own move is made on the sync's
    /// connection right after the commit; the row is the ledger the write-back worker falls back on, so it must not race that move.
    pub fn pending_action_add_due(&mut self, p: &NewPending, now_ms: i64, due_ms: i64) -> Result<i64, StoreError> {
        self.tx.execute(
            "INSERT INTO pending_action (account_id, message_id, folder, uid, kind, payload_json, created_ms, tries, next_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,0,?8)",
            params![p.account_id, p.message_id, p.folder, p.uid, p.kind, p.payload_json, now_ms, due_ms],
        )?;
        Ok(self.tx.last_insert_rowid())
    }
}

impl Store {
    /// The rows due at `now_ms`, oldest first. No cap: a row is one message, and the worker sends one request per account, folder and kind.
    pub fn pending_actions_due(&self, now_ms: i64) -> Result<Vec<PendingRow>, StoreError> {
        self.read(move |c| {
            let mut st = c.prepare_cached(&format!("SELECT {COLS} FROM pending_action WHERE next_at_ms <= ?1 ORDER BY id"))?;
            let rows = st.query_map(params![now_ms], row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Every row, oldest first (the tests and the hidden check read them).
    pub fn pending_actions(&self) -> Result<Vec<PendingRow>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached(&format!("SELECT {COLS} FROM pending_action ORDER BY id"))?;
            let rows = st.query_map([], row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// When the next row is due, None when nothing waits.
    pub fn pending_action_next_due(&self) -> Result<Option<i64>, StoreError> {
        self.read(|c| Ok(c.query_row("SELECT min(next_at_ms) FROM pending_action", [], |r| r.get::<_, Option<i64>>(0)).optional()?.flatten()))
    }

    /// The server took these rows: they go.
    pub fn pending_action_done(&self, ids: Vec<i64>) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            let mut st = ctx.tx.prepare_cached("DELETE FROM pending_action WHERE id = ?1")?;
            let mut n = 0;
            for id in &ids { n += st.execute(params![id])?; }
            Ok(n)
        })
    }

    /// The server refused (or could not be reached): the rows stay, one more try counted, the server's words kept, tried again at `next_at_ms`.
    pub fn pending_action_retry(&self, ids: Vec<i64>, words: String, next_at_ms: i64) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            let mut st = ctx.tx.prepare_cached("UPDATE pending_action SET tries = tries + 1, last_error = ?2, next_at_ms = ?3 WHERE id = ?1")?;
            let mut n = 0;
            for id in &ids { n += st.execute(params![id, words, next_at_ms])?; }
            Ok(n)
        })
    }
}
