//! Words to chips (C-J2.2). Filter words are case-blind. A filter word with no value, or a value not
//! understood, stays as plain words; the second case is also listed in `not_understood` so the bar can say so.
//!
//! - `from:<who>` or `from:"<who>"` (SPEC A142): the typed form is the only sender filter; the word "from" in a sentence stays a
//!   word ("email from consultant" makes no chip). Without a value (`from:` alone) it stays a word too.
//! - `before <date>` · `after <date>`: a month ("June", "June 2025") or a day ("2026-06-01"). Before a month means
//!   before its first day; after a month means from the first day of the next one.
//! - `has attachment` (or `has attachments`) · `unread` (or `is unread`).
//! - `account <name>`: the longest run of following words naming an account's display name.
use crate::domain::dates::{span_of, CivilDate};
use crate::domain::{AccountState, Chip, ChipKind, ParsedQuery};

pub struct ParseContext<'a> {
    pub today: CivilDate,
    pub utc_offset_min: i32,
    pub accounts: &'a [AccountState],
}

const FILTER_WORDS: [&str; 6] = ["from", "before", "after", "has", "unread", "account"];

fn is_filter_word(token: &str) -> bool {
    !token.starts_with('"') && FILTER_WORDS.contains(&token.to_lowercase().as_str())
}

/// The value of a typed `from:` token (case-blind key), its quotes taken off and trimmed; None for any other token.
fn from_value(token: &str) -> Option<&str> {
    let key = token.get(..5)?;
    key.eq_ignore_ascii_case("from:").then(|| token[5..].trim_matches('"').trim())
}

/// Whitespace-separated tokens; a quoted phrase is one token with its quotes. An unterminated quote runs to the end. A quote right
/// after a colon (`from:"Linea Supply"`) belongs to the token before it, so the typed form's value is one token with its spaces.
pub(crate) fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '"' {
            let mut t = String::from(chars.next().unwrap_or('"'));
            for ch in chars.by_ref() {
                t.push(ch);
                if ch == '"' {
                    break;
                }
            }
            out.push(t);
        } else {
            let mut t = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace() {
                    break;
                }
                t.push(ch);
                chars.next();
                if ch == ':' && chars.peek() == Some(&'"') {
                    t.push(chars.next().unwrap_or('"'));
                    for q in chars.by_ref() {
                        t.push(q);
                        if q == '"' {
                            break;
                        }
                    }
                }
            }
            out.push(t);
        }
    }
    out
}

fn push_chip(q: &mut ParsedQuery, chip: Chip) {
    let same = |c: &Chip| c.kind == chip.kind && c.value.as_deref().map(str::to_lowercase) == chip.value.as_deref().map(str::to_lowercase);
    if !q.chips.iter().any(same) {
        q.chips.push(chip);
    }
}

pub fn parse_query(text: &str, ctx: &ParseContext<'_>) -> ParsedQuery {
    let toks = tokens(text);
    let mut q = ParsedQuery::default();
    let mut plain: Vec<String> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        let lower = t.to_lowercase();
        let next_is = |w: &[&str]| toks.get(i + 1).is_some_and(|n| w.contains(&n.to_lowercase().as_str()));
        if t.starts_with('"') {
            let inner = t.trim_matches('"').trim();
            if !inner.is_empty() {
                q.phrases.push(inner.to_string());
                plain.push(format!("\"{inner}\""));
            }
            i += 1;
        } else if lower == "has" && next_is(&["attachment", "attachments"]) {
            push_chip(&mut q, Chip { kind: ChipKind::Attachment, value: None, words: format!("{t} {}", toks[i + 1]) });
            q.filters.has_attachment = true;
            i += 2;
        } else if (lower == "is" && next_is(&["unread"])) || lower == "unread" {
            let used = if lower == "is" { 2 } else { 1 };
            push_chip(&mut q, Chip { kind: ChipKind::Unread, value: None, words: toks[i..i + used].join(" ") });
            q.filters.unread = true;
            i += used;
        } else if let Some(v) = from_value(t).filter(|v| !v.is_empty()) {
            // SPEC A142: only the typed form makes a sender chip; its words are the token as typed (the chip's x removes them)
            let v = v.to_string();
            if !q.filters.from.contains(&v.to_lowercase()) {
                q.filters.from.push(v.to_lowercase());
            }
            push_chip(&mut q, Chip { kind: ChipKind::From, value: Some(v), words: t.clone() });
            i += 1;
        } else if lower == "before" || lower == "after" {
            let rest: Vec<&str> = toks[i + 1..].iter().map(String::as_str).collect();
            match span_of(&rest, ctx.today) {
                Some((span, used)) => {
                    let v = rest[..used].join(" ");
                    let (kind, bound) = if lower == "before" { (ChipKind::Before, span.first) } else { (ChipKind::After, span.end) };
                    let ms = bound.start_ms(ctx.utc_offset_min);
                    if kind == ChipKind::Before {
                        q.filters.before_ms = Some(q.filters.before_ms.map_or(ms, |b| b.min(ms)));
                    } else {
                        q.filters.after_ms = Some(q.filters.after_ms.map_or(ms, |a| a.max(ms)));
                    }
                    push_chip(&mut q, Chip { kind, value: Some(v.clone()), words: format!("{t} {v}") });
                    i += 1 + used;
                }
                None => match rest.first().filter(|w| !is_filter_word(w)) {
                    Some(w) => {
                        let words = format!("{t} {w}");
                        q.not_understood.push(words.clone());
                        plain.push(words);
                        i += 2;
                    }
                    None => {
                        plain.push(t.clone());
                        i += 1;
                    }
                },
            }
        } else if lower == "account" {
            let rest: Vec<String> = toks[i + 1..].iter().take_while(|w| !is_filter_word(w)).map(|w| w.to_lowercase()).collect();
            let found = (1..=rest.len()).rev().find_map(|n| {
                let name = rest[..n].join(" ");
                ctx.accounts.iter().find(|a| a.name.to_lowercase() == name).map(|a| (a, n))
            });
            match found {
                Some((account, n)) => {
                    if !q.filters.accounts.contains(&account.id) {
                        q.filters.accounts.push(account.id.clone());
                    }
                    let v = toks[i + 1..i + 1 + n].join(" ");
                    push_chip(&mut q, Chip { kind: ChipKind::Account, value: Some(v.clone()), words: format!("{t} {v}") });
                    i += 1 + n;
                }
                None => {
                    plain.push(t.clone());
                    i += 1;
                }
            }
        } else {
            plain.push(t.clone());
            i += 1;
        }
    }
    q.residual = plain.join(" ");
    q
}
