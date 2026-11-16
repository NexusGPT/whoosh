//! Step 3: each excerpt cut to the size the owner chose. The quoted history (lines starting with '>') is removed first: it repeats an earlier
//! message, and the writer is never handed it. A sentence or a paragraph is the piece that holds the most words of the questions that found the
//! excerpt (the first such piece on a tie); a section is the whole excerpt.
use crate::domain::text::words;
use contracts::ExcerptSize;

/// The text without quoted lines, trimmed.
pub fn unquoted(text: &str) -> String {
    text.lines().filter(|l| !l.trim_start().starts_with('>')).collect::<Vec<_>>().join("\n").trim().to_string()
}

/// The paragraphs of a text: split at blank lines, each trimmed, none empty.
pub fn paragraphs(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in text.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                out.push(cur.join("\n").trim().to_string());
                cur.clear();
            }
        } else {
            cur.push(line);
        }
    }
    out
}

/// The sentences of a paragraph: a sentence ends at '.', '!', '?' or '…' followed by a space or the end. A line break inside the paragraph is
/// a space (mail is often wrapped at 72 characters, mid-sentence).
pub fn sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let cs: Vec<char> = text.chars().collect();
    for (i, &c) in cs.iter().enumerate() {
        cur.push(if c == '\n' { ' ' } else { c });
        let ends = matches!(c, '.' | '!' | '?' | '…') && cs.get(i + 1).is_none_or(|n| n.is_whitespace());
        if ends {
            push_trimmed(&mut out, &mut cur);
        }
    }
    push_trimmed(&mut out, &mut cur);
    out
}

fn push_trimmed(out: &mut Vec<String>, cur: &mut String) {
    let t = cur.split_whitespace().collect::<Vec<_>>().join(" ");
    if !t.is_empty() {
        out.push(t);
    }
    cur.clear();
}

/// How many of `wanted` the piece holds.
fn hits(piece: &str, wanted: &[String]) -> usize {
    let have = words(piece);
    wanted.iter().filter(|w| have.contains(w)).count()
}

/// The piece holding the most wanted words; the first on a tie. "" for no piece.
fn best(pieces: Vec<String>, wanted: &[String]) -> String {
    let mut pick: Option<(usize, String)> = None;
    for p in pieces {
        let h = hits(&p, wanted);
        if pick.as_ref().is_none_or(|(best, _)| h > *best) {
            pick = Some((h, p));
        }
    }
    pick.map(|(_, p)| p).unwrap_or_default()
}

/// The excerpt at `size`, for questions whose words are `wanted`. "" when nothing is left once the quoted lines are gone.
pub fn size_text(text: &str, size: ExcerptSize, wanted: &[String]) -> String {
    let clean = unquoted(text);
    match size {
        ExcerptSize::Section => clean,
        ExcerptSize::Paragraph => best(paragraphs(&clean), wanted),
        ExcerptSize::Sentence => best(paragraphs(&clean).iter().flat_map(|p| sentences(p)).collect(), wanted),
    }
}
