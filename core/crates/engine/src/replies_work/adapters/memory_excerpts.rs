//! The excerpts of folders of this Mac (and, once the shell hands calendars over, of events), kept IN MEMORY for this round: the store has no table for
//! them yet (migration 0013 holds none; lane replies-contract adds the source chunks in delta 21). `ExcerptSink` of reply-sources writes here; the
//! retrieval's index reads here by words. A relaunch walks the enabled folders again (each file read once per launch).
use std::collections::HashMap;
use std::sync::Mutex;

use contracts::{CoreError, ReplySourceRef};
use reply_retrieval::FoundExcerpt;
use reply_sources::{ExcerptSink, ItemStamp, SourceExcerpt};

#[derive(Default)]
pub struct MemoryExcerpts { items: Mutex<HashMap<ReplySourceRef, HashMap<String, (ItemStamp, Vec<SourceExcerpt>)>>> }

pub(crate) fn lowered_words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() > 2).map(str::to_lowercase).collect()
}

impl MemoryExcerpts {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<ReplySourceRef, HashMap<String, (ItemStamp, Vec<SourceExcerpt>)>>> { self.items.lock().unwrap_or_else(|e| e.into_inner()) }

    /// Every excerpt of a source leaves (a folder removed from the Sources pane).
    pub fn remove_source(&self, source: &ReplySourceRef) { self.lock().remove(source); }

    /// The sources that hold excerpts.
    pub fn sources(&self) -> Vec<ReplySourceRef> { self.lock().keys().cloned().collect() }

    /// How many excerpts a source holds.
    pub fn excerpt_count(&self, source: &ReplySourceRef) -> usize { self.lock().get(source).map(|m| m.values().map(|(_, e)| e.len()).sum()).unwrap_or(0) }

    /// Every excerpt of `sources` as (date, text), oldest first (the calendar's events for `{{calendar}}`).
    pub fn dated_texts(&self, sources: &[ReplySourceRef]) -> Vec<(Option<i64>, String)> {
        let g = self.lock();
        let mut out: Vec<(Option<i64>, String)> = sources.iter().filter_map(|s| g.get(s)).flat_map(|items| items.values().flat_map(|(_, e)| e.iter().map(|x| (x.date_ms, x.text.clone())))).collect();
        out.sort();
        out
    }

    /// The excerpts of `sources` that share words with `query`, best first (the share of the query's words the excerpt holds), at most `limit`.
    pub fn search(&self, query: &str, sources: &[ReplySourceRef], limit: usize) -> Vec<FoundExcerpt> {
        let wanted = lowered_words(query);
        if wanted.is_empty() || limit == 0 { return vec![]; }
        let g = self.lock();
        let mut hits: Vec<FoundExcerpt> = vec![];
        for src in sources {
            let Some(items) = g.get(src) else { continue };
            for (_, excerpts) in items.values() {
                for e in excerpts {
                    let have = lowered_words(&format!("{} {}", e.title, e.text));
                    let n = wanted.iter().filter(|w| have.contains(w)).count();
                    if n == 0 { continue; }
                    let x = e.to_reply_excerpt();
                    hits.push(FoundExcerpt { chunk_id: x.chunk_id, source: x.source, date_ms: x.date_ms, sender: x.sender, folder: x.folder, subject: x.subject, text: x.text, score: n as f32 / wanted.len() as f32 });
                }
            }
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.chunk_id.cmp(&b.chunk_id)));
        hits.truncate(limit);
        hits
    }
}

impl ExcerptSink for MemoryExcerpts {
    fn stamps(&self, source: &ReplySourceRef) -> Result<Vec<ItemStamp>, CoreError> {
        Ok(self.lock().get(source).map(|m| m.values().map(|(s, _)| s.clone()).collect()).unwrap_or_default())
    }
    fn put_item(&self, source: &ReplySourceRef, stamp: &ItemStamp, excerpts: &[SourceExcerpt]) -> Result<(), CoreError> {
        self.lock().entry(source.clone()).or_default().insert(stamp.item_key.clone(), (stamp.clone(), excerpts.to_vec()));
        Ok(())
    }
    fn remove_item(&self, source: &ReplySourceRef, item_key: &str) -> Result<(), CoreError> {
        if let Some(m) = self.lock().get_mut(source) { m.remove(item_key); }
        Ok(())
    }
}
