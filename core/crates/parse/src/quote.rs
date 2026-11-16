//! Where quoted text starts, in plain text and in sanitised HTML.
use std::sync::OnceLock;

use regex::Regex;

use crate::tuning::*;

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("static pattern"))
}

fn attr_start_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"^(?:On|Le|Op|Am|El|Il|Den|Em) ")
}

fn attr_end_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)(?:wrote|a écrit|écrit|schreef\b.*|schrieb\b.*|escribió|ha scritto)\s*:\s*$")
}

fn original_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)^-{2,}\s*(?:original message|message d'origine|oorspronkelijk bericht|ursprüngliche nachricht|mensaje original)\s*-{2,}$")
}

fn from_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)^(?:from|de|van|von)\s*:")
}

fn sent_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)^(?:sent|envoy[ée]|verzonden|gesendet)\s*:")
}

/// Byte offset in `text` where the quoted part begins, or None when nothing is quoted or
/// nothing visible comes before the quote.
pub fn text_quote_start(text: &str) -> Option<usize> {
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut off = 0;
    for l in text.split_inclusive('\n') {
        lines.push((off, l.trim()));
        off += l.len();
    }
    let mut best: Option<usize> = None;
    let mut take = |o: usize| best = Some(best.map_or(o, |b| b.min(o)));

    for (i, (o, line)) in lines.iter().enumerate() {
        if attr_start_re().is_match(line) {
            let mut joined = String::new();
            for k in 0..ATTRIBUTION_WRAP_LINES {
                let Some((_, l)) = lines.get(i + k) else { break };
                if !joined.is_empty() {
                    joined.push(' ');
                }
                joined.push_str(l);
                if attr_end_re().is_match(&joined) {
                    take(*o);
                    break;
                }
            }
        }
        if original_re().is_match(line) {
            take(*o);
        }
        if line.len() >= 10 && line.chars().all(|c| c == '_') && lines.iter().skip(i + 1).map(|(_, l)| *l).find(|l| !l.is_empty()).is_some_and(|l| from_re().is_match(l)) {
            take(*o);
        }
        if from_re().is_match(line) && i > 0 && lines[i - 1].1.is_empty() && lines.iter().skip(i + 1).take(QUOTE_HEADER_LINES).any(|(_, l)| sent_re().is_match(l)) {
            take(*o);
        }
    }
    // a trailing run of "> " lines with nothing but quoted lines after it
    if let Some(first) = lines.iter().position(|(_, l)| l.starts_with('>')) {
        if lines[first..].iter().all(|(_, l)| l.is_empty() || l.starts_with('>')) {
            take(lines[first].0);
        }
    }
    best.filter(|o| !text[..*o].trim().is_empty())
}

fn html_markers() -> &'static [Regex; 3] {
    static C: OnceLock<[Regex; 3]> = OnceLock::new();
    C.get_or_init(|| {
        [
            Regex::new(r#"(?i)<div class="(?:gmail_quote|yahoo_quoted)"#).expect("static"),
            Regex::new(r"(?i)<blockquote\b").expect("static"),
            Regex::new(r"(?is)<hr\b[^>]*>\s*(?:<(?:div|p|span|font|b|strong)\b[^>]*>\s*)*(?:<b>)?\s*(?:From|De|Van|Von)\s*:").expect("static"),
        ]
    })
}

/// Byte offset in sanitised HTML where quoted text begins. None when nothing is quoted or no
/// visible text comes before the quote.
pub fn html_quote_start(safe_html: &str) -> Option<usize> {
    let off = html_markers().iter().filter_map(|r| r.find(safe_html).map(|m| m.start())).min()?;
    let visible: String = {
        static T: OnceLock<Regex> = OnceLock::new();
        re(&T, r"<[^>]*>").replace_all(&safe_html[..off], "").into_owned()
    };
    if visible.trim().is_empty() { None } else { Some(off) }
}
