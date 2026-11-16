//! The writer side. One thread owns the tantivy `IndexWriter` and a read-only SQLite connection and does
//! all the work: build from the message table, catch up on new rows, and reconcile with the table.
//!
//! The commit payload carries the rowid watermarks, so the watermark and the documents are one atomic
//! unit: a crash can leave the index behind the table but never ahead of it, and an index with no
//! readable payload is rebuilt from the table.
use crate::error::Error;
use crate::limits::{BATCH_ROWS, RECHECK_TAIL_ROWS, RECONCILE_WINDOW_ROWIDS};
use crate::schema::Fields;
use crate::source::{self, Indexed};
use rusqlite::Connection;
use std::collections::HashMap;
use tantivy::schema::{Field, Value};
use tantivy::{Index, IndexReader, IndexWriter, TantivyDocument, Term};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Watermarks {
    pub message: i64,
    pub attachment: i64,
}

impl Watermarks {
    pub fn to_payload(self) -> String {
        format!("{{\"v\":1,\"message\":{},\"attachment\":{}}}", self.message, self.attachment)
    }
    pub fn from_payload(text: &str) -> Option<Watermarks> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        if v.get("v")?.as_i64()? != 1 {
            return None;
        }
        Some(Watermarks { message: v.get("message")?.as_i64()?, attachment: v.get("attachment")?.as_i64()? })
    }
}

/// The state of an exact comparison between windows.
pub struct Reconcile {
    held: Vec<(u64, u64)>,
    at: usize,
    lo: i64,
    pub done: bool,
}

pub struct Engine {
    pub fields: Fields,
    pub reader: IndexReader,
    writer: IndexWriter<TantivyDocument>,
    conn: Connection,
    pub marks: Watermarks,
    dirty: bool,
    pub commits: u64,
}

fn to_doc(f: &Fields, m: &Indexed) -> TantivyDocument {
    let mut d = TantivyDocument::default();
    d.add_text(f.id, &m.id);
    d.add_u64(f.rowid, m.rowid as u64);
    d.add_u64(f.stamp, m.stamp);
    d.add_text(f.subject, &m.subject);
    d.add_text(f.from, &m.from);
    d.add_text(f.to, &m.to);
    d.add_text(f.body, &m.body);
    d.add_text(f.attachments, &m.attachments);
    d
}

impl Engine {
    /// The writer with the threads and memory the caller chose (`TextIndex::open` passes WRITER_THREADS and WRITER_HEAP_BYTES;
    /// sync-full-r1's background indexer passes its own).
    pub fn new_with(index: &Index, fields: Fields, reader: IndexReader, conn: Connection, marks: Watermarks, threads: usize, heap_bytes: usize) -> Result<Engine, Error> {
        let writer = index.writer_with_num_threads::<TantivyDocument>(threads, heap_bytes)?;
        Ok(Engine { fields, reader, writer, conn, marks, dirty: false, commits: 0 })
    }

    /// sync-full-r1: index these messages again from the table (the ones still stored), replacing their documents.
    pub fn reindex_ids(&mut self, ids: &[String]) -> Result<(), Error> {
        for chunk in ids.chunks(BATCH_ROWS) {
            for m in source::fetch_ids(&self.conn, chunk)? {
                self.add(&m, true)?;
                self.dirty = true;
            }
        }
        Ok(())
    }

    fn add(&mut self, m: &Indexed, replace: bool) -> Result<(), Error> {
        if replace {
            self.writer.delete_term(Term::from_field_text(self.fields.id, &m.id));
        }
        self.writer.add_document(to_doc(&self.fields, m))?;
        Ok(())
    }

    /// Make the committed state visible and durable. A no-op when nothing changed.
    pub fn commit(&mut self) -> Result<(), Error> {
        if !self.dirty {
            return Ok(());
        }
        let mut prepared = self.writer.prepare_commit()?;
        prepared.set_payload(&self.marks.to_payload());
        prepared.commit()?;
        self.reader.reload()?;
        self.dirty = false;
        self.commits += 1;
        Ok(())
    }

    /// Build a fresh index from the whole message table.
    pub fn rebuild(&mut self) -> Result<(), Error> {
        self.marks = Watermarks::default();
        self.catch_up(false)?;
        // An empty table still needs a payload on disk, or the next open would rebuild again.
        self.dirty = true;
        self.commit()
    }

