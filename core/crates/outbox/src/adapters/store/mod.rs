//! The store crate's SQLite behind the persistence ports: `outbox_entry`, the draft rows, account names.
//! Every statement is parameterised; every write is one transaction on the store's single writer.
use crate::domain::{draft_from_value, draft_to_value, from_row, to_row, Record, RowData};
use crate::ports::{AccountNamePort, DraftEffect, FindByKeyPort, GetEntryPort, ListOpenEntriesPort, ListPendingCopiesPort, LoadDraftPort, SaveEntryPort, SaveRequest};
use contracts::{CoreError, Draft};
use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::{json, Value};
use std::sync::Arc;
use store::{Store, StoreError};

const COLUMNS: &str = "id, draft_json, state, due_at_ms, sent_at_ms, failure_json, attempts, created_at_ms, updated_at_ms";

pub struct StoreOutbox {
    store: Arc<Store>,
}

impl StoreOutbox {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

fn unreachable_store(what: &str, e: StoreError) -> CoreError {
    CoreError::Unexpected { reason_words: format!("the outbox could not {what}: {e}") }
}

fn row_data(r: &rusqlite::Row<'_>) -> rusqlite::Result<RowData> {
    Ok(RowData {
        id: r.get(0)?, draft_json: r.get(1)?, state: r.get(2)?, due_at_ms: r.get(3)?, sent_at_ms: r.get(4)?,
        failure_json: r.get(5)?, attempts: r.get(6)?, created_at_ms: r.get(7)?, updated_at_ms: r.get(8)?,
    })
}

/// delta 22: the draft's files follow it: re-owned to the outbox entry BEFORE the draft row goes (its delete trigger takes the rows a draft
/// still owns), and back to the draft when the entry returns to editing (undo, cancel). Same transaction as the entry and the draft.
fn apply_draft_effect(ctx: &mut store::WriteCtx<'_>, entry_id: &str, effect: &DraftEffect, now_ms: i64) -> Result<(), StoreError> {
    match effect {
        DraftEffect::Keep => Ok(()),
        DraftEffect::Remove { draft_id } => {
            ctx.files_to_outbox(draft_id, entry_id)?;
            ctx.tx.execute("DELETE FROM draft WHERE id = ?1", [draft_id])?;
            Ok(())
        }
        DraftEffect::Restore(d) => {
            restore_draft_row(ctx.tx, d, now_ms)?;
            ctx.files_to_draft(entry_id, &d.id)?;
            Ok(())
        }
    }
}

fn restore_draft_row(tx: &Transaction<'_>, d: &Draft, now_ms: i64) -> rusqlite::Result<()> {
            let v = draft_to_value(d);
            tx.execute(
                "INSERT INTO draft (id, from_account_id, to_json, cc_json, subject, body, attachment_ids_json, ai_marks_json, reply_to_thread_id, updated_at_ms) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) \
                 ON CONFLICT(id) DO UPDATE SET from_account_id=excluded.from_account_id, to_json=excluded.to_json, cc_json=excluded.cc_json, subject=excluded.subject, \
                 body=excluded.body, attachment_ids_json=excluded.attachment_ids_json, ai_marks_json=excluded.ai_marks_json, reply_to_thread_id=excluded.reply_to_thread_id, updated_at_ms=excluded.updated_at_ms",
                params![d.id, d.from_account_id, v["to"].to_string(), v["cc"].to_string(), d.subject, d.body, v["attachment_ids"].to_string(), v["ai_marks"].to_string(), d.reply_to_thread_id, now_ms],
            )
            .map(|_| ())
}

impl SaveEntryPort for StoreOutbox {
    /// engine-act: the record does not carry the reply reminder (engine-actions stores it in the envelope, `remind_if_no_reply_at_ms`);
    /// an update keeps the time the stored row holds, so a state change (sending, sent) never drops it. Clearing it is the store's
    /// own `outbox_reply_reminder_clear`, which this upsert then keeps cleared.
    fn execute(&self, request: &SaveRequest) -> Result<(), CoreError> {
        let row = to_row(&request.record);
        let effect = request.draft.clone();
        let entry_id = request.record.entry.id.clone();
        let now = request.record.updated_at_ms;
        self.store
            .write(move |ctx| {
                ctx.tx.execute(
                    &format!(
                        "INSERT INTO outbox_entry ({COLUMNS}) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) \
                         ON CONFLICT(id) DO UPDATE SET draft_json=CASE WHEN json_extract(outbox_entry.draft_json, '$.remind_if_no_reply_at_ms') IS NULL THEN excluded.draft_json \
                         ELSE json_set(excluded.draft_json, '$.remind_if_no_reply_at_ms', json_extract(outbox_entry.draft_json, '$.remind_if_no_reply_at_ms')) END, state=excluded.state, due_at_ms=excluded.due_at_ms, sent_at_ms=excluded.sent_at_ms, \
                         failure_json=excluded.failure_json, attempts=excluded.attempts, updated_at_ms=excluded.updated_at_ms"
                    ),
                    params![row.id, row.draft_json, row.state, row.due_at_ms, row.sent_at_ms, row.failure_json, row.attempts, row.created_at_ms, row.updated_at_ms],
                )?;
                apply_draft_effect(ctx, &entry_id, &effect, now)?;
                Ok(())
            })
            .map_err(|e| unreachable_store("save an entry", e))
    }
}

impl GetEntryPort for StoreOutbox {
    fn execute(&self, id: &str) -> Result<Option<Record>, CoreError> {
        let id = id.to_string();
        let row = self
            .store
            .write(move |ctx| Ok(ctx.tx.query_row(&format!("SELECT {COLUMNS} FROM outbox_entry WHERE id = ?1"), [&id], row_data).optional()?))
            .map_err(|e| unreachable_store("read an entry", e))?;
        row.map(from_row).transpose()
    }
}

impl ListOpenEntriesPort for StoreOutbox {
    fn execute(&self) -> Result<Vec<Record>, CoreError> {
        let rows = self
            .store
            .write(|ctx| {
                let mut stmt = ctx.tx.prepare(&format!(
                    "SELECT {COLUMNS} FROM outbox_entry WHERE state IN ('undoable','scheduled','sending','failed') ORDER BY created_at_ms, id"
                ))?;
                let rows = stmt.query_map([], row_data)?.collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .map_err(|e| unreachable_store("list the open entries", e))?;
        rows.into_iter().map(from_row).collect()
    }
}

impl FindByKeyPort for StoreOutbox {
    /// By the key in the row's envelope; a row written before keys existed answers to its own id.
    fn execute(&self, key: &str) -> Result<Vec<Record>, CoreError> {
        let key = key.to_string();
        let rows = self
            .store
            .write(move |ctx| {
                let mut stmt = ctx.tx.prepare(&format!(
                    "SELECT {COLUMNS} FROM outbox_entry WHERE json_extract(draft_json, '$.idempotency_key') = ?1 \
                     OR (id = ?1 AND json_extract(draft_json, '$.idempotency_key') IS NULL) ORDER BY created_at_ms, id"
                ))?;
                let rows = stmt.query_map([&key], row_data)?.collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .map_err(|e| unreachable_store("look an entry up by its key", e))?;
        rows.into_iter().map(from_row).collect()
    }
}

impl ListPendingCopiesPort for StoreOutbox {
    fn execute(&self) -> Result<Vec<Record>, CoreError> {
        let rows = self
            .store
            .write(|ctx| {
                let mut stmt = ctx.tx.prepare(&format!(
                    "SELECT {COLUMNS} FROM outbox_entry WHERE state = 'sent' AND json_extract(draft_json, '$.copy_pending') IS NOT NULL ORDER BY created_at_ms, id"
                ))?;
                let rows = stmt.query_map([], row_data)?.collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(rows)
            })
            .map_err(|e| unreachable_store("list the unsaved Sent copies", e))?;
        rows.into_iter().map(from_row).collect()
    }
}

impl LoadDraftPort for StoreOutbox {
    fn execute(&self, draft_id: &str) -> Result<Draft, CoreError> {
        let id = draft_id.to_string();
        let lookup = id.clone();
        let value: Option<Value> = self
            .store
            .write(move |ctx| {
                Ok(ctx
                    .tx
                    .query_row(
                        "SELECT id, from_account_id, to_json, cc_json, subject, body, attachment_ids_json, ai_marks_json, reply_to_thread_id, bcc_json FROM draft WHERE id = ?1",
                        [&lookup],
                        |r| {
                            let json_col = |i: usize| -> rusqlite::Result<Value> {
                                let s: String = r.get(i)?;
                                serde_json::from_str(&s).map_err(|e| rusqlite::Error::FromSqlConversionFailure(i, rusqlite::types::Type::Text, Box::new(e)))
                            };
                            Ok(json!({
                                "id": r.get::<_, String>(0)?, "from_account_id": r.get::<_, String>(1)?, "to": json_col(2)?, "cc": json_col(3)?,
                                "subject": r.get::<_, String>(4)?, "body": r.get::<_, String>(5)?, "attachment_ids": json_col(6)?, "ai_marks": json_col(7)?,
                                "reply_to_thread_id": r.get::<_, Option<String>>(8)?, "bcc": json_col(9)?,
                            }))
                        },
                    )
                    .optional()?)
            })
            .map_err(|e| unreachable_store("read the draft", e))?;
        match value {
            // delta 22: `attachment_ids` is the list of the draft's `outgoing_attachment` rows (the rows are the truth, the column a copy)
            Some(v) => {
                let mut draft = draft_from_value(&v)?;
                let rows = self.store.draft_files(&id).map_err(|e| unreachable_store("read the draft's files", e))?;
                draft.attachment_ids = rows.into_iter().map(|a| a.id).collect();
                Ok(draft)
            }
            None => Err(CoreError::NotFound { kind: "draft".into(), id }),
        }
    }
}

impl AccountNamePort for StoreOutbox {
    fn execute(&self, account_id: &str) -> String {
        let id = account_id.to_string();
        let lookup = id.clone();
        self.store
            .write(move |ctx| Ok(ctx.tx.query_row("SELECT display_name FROM account WHERE id = ?1", [&lookup], |r| r.get::<_, String>(0)).optional()?))
            .ok()
            .flatten()
            .unwrap_or(id)
    }
}
