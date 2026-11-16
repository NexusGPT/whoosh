//! Typed label operations. A label is named by its path ("Clients > Provanta"); the database keeps `path` and `depth`
//! true with triggers (migration 0002), and the triggers refuse a bad write in words. These operations never write
//! `path` or `depth` themselves: they write name and parent, then read the derived columns back.
//!
//! Every operation is a method of `WriteCtx`, so a use case can run it inside its own transaction (the actions engine
//! applies a change and its undo entry in one commit). The `Store` methods at the bottom are the one-transaction forms.
use crate::error::StoreError;
use crate::writer::WriteCtx;
use crate::Store;
use rusqlite::{params, Connection, ErrorCode, OptionalExtension};
use std::sync::atomic::{AtomicU64, Ordering};

const LEVEL_SEPARATOR: char = '>';

/// A label as stored. `path` and `depth` are derived by the database.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelRecord {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub path: String,
    pub depth: u32,
    pub colour: Option<String>,
    /// Empty = the label applies to every account.
    pub account_ids: Vec<String>,
    pub updated_at_ms: i64,
}

/// What `label_create` needs. `id` None = the store makes one.
#[derive(Debug, Clone, Default)]
pub struct NewLabel {
    pub id: Option<String>,
    pub name: String,
    pub parent_id: Option<String>,
    pub colour: Option<String>,
    pub account_ids: Vec<String>,
    pub now_ms: i64,
}

/// The answer of `label_ensure_path`: the leaf, and the ids this call created, ancestors first.
#[derive(Debug, Clone, PartialEq)]
pub struct EnsuredLabel {
    pub label: LabelRecord,
    pub created: Vec<String>,
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A fresh id: prefix, nanoseconds, a process counter.
pub fn new_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{prefix}-{nanos:x}-{}", SEQ.fetch_add(1, Ordering::Relaxed))
}

/// A JSON array of strings, escaped (the schema checks json_valid on account_ids_json).
fn json_list(items: &[String]) -> String {
    crate::ops::json_strings(items)
}

/// A refusal written by one of the label triggers is already in words: keep its text. Any other database error keeps its
/// real SQLite message.
pub(crate) fn in_words(e: rusqlite::Error) -> StoreError {
    if let rusqlite::Error::SqliteFailure(code, Some(message)) = &e {
        if code.code == ErrorCode::ConstraintViolation && code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER {
            return StoreError::Invalid(message.clone());
        }
    }
    StoreError::Sqlite(e)
}

fn missing(id: &str) -> StoreError {
    StoreError::Invalid(format!("the label {id} does not exist"))
}

fn read_label(r: &rusqlite::Row<'_>) -> rusqlite::Result<(LabelRecord, String)> {
    let accounts: String = r.get(5)?;
    let depth: Option<i64> = r.get(7)?;
    let path: Option<String> = r.get(6)?;
    Ok((
        LabelRecord {
            id: r.get(0)?,
            name: r.get(1)?,
            parent_id: r.get(2)?,
            colour: r.get(3)?,
            updated_at_ms: r.get(4)?,
            account_ids: Vec::new(),
            path: path.unwrap_or_default(),
            depth: depth.unwrap_or(0).max(0) as u32,
        },
        accounts,
    ))
}

const SELECT_LABEL: &str = "SELECT id, name, parent_id, colour, updated_at_ms, account_ids_json, path, depth FROM label";

