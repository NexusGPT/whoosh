//! Text cut into paragraph-sized excerpts: paragraphs are split at blank lines, a short one joins the next (up to the maximum), a long
//! one is cut at a sentence end, else at a space, else at the maximum.

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// The paragraphs of a text: runs of non-blank lines, each line's spaces collapsed, lines kept apart by a line break.
fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    for line in text.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() {
            if !cur.is_empty() {
                out.push(cur.join("\n"));
                cur.clear();
            }
        } else {
            cur.push(l);
        }
    }
    if !cur.is_empty() {
        out.push(cur.join("\n"));
    }
    out
}

/// One paragraph longer than `max` cut into pieces of at most `max` characters.
fn cut_long(p: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest: &str = p;
    while chars(rest) > max {
        let limit = rest.char_indices().nth(max).map(|(i, _)| i).unwrap_or(rest.len());
        let head = &rest[..limit];
        let sentence = head.rfind(|c: char| matches!(c, '.' | '!' | '?' | '\n')).map(|i| i + 1).filter(|&i| i > limit / 2);
        let at = sentence.or_else(|| head.rfind(' ').filter(|&i| i > 0)).unwrap_or(limit);
        out.push(rest[..at].trim().to_string());
        rest = rest[at..].trim_start();
    }
    if !rest.trim().is_empty() {
        out.push(rest.trim().to_string());
    }
    out
}

/// The excerpts of a text, each at most `max` characters, each at least `min` when the text allows it.
pub fn split_excerpts(text: &str, min: usize, max: usize) -> Vec<String> {
    let pieces: Vec<String> = paragraphs(text).iter().flat_map(|p| cut_long(p, max)).collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for piece in pieces {
        if cur.is_empty() {
            cur = piece;
        } else if chars(&cur) < min && chars(&cur) + 2 + chars(&piece) <= max {
            cur.push_str("\n\n");
            cur.push_str(&piece);
        } else {
            out.push(std::mem::replace(&mut cur, piece));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
