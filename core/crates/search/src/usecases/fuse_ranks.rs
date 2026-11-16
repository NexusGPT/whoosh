//! Reciprocal-rank fusion: a message at 1-based rank r in a list earns 1 / (k + r) from that list, and its
//! score is the sum over the lists it appears in. k is `contracts::constants::RRF_K` (docs/CONSTANTS.md).
//! Equal scores keep the better text rank first, then the better meaning rank, then the id, so the order
//! never depends on a hash.
use contracts::constants::RRF_K;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Fused {
    pub message_id: String,
    pub score: f64,
    /// 1-based rank in the text list, if it is there.
    pub text_rank: Option<usize>,
    /// 1-based rank in the meaning list, if it is there.
    pub meaning_rank: Option<usize>,
}

fn share(rank: usize) -> f64 {
    1.0 / (RRF_K as f64 + rank as f64)
}

/// Fuse two ranked id lists, best first. A repeated id counts at its first (best) rank only.
pub fn fuse_ranks(text: &[String], meaning: &[String]) -> Vec<Fused> {
    let mut by_id: HashMap<&str, Fused> = HashMap::new();
    for (list, is_text) in [(text, true), (meaning, false)] {
        for (i, id) in list.iter().enumerate() {
            let entry = by_id.entry(id.as_str()).or_insert_with(|| Fused { message_id: id.clone(), score: 0.0, text_rank: None, meaning_rank: None });
            let slot = if is_text { &mut entry.text_rank } else { &mut entry.meaning_rank };
            if slot.is_none() {
                *slot = Some(i + 1);
                entry.score += share(i + 1);
            }
        }
    }
    let mut out: Vec<Fused> = by_id.into_values().collect();
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.text_rank.unwrap_or(usize::MAX).cmp(&b.text_rank.unwrap_or(usize::MAX)))
            .then(a.meaning_rank.unwrap_or(usize::MAX).cmp(&b.meaning_rank.unwrap_or(usize::MAX)))
            .then(a.message_id.cmp(&b.message_id))
    });
    out
}

/// Adds a third ranked list (attachment hits, contracts delta 10) to fused results: each id earns 1 / (k + r) from it too,
/// and an id only it holds joins with that share. The order is re-sorted by score, ties by the text then meaning rank.
pub fn add_ranked_list(fused: Vec<Fused>, list: &[String]) -> Vec<Fused> {
    let mut out = fused;
    let mut seen = std::collections::HashSet::new();
    for (i, id) in list.iter().enumerate() {
        if !seen.insert(id.as_str()) {
            continue;
        }
        match out.iter_mut().find(|f| &f.message_id == id) {
            Some(f) => f.score += share(i + 1),
            None => out.push(Fused { message_id: id.clone(), score: share(i + 1), text_rank: None, meaning_rank: None }),
        }
    }
    out.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.text_rank.unwrap_or(usize::MAX).cmp(&b.text_rank.unwrap_or(usize::MAX)))
            .then(a.meaning_rank.unwrap_or(usize::MAX).cmp(&b.meaning_rank.unwrap_or(usize::MAX)))
            .then(a.message_id.cmp(&b.message_id))
    });
    out
}
