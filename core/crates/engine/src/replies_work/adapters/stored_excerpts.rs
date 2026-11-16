//! attach-engine: the excerpts of folders of this Mac and of calendar events, kept in the LOCAL DATABASE (`source_chunk`, migration 0016; each item's stamp in
//! `source_item`, 0020), so a relaunch reads them without walking again ("it should store locally"). The same port and the same methods as the in-memory
//! adapter (`memory_excerpts.rs`, which stays for the tests that use it): `ExcerptSink` of reply-sources writes here, the retrieval's index reads here by words.
//!
//! The row of the contract (delta 23, `SourceChunk`) holds the source key, the item key, the number, the date, the title and the text. What it does not hold
//! is derived on the way out: a file's "folder" (the owner's reading of where it lives) is its folder path and its item path, an event's is the calendar's name
//! (read from `reply_calendar`), and an excerpt's id stays what the walks make it (`reply_sources::excerpt_id`: source key, item key, number, joined by U+001F; stable across syncs and restarts; the draft's `excerpt_ids` and the existing engine tests read that form). The contract's `source_chunk_excerpt_id` (`s<row id>`) is the other spelling: named in the lane note.
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use contracts::{reply_source_from_key, reply_source_key, CoreError, ReplySourceRef};
use reply_retrieval::FoundExcerpt;
use reply_sources::{excerpt_id, ExcerptSink, ItemStamp, SourceExcerpt};
use store::Store;

use super::memory_excerpts::lowered_words;

/// A stored excerpt as the store hands it over: (id, source key, item key, chunk number, date, title, text).
type StoredRow = (i64, String, String, u32, Option<i64>, String, String);

pub struct StoredExcerpts { store: Arc<Store>, rows_read: AtomicUsize }

fn failed(what: &str, e: store::StoreError) -> CoreError { CoreError::Unexpected { reason_words: format!("{what}: {e}") } }
fn key_of(source: &ReplySourceRef) -> String { reply_source_key(source.clone()) }

impl StoredExcerpts {
    pub fn new(store: Arc<Store>) -> StoredExcerpts { StoredExcerpts { store, rows_read: AtomicUsize::new(0) } }

    /// How many stored rows the last `search` or `search_scan` read from the database (the index answers with at most `limit`; the scan reads every row).
    pub fn rows_read(&self) -> usize { self.rows_read.load(Ordering::SeqCst) }

    /// Every excerpt of a source leaves (a folder removed from the Sources pane).
    pub fn remove_source(&self, source: &ReplySourceRef) { let _ = self.store.source_remove_source(&key_of(source)); }

    /// The sources that hold excerpts or stamps.
    pub fn sources(&self) -> Vec<ReplySourceRef> {
        self.store.source_keys().unwrap_or_default().into_iter().filter_map(reply_source_from_key).collect()
    }

    /// How many excerpts a source holds.
    pub fn excerpt_count(&self, source: &ReplySourceRef) -> usize { self.store.source_chunk_count(&key_of(source)).unwrap_or(0) as usize }

    /// A stored row as the walks cut it (what the row does not hold is derived, see the header).
    fn excerpt_of(row: StoredRow, names: &HashMap<String, String>) -> Option<SourceExcerpt> {
        let (_id, source_key, item_key, chunk_no, date_ms, title, text) = row;
        let source = reply_source_from_key(source_key)?;
        let folder = match &source {
            ReplySourceRef::LocalFolder { path } => format!("{}/{item_key}", path.trim_end_matches('/')),
            ReplySourceRef::Calendar { calendar_id } => names.get(calendar_id).cloned().unwrap_or_else(|| calendar_id.clone()),
            ReplySourceRef::MailFolder { .. } => return None,
        };
        Some(SourceExcerpt { id: excerpt_id(&source, &item_key, chunk_no), source, item_key, ordinal: chunk_no, title, folder, date_ms, text })
    }

    fn calendar_names(&self) -> HashMap<String, String> {
        self.store.reply_calendars().map(|all| all.into_iter().map(|c| (c.calendar_id, c.name)).collect()).unwrap_or_default()
    }

    /// The excerpts of `sources`, as the walks cut them, in stored order: EVERY row (the reference scan and `dated_texts`).
    fn excerpts_of(&self, sources: &[ReplySourceRef]) -> Vec<SourceExcerpt> {
        let keys: Vec<String> = sources.iter().map(key_of).collect();
        let rows = match self.store.source_chunks(keys) { Ok(r) => r, Err(e) => { eprintln!("replies: the stored excerpts could not be read: {e}"); return vec![] } };
        self.rows_read.store(rows.len(), Ordering::SeqCst);
        let names = self.calendar_names();
        rows.into_iter().filter_map(|row| Self::excerpt_of(row, &names)).collect()
    }

