//! Contracts delta 18, the store side of the mail import (migration 0010): imported mail names its source and the time the run stored
//! it, a source's history is DERIVED from those rows (no table), and the local "Imported" account that takes mail no address matches.
//! One rule, repeated by the migration's triggers and by the contract's `imported_row_is_legal`: an imported row has both marks.
use crate::error::StoreError;
use crate::ops::NewMessage;
use crate::Store;
use rusqlite::{params, OptionalExtension};

impl Store {
    /// The import's write: the batch goes in through the sync's own insert (same threads, same counts, same events) and every row that
    /// is NEW gets `source = 'imported'`, the source id and the import time, in the same transaction. A message already stored keeps
    /// the marks it has (an earlier run's). Returns the ids of the new rows, in batch order.
    pub fn insert_imported(&self, batch: Vec<NewMessage>, import_source_id: &str, imported_at_ms: i64) -> Result<Vec<String>, StoreError> {
        let source = import_source_id.to_string();
        self.write(move |ctx| {
            let mut fresh: Vec<String> = Vec::new();
            {
                let mut known = ctx.tx.prepare_cached("SELECT EXISTS (SELECT 1 FROM message WHERE id = ?1)")?;
                for n in &batch {
                    if !known.query_row(params![n.message.id], |r| r.get::<_, bool>(0))? && !fresh.contains(&n.message.id) {
                        fresh.push(n.message.id.clone());
                    }
                }
            }
            ctx.insert_batch(&batch)?;
            let mut mark = ctx.tx.prepare_cached(
                "UPDATE message SET source = 'imported', import_source_id = ?2, imported_at_ms = ?3 WHERE id = ?1 AND source = 'synced'",
            )?;
            for id in &fresh {
                mark.execute(params![id, source, imported_at_ms])?;
            }
            Ok(fresh)
        })
    }

    /// What earlier runs of one source stored: how many rows carry its id, and when the newest was stored. Derived, never kept twice.
    pub fn imported_history(&self, import_source_id: &str) -> Result<(u64, Option<i64>), StoreError> {
        let id = import_source_id.to_string();
        self.read(move |c| {
            Ok(c.query_row(
                "SELECT count(*), max(imported_at_ms) FROM message WHERE source = 'imported' AND import_source_id = ?1",
                params![id],
                |r| Ok((r.get::<_, i64>(0)?.max(0) as u64, r.get::<_, Option<i64>>(1)?)),
            )?)
        })
    }

    /// The local account that takes imported mail no account address matches. It has no server configuration, so the sync never
    /// starts it and nothing is ever sent from it. An account with this address already on the Mac is returned as it is.
    pub fn add_local_account(&self, address: &str, display_name: &str, now_ms: i64) -> Result<String, StoreError> {
        let address = address.trim().to_lowercase();
        let name = if display_name.trim().is_empty() { address.clone() } else { display_name.trim().to_string() };
        let id = crate::new_id("acc");
        self.write(move |ctx| {
            let existing: Option<String> = ctx.tx.query_row("SELECT id FROM account WHERE lower(address) = ?1", params![address], |r| r.get(0)).optional()?;
            if let Some(found) = existing {
                return Ok(found);
            }
            let n: i64 = ctx.tx.query_row("SELECT count(*) FROM account", [], |r| r.get(0))?;
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES (?1, ?2, 'imap', ?3, '#8E8E93', ?4, 'manual', 'whoosh.local' || char(31) || 'imported', ?5)",
                params![id, address, name, n, now_ms],
            )?;
            Ok(id)
        })
    }

    /// The flagged messages of an import become Starred (as the sync does for `\Flagged`), for the rows THIS run stored only: a star the
    /// owner took off after an earlier run is not put back. Returns how many were starred.
    pub fn star_imported_in_run(&self, ids: Vec<String>, import_source_id: &str, imported_at_ms: i64) -> Result<usize, StoreError> {
        let source = import_source_id.to_string();
        self.write(move |ctx| {
            ctx.will_change(&ids)?;
            let mut st = ctx.tx.prepare_cached(
                "UPDATE message SET starred = 1 WHERE id = ?1 AND source = 'imported' AND import_source_id = ?2 AND imported_at_ms = ?3 AND starred = 0",
            )?;
            let mut n = 0;
            for id in &ids {
                n += st.execute(params![id, source, imported_at_ms])?;
            }
            Ok(n)
        })
    }
}
