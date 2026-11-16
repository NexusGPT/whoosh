//! Step 4: rank, drop what says the same thing twice, keep the `keep_n` best. The rank is the index's score (best first), then the number of
//! questions that found the excerpt, then the newer date, then the chunk id, so equal answers from the ports always give the same order.
use crate::domain::text::normalised;
use crate::domain::{DropReason, Dropped};
use crate::usecases::gather::Candidate;
use std::cmp::Ordering;

/// The rank order: better first.
pub fn rank_order(a: &Candidate, b: &Candidate) -> Ordering {
    b.found
        .score
        .total_cmp(&a.found.score)
        .then(b.found_by.len().cmp(&a.found_by.len()))
        .then(match (a.found.date_ms, b.found.date_ms) {
            (Some(x), Some(y)) => y.cmp(&x),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then(a.found.chunk_id.cmp(&b.found.chunk_id))
}

/// The candidates in rank order.
pub fn rank(mut c: Vec<Candidate>) -> Vec<Candidate> {
    c.sort_by(rank_order);
    c
}

/// Drops an excerpt whose (sized) text a better-ranked one already says, compared without case, spacing or end punctuation; the better one
/// inherits the questions that found the dropped one. `c` must be ranked.
pub fn dedupe(c: Vec<Candidate>, dropped: &mut Vec<Dropped>) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for cand in c {
        let key = normalised(&cand.found.text);
        match out.iter_mut().find(|o| normalised(&o.found.text) == key) {
            Some(winner) => {
                for q in &cand.found_by {
                    if !winner.found_by.contains(q) {
                        winner.found_by.push(*q);
                    }
                }
                dropped.push(Dropped { chunk_id: cand.found.chunk_id, reason: DropReason::Duplicate { of: winner.found.chunk_id.clone() } });
            }
            None => out.push(cand),
        }
    }
    out
}

/// The first `n` of a ranked list; the rest dropped as beyond the keep.
pub fn keep_first(mut c: Vec<Candidate>, n: usize, dropped: &mut Vec<Dropped>) -> Vec<Candidate> {
    if c.len() > n {
        for rest in c.split_off(n) {
            dropped.push(Dropped { chunk_id: rest.found.chunk_id, reason: DropReason::BeyondKeep });
        }
    }
    c
}
