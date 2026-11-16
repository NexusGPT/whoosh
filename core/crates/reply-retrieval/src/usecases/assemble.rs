//! Step 7b: what the writer is handed, inside its budget. Each excerpt is a block "label line, new line, text"; blocks are joined by a blank
//! line, best first. When the room runs out, the lowest-ranked go first (dropped as NoRoom); when even the best alone is over the budget, its
//! text is cut instead, never the whole context refused. Numbers are given AFTER the cut, so they run 1..n with no gap, and a clash label
//! names only excerpts the writer really has. Never exceeds `budget` characters.
use crate::domain::text::{chars, take_chars};
use crate::domain::{ContextExcerpt, DropReason, Dropped};
use crate::usecases::gather::Candidate;
use crate::usecases::label_line::label_line;
use contracts::{ClashRule, ExcerptFields};

fn block(label: &str, text: &str) -> String {
    format!("{label}\n{text}")
}

/// The labels of `c` numbered 1..n, each with the numbers of its clash partners inside `c` (KeepBothDated only; the other rules left none).
fn labelled(c: &[Candidate], pairs: &[(String, String)], fields: ExcerptFields, rule: ClashRule) -> Vec<ContextExcerpt> {
    let number_of = |id: &str| c.iter().position(|x| x.found.chunk_id == id).map(|i| i as u32 + 1);
    c.iter()
        .enumerate()
        .map(|(i, cand)| {
            let id = cand.found.chunk_id.as_str();
            let mut with: Vec<u32> = Vec::new();
            if rule == ClashRule::KeepBothDated {
                for (a, b) in pairs {
                    let other = if a == id { b } else if b == id { a } else { continue };
                    with.extend(number_of(other));
                }
            }
            with.sort_unstable();
            with.dedup();
            let excerpt = cand.found.excerpt();
            ContextExcerpt { number: i as u32 + 1, label: label_line(i as u32 + 1, &excerpt, fields, !with.is_empty(), &with), excerpt, clashes_with: with }
        })
        .collect()
}

/// The context and its text. Returns the excerpts handed on, the text, and whether the one excerpt kept had to be cut.
pub fn assemble(c: Vec<Candidate>, pairs: &[(String, String)], fields: ExcerptFields, rule: ClashRule, budget: usize, dropped: &mut Vec<Dropped>) -> (Vec<ContextExcerpt>, String, bool) {
    // A label with every candidate present is at least as long as the label the same excerpt gets once some are cut (its number can only get
    // smaller, its clash list only shorter), so a choice made on these lengths still fits once renumbered.
    let provisional = labelled(&c, pairs, fields, rule);
    let mut used = 0usize;
    let mut take = 0usize;
    for p in &provisional {
        let need = chars(&block(&p.label, &p.excerpt.text)) + if take == 0 { 0 } else { 2 };
        if used + need > budget {
            break;
        }
        used += need;
        take += 1;
    }
    let mut cut = false;
    let mut c = c;
    if take == 0 && !c.is_empty() {
        // the best alone is over the budget: keep the start of its text
        let label = label_line(1, &c[0].found.excerpt(), fields, false, &[]);
        let room = budget.saturating_sub(chars(&label) + 1);
        if room > 0 {
            c[0].found.text = take_chars(&c[0].found.text, room);
            take = 1;
            cut = true;
        }
    }
    for rest in c.split_off(take) {
        dropped.push(Dropped { chunk_id: rest.found.chunk_id, reason: DropReason::NoRoom });
    }
    let context = labelled(&c, pairs, fields, rule);
    let text = context.iter().map(|x| block(&x.label, &x.excerpt.text)).collect::<Vec<_>>().join("\n\n");
    (context, text, cut)
}
