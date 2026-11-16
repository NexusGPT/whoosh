//! Marking the searched words inside a snippet. Whole words only and case-blind: "0.84 EUR" is marked in
//! "is 0.84 EUR," and not inside "110.84 EUR". The answer is a list of runs, so no offset unit crosses the FFI.
use crate::domain::Segment;

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Runs of `text`, each searched term marked. Longer terms win where two overlap.
pub fn mark_words(text: &str, terms: &[String]) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let folded: Vec<char> = chars.iter().map(|c| fold(*c)).collect();
    let mut wanted: Vec<Vec<char>> = terms.iter().map(|t| t.trim().chars().map(fold).collect::<Vec<char>>()).filter(|t| !t.is_empty()).collect();
    wanted.sort_by_key(|t| std::cmp::Reverse(t.len()));
    let mut marked = vec![false; chars.len()];
    let edge = |i: Option<usize>| i.and_then(|i| chars.get(i)).is_none_or(|c| !c.is_alphanumeric());
    let mut i = 0;
    while i < chars.len() {
        let hit = wanted.iter().find(|t| folded[i..].starts_with(t) && edge(i.checked_sub(1)) && edge(Some(i + t.len())));
        match hit {
            Some(t) => {
                marked[i..i + t.len()].iter_mut().for_each(|m| *m = true);
                i += t.len();
            }
            None => i += 1,
        }
    }
    let mut out: Vec<Segment> = Vec::new();
    for (c, m) in chars.into_iter().zip(marked) {
        match out.last_mut() {
            Some(s) if s.marked == m => s.text.push(c),
            _ => out.push(Segment { text: c.to_string(), marked: m }),
        }
    }
    out
}
