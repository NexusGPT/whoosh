//! Step 5: two kept excerpts that may contradict each other. Without a model call (a reply has at most three, AI-REPLIES-MODEL §2), a clash is
//! read from the facts: the same conversation (the same subject once Re:/Fwd: are gone), found by a same question (the same point), written on
//! different days or times (so one is newer). Their texts differ already (step 4 dropped what says the same thing). An undated excerpt never
//! clashes: no rule can say which is newer.
//!   KeepBothDated (the default the owner confirmed): both stay; each label carries its date and the number of the other (step 7).
//!   KeepNewest / KeepOldest: an excerpt is dropped when a clashing one is newer (older); the trace names the one kept instead.
use crate::domain::text::bare_subject;
use crate::domain::{DropReason, Dropped};
use crate::usecases::gather::Candidate;
use contracts::ClashRule;

/// Whether two excerpts clash.
pub fn clashing(a: &Candidate, b: &Candidate) -> bool {
    let (Some(da), Some(db)) = (a.found.date_ms, b.found.date_ms) else { return false };
    let sa = bare_subject(a.found.subject.as_deref().unwrap_or(""));
    let same_subject = !sa.is_empty() && sa == bare_subject(b.found.subject.as_deref().unwrap_or(""));
    da != db && same_subject && a.found_by.iter().any(|q| b.found_by.contains(q))
}

/// Every clashing pair, as indices into `c`, older first; pairs in the order of `c`.
pub fn clash_pairs(c: &[Candidate]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for i in 0..c.len() {
        for j in i + 1..c.len() {
            if clashing(&c[i], &c[j]) {
                out.push(if c[i].found.date_ms < c[j].found.date_ms { (i, j) } else { (j, i) });
            }
        }
    }
    out
}

/// Applies the rule. Returns the excerpts that stay (in their order) and the pairs found, as chunk ids, older first.
pub fn apply_clash_rule(c: Vec<Candidate>, rule: ClashRule, dropped: &mut Vec<Dropped>) -> (Vec<Candidate>, Vec<(String, String)>) {
    let pairs = clash_pairs(&c);
    let ids = pairs.iter().map(|&(o, n)| (c[o].found.chunk_id.clone(), c[n].found.chunk_id.clone())).collect();
    if rule == ClashRule::KeepBothDated {
        return (c, ids);
    }
    let date = |i: usize| c[i].found.date_ms.unwrap_or(0);
    // for each excerpt, the partner the rule prefers to it (the newest newer one, or the oldest older one), if any
    let winner = |i: usize| -> Option<usize> {
        let partners = pairs.iter().filter_map(|&(o, n)| if o == i { Some(n) } else if n == i { Some(o) } else { None });
        match rule {
            ClashRule::KeepNewest => partners.filter(|&p| date(p) > date(i)).max_by_key(|&p| date(p)),
            ClashRule::KeepOldest => partners.filter(|&p| date(p) < date(i)).min_by_key(|&p| date(p)),
            ClashRule::KeepBothDated => None,
        }
    };
    // followed to the end of the chain, so the trace names an excerpt that is really kept (dates move one way, so the chain ends)
    let last = |i: usize| -> Option<usize> {
        let mut w = winner(i)?;
        while let Some(next) = winner(w) {
            w = next;
        }
        Some(w)
    };
    let verdicts: Vec<Option<usize>> = (0..c.len()).map(last).collect();
    let chunk_of: Vec<String> = c.iter().map(|x| x.found.chunk_id.clone()).collect();
    let mut kept = Vec::new();
    for (i, cand) in c.into_iter().enumerate() {
        match verdicts[i] {
            Some(w) => dropped.push(Dropped { chunk_id: cand.found.chunk_id, reason: DropReason::Clash { kept: chunk_of[w].clone() } }),
            None => kept.push(cand),
        }
    }
    (kept, ids)
}
