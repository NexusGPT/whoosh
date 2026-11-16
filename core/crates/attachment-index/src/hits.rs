//! Turns what the indexes return into rows that name the attachment and its message (`SearchHit.matched_in`, delta 8).
//! Pure functions over a catalog of the candidate messages' attachments; the search crate fuses the lists by rank (RRF_K), so a
//! word or name hit carries 1.0 (every meaningful query word matched) and only a vector hit carries its cosine.
use crate::entry::{IndexState, NOT_DOWNLOADED_WORDS, UNREADABLE_WORDS};
use contracts::{MatchedIn, SearchHit};

/// One attachment of a candidate message, as the store holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogRow {
    pub message_id: String,
    pub attachment_id: String,
    pub file_name: String,
    pub state: IndexState,
    /// `AttachmentIndexEntry::word_text` as stored (empty when not indexed)
    pub text: String,
}

/// A vector-index answer: the key a vector was filed under ("<message_id>/<attachment_id>" plus a part suffix) and its cosine.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorMatch {
    pub key: String,
    pub score: f32,
}

const FILLER: &[&str] = &["the", "and", "for", "with", "from", "that", "this", "what", "which", "any", "all", "about", "in", "of", "to", "on", "at", "a", "an", "is", "it"];

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

fn query_words(q: &str) -> Vec<String> {
    words(q).into_iter().filter(|w| !FILLER.contains(&w.as_str())).collect()
}

fn snippet_of(text: &str, first_word: &str) -> String {
    let lower = text.to_lowercase();
    let before = lower.find(first_word).map_or(0, |at| lower[..at].chars().count());
    let start = before.saturating_sub(40);
    let tail: String = text.chars().skip(start).take(130).collect();
    // begin on a whole word, not in the middle of one
    let tail = if start > 0 { tail.split_once(char::is_whitespace).map_or(tail.as_str(), |(_, rest)| rest) } else { tail.as_str() };
    tail.replace('\n', " ")
}

fn hit(row: &CatalogRow, score: f64, snippet: String, matched_in: MatchedIn) -> SearchHit {
    SearchHit { message_id: row.message_id.clone(), score, snippet, matched_in, attachment_name: Some(row.file_name.clone()), attachment_note: None }
}

/// Meaning hits on an attachment's vector: one row per attachment, its best vector's score. Parts ("#f3", "#t0") fold to the attachment.
pub fn hits_for_vectors(matches: &[VectorMatch], catalog: &[CatalogRow]) -> Vec<SearchHit> {
    let mut best: Vec<(&CatalogRow, f32)> = Vec::new();
    for m in matches {
        let base = m.key.split('#').next().unwrap_or("");
        let Some(row) = catalog.iter().find(|r| format!("{}/{}", r.message_id, r.attachment_id) == base) else { continue };
        match best.iter_mut().find(|(r, _)| r.attachment_id == row.attachment_id) {
            Some(slot) if slot.1 >= m.score => {}
            Some(slot) => slot.1 = m.score,
            None => best.push((row, m.score)),
        }
    }
    best.sort_by(|a, b| b.1.total_cmp(&a.1));
    best.into_iter().map(|(r, s)| hit(r, s as f64, if r.text.is_empty() { r.file_name.clone() } else { snippet_of(&r.text, "") }, MatchedIn::AttachmentContent)).collect()
}

/// Word hits inside an attachment's text: every meaningful query word is in it.
pub fn hits_for_text(query: &str, catalog: &[CatalogRow]) -> Vec<SearchHit> {
    let q = query_words(query);
    let Some(first) = q.first() else { return Vec::new() };
    catalog
        .iter()
        .filter(|r| r.state == IndexState::Indexed && !r.text.is_empty())
        .filter(|r| {
            let have = words(&r.text);
            q.iter().all(|w| have.contains(w))
        })
        .map(|r| hit(r, 1.0, snippet_of(&r.text, first), MatchedIn::AttachmentContent))
        .collect()
}

/// Name hits: every meaningful query word begins a word of the file name. A file that could not be searched inside says so in the
/// row's words (J2.8): not downloaded yet, or a kind that cannot be read.
pub fn hits_for_name(query: &str, catalog: &[CatalogRow]) -> Vec<SearchHit> {
    let q = query_words(query);
    if q.is_empty() {
        return Vec::new();
    }
    catalog
        .iter()
        .filter(|r| {
            let name = words(&r.file_name);
            q.iter().all(|w| name.iter().any(|n| n.starts_with(w.as_str())))
        })
        .map(|r| {
            let said = match &r.state {
                IndexState::NotDownloaded => NOT_DOWNLOADED_WORDS.to_string(),
                IndexState::Unreadable => UNREADABLE_WORDS.to_string(),
                IndexState::Failed { reason_words } => reason_words.clone(),
                IndexState::Indexed => r.file_name.clone(),
            };
            hit(r, 1.0, said, MatchedIn::AttachmentName)
        })
        .collect()
}

/// One row per attachment: a content hit (text or meaning) beats a name hit for the same attachment; the best score stays.
pub fn merge_attachment_hits(lists: Vec<Vec<SearchHit>>) -> Vec<SearchHit> {
    let mut out: Vec<SearchHit> = Vec::new();
    for h in lists.into_iter().flatten() {
        match out.iter_mut().find(|o| o.message_id == h.message_id && o.attachment_name == h.attachment_name) {
            Some(o) => {
                let content = |m: &MatchedIn| matches!(m, MatchedIn::AttachmentContent);
                if (content(&h.matched_in) && !content(&o.matched_in)) || (content(&h.matched_in) == content(&o.matched_in) && h.score > o.score) {
                    *o = h;
                }
            }
            None => out.push(h),
        }
    }
    let content = |h: &SearchHit| matches!(h.matched_in, MatchedIn::AttachmentContent);
    out.sort_by(|a, b| content(b).cmp(&content(a)).then(b.score.total_cmp(&a.score)));
    out
}