fn label_from(conn: &Connection, sql: &str, arg: &str) -> Result<Option<LabelRecord>, StoreError> {
    let row = conn.prepare_cached(sql)?.query_row([arg], read_label).optional()?;
    let Some((mut label, accounts)) = row else { return Ok(None) };
    let mut st = conn.prepare_cached("SELECT value FROM json_each(?1)")?;
    label.account_ids = st.query_map([&accounts], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(Some(label))
}

/// One label by id.
pub fn get_label(conn: &Connection, id: &str) -> Result<Option<LabelRecord>, StoreError> {
    label_from(conn, &format!("{SELECT_LABEL} WHERE id = ?1"), id)
}

/// One label by its path, the case ignored (Gmail label names are case-insensitive). The oldest one when two share a path.
pub fn find_label(conn: &Connection, path: &str) -> Result<Option<LabelRecord>, StoreError> {
    label_from(conn, &format!("{SELECT_LABEL} WHERE lower(path) = lower(?1) ORDER BY rowid LIMIT 1"), path)
}

/// Every label, parents before children (by path).
pub fn all_labels(conn: &Connection) -> Result<Vec<LabelRecord>, StoreError> {
    let ids: Vec<String> = {
        let mut st = conn.prepare_cached("SELECT id FROM label ORDER BY path, rowid")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(l) = get_label(conn, &id)? {
            out.push(l);
        }
    }
    Ok(out)
}

impl WriteCtx<'_> {
    /// One label by id.
    pub fn label_get(&self, id: &str) -> Result<Option<LabelRecord>, StoreError> {
        get_label(self.tx, id)
    }

    /// One label by its path, the case ignored. The oldest one when two share a path.
    pub fn label_find_by_path(&self, path: &str) -> Result<Option<LabelRecord>, StoreError> {
        find_label(self.tx, path)
    }

    /// Create a label under a parent (or at the root). The database refuses, in words: an empty name, a ">" in the name,
    /// spaces at the ends, a sixth level. A parent that does not exist is refused here, in words.
    pub fn label_create(&mut self, new: NewLabel) -> Result<LabelRecord, StoreError> {
        if let Some(parent) = &new.parent_id {
            if self.label_get(parent)?.is_none() {
                return Err(StoreError::Invalid(format!("the parent label {parent} does not exist")));
            }
        }
        let id = new.id.clone().unwrap_or_else(|| new_id("label"));
        self.tx
            .execute(
                "INSERT INTO label (id, name, parent_id, colour, account_ids_json, updated_at_ms, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, (SELECT coalesce(max(sort_order) + 1, 0) FROM label WHERE parent_id IS ?3))",
                params![id, new.name, new.parent_id, new.colour, json_list(&new.account_ids), new.now_ms],
            )
            .map_err(in_words)?;
        let made = self.label_get(&id)?.ok_or_else(|| missing(&id))?;
        if made.path.is_empty() || made.depth == 0 {
            return Err(StoreError::Invalid(format!("the label {id} was stored without its path: the database's label rules are not installed")));
        }
        Ok(made)
    }

    /// The label at `path`, with every missing level created under its parent. "Clients > Provanta" makes "Clients" first when
    /// it is new. A path needs a name at every level.
    pub fn label_ensure_path(&mut self, path: &str, account_ids: &[String], now_ms: i64) -> Result<EnsuredLabel, StoreError> {
        let names: Vec<&str> = path.split(LEVEL_SEPARATOR).map(str::trim).collect();
        if names.iter().any(|n| n.is_empty()) {
            return Err(StoreError::Invalid("a label needs a name at every level".into()));
        }
        // All or nothing: a refusal at level 4 must not leave levels 1 to 3 behind.
        self.tx.execute_batch("SAVEPOINT label_ensure_path")?;
        match self.ensure_levels(&names, account_ids, now_ms) {
            Ok(done) => {
                self.tx.execute_batch("RELEASE label_ensure_path")?;
                Ok(done)
            }
            Err(e) => {
                self.tx.execute_batch("ROLLBACK TO label_ensure_path; RELEASE label_ensure_path")?;
                Err(e)
            }
        }
    }

    fn ensure_levels(&mut self, names: &[&str], account_ids: &[String], now_ms: i64) -> Result<EnsuredLabel, StoreError> {
        let mut created = Vec::new();
        let mut parent: Option<LabelRecord> = None;
        for (i, name) in names.iter().enumerate() {
            let so_far = names[..=i].join(" > ");
            let level = match self.label_find_by_path(&so_far)? {
                Some(found) => found,
                None => {
                    let made = self.label_create(NewLabel {
                        id: None,
                        name: (*name).to_string(),
                        parent_id: parent.as_ref().map(|p| p.id.clone()),
                        colour: None,
                        account_ids: account_ids.to_vec(),
                        now_ms,
                    })?;
                    created.push(made.id.clone());
                    made
                }
            };
            parent = Some(level);
        }
        let label = parent.ok_or_else(|| StoreError::Invalid("a label needs a name".into()))?;
        Ok(EnsuredLabel { label, created })
    }

    /// Let a label that is limited to some accounts also apply to `accounts`. A label with an empty list applies to every
    /// account already, so it is left alone. Returns whether the stored list changed.
    pub fn label_add_accounts(&mut self, id: &str, accounts: &[String], now_ms: i64) -> Result<bool, StoreError> {
        let label = self.label_get(id)?.ok_or_else(|| missing(id))?;
        if label.account_ids.is_empty() {
            return Ok(false);
        }
        let mut all = label.account_ids.clone();
        for a in accounts {
            if !all.contains(a) {
                all.push(a.clone());
            }
        }
        if all.len() == label.account_ids.len() {
            return Ok(false);
        }
        self.tx.execute("UPDATE label SET account_ids_json = ?1, updated_at_ms = ?2 WHERE id = ?3", params![json_list(&all), now_ms, id])?;
        Ok(true)
    }

    /// Rename a label. The database rewrites the path of the label and of every sub-label in the same statement.
    pub fn label_rename(&mut self, id: &str, new_name: &str, now_ms: i64) -> Result<LabelRecord, StoreError> {
        if self.label_get(id)?.is_none() {
            return Err(missing(id));
        }
        self.tx.execute("UPDATE label SET name = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, new_name, now_ms]).map_err(in_words)?;
        self.label_get(id)?.ok_or_else(|| missing(id))
    }

    /// Move a label, with its sub-labels, under another parent, or to the root (`None`). The database refuses, in words, a move
    /// under itself or under one of its own sub-labels, and a move that would push a sub-label past the deepest level.
    pub fn label_move(&mut self, id: &str, new_parent: Option<&str>, now_ms: i64) -> Result<LabelRecord, StoreError> {
        if self.label_get(id)?.is_none() {
            return Err(missing(id));
        }
        if let Some(p) = new_parent {
            if self.label_get(p)?.is_none() {
                return Err(StoreError::Invalid(format!("the parent label {p} does not exist")));
            }
        }
        self.tx.execute("UPDATE label SET parent_id = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, new_parent, now_ms]).map_err(in_words)?;
        self.label_get(id)?.ok_or_else(|| missing(id))
    }

    /// Delete a label. Its assignments go with it; its sub-labels are promoted one level (the database re-paths them).
    pub fn label_delete(&mut self, id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.execute("DELETE FROM label WHERE id = ?1", params![id]).map_err(in_words)? > 0)
    }

    /// Delete the labels in `ids` that nothing uses: no assignment and no sub-label. Newest first, so a leaf goes before its
    /// parent. This is the undo of `label_ensure_path`.
    pub fn label_delete_unused(&mut self, ids: &[String]) -> Result<usize, StoreError> {
        let mut st = self.tx.prepare_cached(
            "DELETE FROM label WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM label_assignment WHERE label_id = ?1) \
             AND NOT EXISTS (SELECT 1 FROM label c WHERE c.parent_id = ?1)",
        )?;
        let mut removed = 0;
        for id in ids.iter().rev() {
            removed += st.execute(params![id])?;
        }
        Ok(removed)
    }

    /// Put the label on the messages that do not carry it. Returns the ids that gained it. The messages are declared first.
    pub fn label_assign(&mut self, label_id: &str, message_ids: &[String], now_ms: i64) -> Result<Vec<String>, StoreError> {
        let fresh = self.label_carriers(label_id, message_ids, false)?;
        self.will_change(&fresh)?;
        let mut st = self.tx.prepare_cached("INSERT OR IGNORE INTO label_assignment (label_id, message_id, updated_at_ms) VALUES (?1, ?2, ?3)")?;
        for id in &fresh {
            st.execute(params![label_id, id, now_ms])?;
        }
        Ok(fresh)
    }

    /// Take the label off the messages that carry it. Returns the ids that lost it. The messages are declared first.
    pub fn label_unassign(&mut self, label_id: &str, message_ids: &[String]) -> Result<Vec<String>, StoreError> {
        let carrying = self.label_carriers(label_id, message_ids, true)?;
        self.will_change(&carrying)?;
        let mut st = self.tx.prepare_cached("DELETE FROM label_assignment WHERE label_id = ?1 AND message_id = ?2")?;
        for id in &carrying {
            st.execute(params![label_id, id])?;
        }
        drop(st);
        for id in &carrying {
            self.builtin_tab_drop(label_id, id)?;
        }
        Ok(carrying)
    }

    /// Of `message_ids`, the ones that carry the label (`carrying` true) or do not (false), in the order given.
    pub fn label_carriers(&self, label_id: &str, message_ids: &[String], carrying: bool) -> Result<Vec<String>, StoreError> {
        let mut st = self.tx.prepare_cached("SELECT 1 FROM label_assignment WHERE label_id = ?1 AND message_id = ?2")?;
        let mut out = Vec::new();
        for id in message_ids {
            if st.exists(params![label_id, id])? == carrying {
                out.push(id.clone());
            }
        }
        Ok(out)
    }
}

impl Store {
    /// Create a label (see `WriteCtx::label_create`) in its own transaction.
    pub fn label_create(&self, new: NewLabel) -> Result<LabelRecord, StoreError> {
        self.write(move |ctx| ctx.label_create(new))
    }

    pub fn label_ensure_path(&self, path: String, account_ids: Vec<String>, now_ms: i64) -> Result<EnsuredLabel, StoreError> {
        self.write(move |ctx| ctx.label_ensure_path(&path, &account_ids, now_ms))
    }

    pub fn label_rename(&self, id: String, new_name: String, now_ms: i64) -> Result<LabelRecord, StoreError> {
        self.write(move |ctx| ctx.label_rename(&id, &new_name, now_ms))
    }

    pub fn label_move(&self, id: String, new_parent: Option<String>, now_ms: i64) -> Result<LabelRecord, StoreError> {
        self.write(move |ctx| ctx.label_move(&id, new_parent.as_deref(), now_ms))
    }

    pub fn label_delete(&self, id: String) -> Result<bool, StoreError> {
        self.write(move |ctx| ctx.label_delete(&id))
    }

    pub fn label_assign(&self, label_id: String, message_ids: Vec<String>, now_ms: i64) -> Result<Vec<String>, StoreError> {
        self.write(move |ctx| ctx.label_assign(&label_id, &message_ids, now_ms))
    }

    pub fn label_unassign(&self, label_id: String, message_ids: Vec<String>) -> Result<Vec<String>, StoreError> {
        self.write(move |ctx| ctx.label_unassign(&label_id, &message_ids))
    }

    /// Every label, parents before children (by path).
    pub fn labels(&self) -> Result<Vec<LabelRecord>, StoreError> {
        self.read(all_labels)
    }

    /// real-labels: the unread inbox mail filed under each label (the same VISIBLE rows as a tab), for the `labels/tree` and
    /// `labels/pinned` state paths. A label with none is absent. One grouped walk of the assignments.
    pub fn label_unread_counts(&self) -> Result<std::collections::HashMap<String, u32>, StoreError> {
        self.read(|c| {
            let sql = format!(
                "SELECT la.label_id, count(*) FROM label_assignment la JOIN message m ON m.id = la.message_id WHERE m.read = 0 AND {} GROUP BY la.label_id",
                crate::read::VISIBLE
            );
            let mut st = c.prepare_cached(&sql)?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            let mut out = std::collections::HashMap::new();
            for row in rows {
                let (id, n) = row?;
                out.insert(id, n.max(0) as u32);
            }
            Ok(out)
        })
    }

    /// real-labels: the names of the labels a message is filed under, by name (state path `label_assignment/<message>`).
    pub fn label_names_of(&self, message_id: &str) -> Result<Vec<String>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT l.name FROM label_assignment la JOIN label l ON l.id = la.label_id WHERE la.message_id = ?1 ORDER BY l.name, l.id")?;
            let names = st.query_map(params![message_id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            Ok(names)
        })
    }

    /// real-labels: whether the owner took this label off the message's thread (the read form of `WriteCtx::label_removed_by_user`,
    /// on the reader: a state read never queues behind the writer). Message unknown = false.
    pub fn label_removed(&self, label_id: &str, message_id: &str) -> Result<bool, StoreError> {
        self.read(|c| {
            Ok(c.query_row(
                "SELECT EXISTS (SELECT 1 FROM message m JOIN thread t ON t.id = m.thread_id, json_each(t.labels_removed_json) j WHERE m.id = ?1 AND j.value = ?2)",
                params![message_id, label_id],
                |r| r.get::<_, bool>(0),
            )?)
        })
    }

    /// The newest `limit` messages under a label, its sub-labels included (the descendant query by path prefix).
    pub fn messages_under_label(&self, label_id: &str, limit: usize) -> Result<Vec<contracts::InboxRow>, StoreError> {
        self.read(|c| crate::read::messages_under_label(c, label_id, limit))
    }
}