    /// Every excerpt of `sources` as (date, text), oldest first (the calendar's events for `{{calendar}}`).
    pub fn dated_texts(&self, sources: &[ReplySourceRef]) -> Vec<(Option<i64>, String)> {
        let mut out: Vec<(Option<i64>, String)> = self.excerpts_of(sources).into_iter().map(|x| (x.date_ms, x.text)).collect();
        out.sort();
        out
    }

    /// The excerpts of `sources` that share words with `query`, best first (the share of the query's words the excerpt holds), at most `limit`. RANKED BY THE FULL-TEXT
    /// INDEX over `source_chunk` (migration 0021): the database scores each excerpt by the query's words it holds, orders by score and the excerpt's id, and hands over only
    /// the `limit` best rows, so the cost follows the rows that match, not the rows stored. The same set and order as `search_scan` (a test compares them) for words without diacritics; the index folds Latin diacritics ("cafe" finds "café"), so on accented text it finds a superset the scan would not.
    pub fn search(&self, query: &str, sources: &[ReplySourceRef], limit: usize) -> Vec<FoundExcerpt> {
        let wanted = lowered_words(query);
        if wanted.is_empty() || limit == 0 || sources.is_empty() { return vec![]; }
        let total = wanted.len() as f32;
        let rows = match self.store.source_chunks_ranked(sources.iter().map(key_of).collect(), wanted, limit) { Ok(r) => r, Err(e) => { eprintln!("replies: the stored excerpts could not be searched: {e}"); return vec![] } };
        self.rows_read.store(rows.len(), Ordering::SeqCst);
        let names = self.calendar_names();
        rows.into_iter().filter_map(|(n, row)| {
            let x = Self::excerpt_of(row, &names)?.to_reply_excerpt();
            Some(FoundExcerpt { chunk_id: x.chunk_id, source: x.source, date_ms: x.date_ms, sender: x.sender, folder: x.folder, subject: x.subject, text: x.text, score: n as f32 / total })
        }).collect()
    }

    /// The REFERENCE: the same search by reading every stored row of the sources and ranking in memory (what this adapter did before the index; the in-memory
    /// adapter's own ranking). Kept so a test can compare the two.
    pub fn search_scan(&self, query: &str, sources: &[ReplySourceRef], limit: usize) -> Vec<FoundExcerpt> {
        let wanted = lowered_words(query);
        if wanted.is_empty() || limit == 0 || sources.is_empty() { return vec![]; }
        let mut hits: Vec<FoundExcerpt> = vec![];
        for e in self.excerpts_of(sources) {
            let have = lowered_words(&format!("{} {}", e.title, e.text));
            let n = wanted.iter().filter(|w| have.contains(w)).count();
            if n == 0 { continue; }
            let x = e.to_reply_excerpt();
            hits.push(FoundExcerpt { chunk_id: x.chunk_id, source: x.source, date_ms: x.date_ms, sender: x.sender, folder: x.folder, subject: x.subject, text: x.text, score: n as f32 / wanted.len() as f32 });
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.chunk_id.cmp(&b.chunk_id)));
        hits.truncate(limit);
        hits
    }
}

impl ExcerptSink for StoredExcerpts {
    fn stamps(&self, source: &ReplySourceRef) -> Result<Vec<ItemStamp>, CoreError> {
        let rows = self.store.source_item_stamps(&key_of(source)).map_err(|e| failed("the stored items could not be read", e))?;
        Ok(rows.into_iter().map(|(item_key, version, excerpt_count)| ItemStamp { item_key, version, excerpt_count }).collect())
    }
    fn put_item(&self, source: &ReplySourceRef, stamp: &ItemStamp, excerpts: &[SourceExcerpt]) -> Result<(), CoreError> {
        let chunks = excerpts.iter().map(|x| (x.ordinal, x.date_ms, x.title.clone(), x.text.clone())).collect();
        self.store.source_put_item(&key_of(source), &stamp.item_key, &stamp.version, stamp.excerpt_count, chunks).map_err(|e| failed("the item could not be stored", e))
    }
    fn remove_item(&self, source: &ReplySourceRef, item_key: &str) -> Result<(), CoreError> {
        self.store.source_remove_item(&key_of(source), item_key).map_err(|e| failed("the item could not be removed", e))
    }
}