    /// Index the messages after the watermark, and re-index the owners of attachments added since.
    /// `replace` is false only on a fresh index, where no document can exist yet.
    pub fn catch_up(&mut self, replace: bool) -> Result<bool, Error> {
        let top = source::max_message_rowid(&self.conn)?;
        if top < self.marks.message {
            // Rows were removed from the top; a later insert may reuse their rowids.
            self.marks.message = top;
            self.dirty = true;
        }
        let mut changed = false;
        loop {
            let batch = source::fetch_after(&self.conn, self.marks.message, BATCH_ROWS)?;
            if batch.is_empty() {
                break;
            }
            for m in &batch {
                self.add(m, replace)?;
            }
            self.marks.message = batch.last().map(|m| m.rowid).unwrap_or(self.marks.message);
            changed = true;
        }
        let att_top = source::max_attachment_rowid(&self.conn)?;
        if att_top < self.marks.attachment {
            self.marks.attachment = att_top;
            self.dirty = true;
        } else if att_top > self.marks.attachment {
            let owners = source::attachment_owners_after(&self.conn, self.marks.attachment)?;
            for chunk in owners.chunks(BATCH_ROWS) {
                for m in source::fetch_ids(&self.conn, chunk)? {
                    self.add(&m, true)?;
                }
            }
            self.marks.attachment = att_top;
            changed = true;
        }
        self.dirty |= changed;
        Ok(changed)
    }

    /// What the committed index holds: (rowid, stamp) of every live document, sorted by rowid.
    fn indexed_stamps(&self) -> Result<Vec<(u64, u64)>, Error> {
        let searcher = self.reader.searcher();
        let mut all = Vec::with_capacity(searcher.num_docs() as usize);
        for seg in searcher.segment_readers() {
            let rowids = seg.fast_fields().u64("rowid")?;
            let stamps = seg.fast_fields().u64("stamp")?;
            let alive = seg.alive_bitset();
            for doc in 0..seg.max_doc() {
                if alive.is_some_and(|a| a.is_deleted(doc)) {
                    continue;
                }
                if let (Some(r), Some(s)) = (rowids.first(doc), stamps.first(doc)) {
                    all.push((r, s));
                }
            }
        }
        all.sort_unstable();
        Ok(all)
    }

    /// Start an exact comparison of the table with the committed index for rows after `after_rowid` (0 = everything).
    /// Drive it with `reconcile_window` until it says it is done; new mail can be served between windows.
    pub fn reconcile_start(&mut self, after_rowid: i64) -> Result<Reconcile, Error> {
        self.commit()?;
        let held = self.indexed_stamps()?;
        let from = held.partition_point(|(r, _)| (*r as i64) <= after_rowid);
        Ok(Reconcile { held, at: from, lo: after_rowid, done: false })
    }

    /// Compare one window of rowids (`RECONCILE_WINDOW_ROWIDS` wide): index what is missing or changed, drop what is gone.
    /// The last window has no upper end, so rows left in the index above the table's top are dropped too.
    pub fn reconcile_window(&mut self, r: &mut Reconcile) -> Result<(), Error> {
        let top = source::max_message_rowid(&self.conn)?;
        let hi = r.lo.saturating_add(RECONCILE_WINDOW_ROWIDS);
        let last = hi >= top;
        let upper = if last { i64::MAX } else { hi };
        let mut held: HashMap<u64, u64> = HashMap::new();
        while r.at < r.held.len() && (r.held[r.at].0 as i64) <= upper {
            held.insert(r.held[r.at].0, r.held[r.at].1);
            r.at += 1;
        }
        let mut redo: Vec<i64> = Vec::new();
        for (rowid, stamp) in source::stamps_between(&self.conn, r.lo, upper)? {
            match held.remove(&(rowid as u64)) {
                Some(s) if s == stamp => {}
                _ => redo.push(rowid),
            }
        }
        for rowid in held.keys() {
            self.writer.delete_term(Term::from_field_u64(self.fields.rowid, *rowid));
        }
        for chunk in redo.chunks(BATCH_ROWS) {
            for m in source::fetch_rowids(&self.conn, chunk)? {
                self.add(&m, true)?;
            }
        }
        self.dirty |= !held.is_empty() || !redo.is_empty();
        self.commit()?;
        r.lo = hi;
        r.done = last;
        Ok(())
    }

    /// The whole comparison in one call (tests and the first open use it).
    pub fn reconcile(&mut self, after_rowid: i64) -> Result<(), Error> {
        let mut r = self.reconcile_start(after_rowid)?;
        while !r.done {
            self.reconcile_window(&mut r)?;
        }
        Ok(())
    }

    /// The re-check after an update signal: the newest rows only.
    pub fn recheck_tail(&mut self) -> Result<(), Error> {
        let from = (self.marks.message - RECHECK_TAIL_ROWS).max(0);
        self.reconcile(from)
    }

    pub fn documents(&self) -> u64 {
        self.reader.searcher().num_docs()
    }
}

/// The stored string of a field, if any.
pub fn text_of(doc: &TantivyDocument, field: Field) -> Option<String> {
    doc.get_first(field).and_then(|v| v.as_str()).map(String::from)
}
