//! `ExcerptIndex` of reply-retrieval: the mail by the word index and the meaning index the background indexer builds (sync-full-r1: `search_words`,
//! `search_meaning`), the folders of this Mac (and events) by the in-memory excerpts. Only the sources the query names are answered: a hit outside them
//! is dropped here (retrieval drops it again and counts it as a leak). Word and meaning scores are not on one scale, so each list is divided by its
//! best score before the lists are merged.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use contracts::{chunk_excerpt_id, CoreError, ReplySourceRef};
use reply_retrieval::{ExcerptIndex, ExcerptQuery, FoundExcerpt};
use store::Store;

use super::stored_excerpts::StoredExcerpts;
use crate::bodies::Bodies;

/// The excerpt id of a mail excerpt: its chunk's own id in decimal words (`chunk_excerpt_id`, delta 21). A message the indexer has not cut into chunks
/// yet (a word hit just after its body landed) is named by its message, `mail` U+001F id, until it is.
pub(crate) fn unchunked_id(message_id: &str) -> String { format!("mail\u{1f}{message_id}") }

/// The excerpt id of a mail chunk (word hits and meaning hits name it the same way).
fn chunk_name(chunk_id: i64) -> String { chunk_excerpt_id(chunk_id) }

/// Of a message's body chunks, the one sharing the most words with the query (the first on a tie), as (id, text).
fn best_chunk(chunks: Vec<(i64, String, bool)>, query: &str) -> Option<(i64, String)> {
    let wanted: Vec<String> = query.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() > 2).map(str::to_lowercase).collect();
    let score = |t: &str| { let t = t.to_lowercase(); wanted.iter().filter(|w| t.contains(w.as_str())).count() };
    chunks.into_iter().map(|(id, text, _)| (id, text)).enumerate().max_by_key(|(i, (_, t))| (score(t), std::cmp::Reverse(*i))).map(|(_, c)| c)
}

pub(crate) struct Index<'a> { pub store: &'a Store, pub bodies: Option<Arc<Bodies>>, pub memory: &'a StoredExcerpts }

fn scaled(mut hits: Vec<FoundExcerpt>) -> Vec<FoundExcerpt> {
    let best = hits.iter().map(|h| h.score).fold(0f32, f32::max);
    if best > 0.0 { for h in &mut hits { h.score /= best; } }
    hits
}

/// What an excerpt shows of its message, from the message row as it is now.
struct Row { message_id: String, account_id: String, folder: String, subject: String, sender: String, received_at_ms: i64, text: String }

/// The message rows of these ids, read in ONE batch on the background work's own connection (`rowids_of` + `index_rows`; never the reader the views
/// wait on, never a whole-table read), by id.
fn message_rows(store: &Store, ids: &[String]) -> HashMap<String, Row> {
    let read = || -> Result<Vec<Row>, store::StoreError> {
        let r = store.background_reads()?;
        let rowids = r.rowids_of(ids)?;
        Ok(r.index_rows(&rowids)?.into_iter().map(|m| Row {
            sender: if m.sender_name.trim().is_empty() { m.from_address.clone() } else { m.sender_name.clone() },
            text: m.body_text.clone().or_else(|| m.body_html.as_deref().map(parse::safe_html_to_text)).unwrap_or_else(|| m.preview.clone()),
            message_id: m.message_id, account_id: m.account_id, folder: m.folder, subject: m.subject, received_at_ms: m.received_at_ms,
        }).collect())
    };
    match read() {
        Ok(rows) => rows.into_iter().map(|r| (r.message_id.clone(), r)).collect(),
        Err(e) => { eprintln!("replies: the found messages could not be read: {e}"); HashMap::new() }
    }
}

impl Index<'_> {
    /// Word hits and meaning hits of the mail. The date, sender, folder and subject of every excerpt are its MESSAGE row's as it is now (a moved message
    /// is in its new folder, so the account wall is read on the row), read in one batch through the hits' message ids; the text is the chunk's.
    fn mail(&self, query: &ExcerptQuery, folders: &HashSet<(String, String)>) -> Vec<FoundExcerpt> {
        let Some(bodies) = &self.bodies else { return vec![] };
        let wide = (query.limit as usize).saturating_mul(4).max(1);
        let words_found = bodies.search_words(&query.text, wide);
        let meaning_found = match bodies.search_meaning(&query.text, wide) {
            Ok(hits) => hits,
            Err(e) => { eprintln!("replies: the meaning search failed, the word search answers alone: {e}"); vec![] }
        };
        let mut ids: Vec<String> = words_found.iter().map(|h| h.message_id.clone()).chain(meaning_found.iter().map(|h| h.chunk.message_id.clone())).collect();
        ids.sort();
        ids.dedup();
        let rows = message_rows(self.store, &ids);
        let in_scope = |id: &str| rows.get(id).filter(|r| folders.contains(&(r.account_id.clone(), r.folder.clone())));
        let excerpt = |r: &Row, chunk_id: String, text: String, score: f32| FoundExcerpt {
            chunk_id, source: ReplySourceRef::MailFolder { account_id: r.account_id.clone(), folder: r.folder.clone() }, date_ms: Some(r.received_at_ms),
            sender: Some(r.sender.clone()), folder: Some(r.folder.clone()), subject: Some(r.subject.clone()), text, score,
        };
        let reads = self.store.background_reads().ok();
        let mut words = vec![];
        for h in words_found {
            let Some(r) = in_scope(&h.message_id) else { continue };
            let chunk = reads.as_ref().and_then(|b| b.body_chunks(&r.message_id).ok()).and_then(|c| best_chunk(c, &query.text));
            let (chunk_id, text) = match chunk {
                Some((id, text)) => (chunk_name(id), text),
                None => (unchunked_id(&r.message_id), r.text.clone()),
            };
            words.push(excerpt(r, chunk_id, text, h.score as f32));
        }
        let meaning: Vec<FoundExcerpt> = meaning_found.into_iter().filter_map(|h| {
            let r = in_scope(&h.chunk.message_id)?;
            Some(excerpt(r, chunk_name(h.chunk.id), h.chunk.text, h.score))
        }).collect();
        let mut all = scaled(words);
        all.extend(scaled(meaning));
        all
    }
}

impl ExcerptIndex for Index<'_> {
    fn execute(&self, query: &ExcerptQuery) -> Result<Vec<FoundExcerpt>, CoreError> {
        let folders: HashSet<(String, String)> = query.sources.iter().filter_map(|s| match s {
            ReplySourceRef::MailFolder { account_id, folder } => Some((account_id.clone(), folder.clone())), _ => None }).collect();
        let others: Vec<ReplySourceRef> = query.sources.iter().filter(|s| !matches!(s, ReplySourceRef::MailFolder { .. })).cloned().collect();
        let mut all = if folders.is_empty() { vec![] } else { self.mail(query, &folders) };
        all.extend(scaled(self.memory.search(&query.text, &others, query.limit as usize)));
        all.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.chunk_id.cmp(&b.chunk_id)));
        let mut seen = HashSet::new();
        all.retain(|h| seen.insert(h.chunk_id.clone()));
        all.truncate(query.limit as usize);
        Ok(all)
    }
}
