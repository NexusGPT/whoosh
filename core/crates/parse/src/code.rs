//! Login codes (one-time codes, verification numbers) and sign-in links.
//!
//! A candidate token is only a code when the words around it say so (a "code" word, a "use ...
//! to sign in" verb, "<n> is your ... code") and no word says it is something else (promo,
//! postal, order, invoice...). Mail in English, French, Dutch and German is read.
use std::sync::OnceLock;

use regex::Regex;

use crate::tuning::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoginFinding {
    /// The code with spaces and hyphens removed (and Google's `G-` prefix dropped).
    pub code: Option<String>,
    /// Sign-in or verification links, in order of appearance, without duplicates.
    pub links: Vec<String>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("static pattern"))
}

fn cand_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, &format!(
        r"(?:G-)?(?:\b[0-9]{{3}}[ -][0-9]{{3}}\b|\b[0-9]{{4}}[ -][0-9]{{4}}\b|\b[0-9]{{{CODE_DIGITS_MIN},{CODE_DIGITS_MAX}}}\b|\b[A-Z0-9]{{{CODE_ALNUM_MIN},{CODE_ALNUM_MAX}}}\b)"
    ))
}

fn code_word_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?:^|[^a-zà-ÿ])(?:[a-zà-ÿ]*(?:code|kode)|pin|otp|passcode|2fa)(?:[^a-zà-ÿ0-9]|$)")
}

fn verb_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?:^|\s)(?:use|enter|entering|type|input|saisissez|utilisez|gebruik|voer|geben sie|verwende)\s*$")
}

fn qual_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?:sign[- ]?in|log[- ]?in|verif|confirm|authenticat|one[- ]time|security|expire|2fa|two[- ]factor|connect|vérif|sécurit|verifi|inlogg|loggen|bevestig|bestätig|anmeld|gültig|verloopt)")
}

fn after_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"^\s+(?:is|est|ist)\s+(?:your|votre|ihr|dein|uw|je)\b.{0,40}?(?:code|pin|passcode|otp|kode)")
}

fn neg_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"promo|discount|coupon|voucher|gift|%|\boff\b|\bsave\b|\bfree\b|referral|redeem|postal|postcode|post code|zip|product|order|invoice|tracking|reference|référence|item|article|conduct|barcode|area code|country|dial|iban|\bvat\b|\bbtw\b|\btva\b|phone|\btel\b|call|ticket|\broom\b|flight|booking|reservation")
}

fn month_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s*$")
}

fn sentence_end_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"[.!?]\s")
}

fn before(s: &str, pos: usize, chars: usize) -> &str {
    let start = s[..pos].char_indices().rev().nth(chars.saturating_sub(1)).map_or(0, |(i, _)| i);
    &s[start..pos]
}

fn after(s: &str, pos: usize, chars: usize) -> &str {
    let end = s[pos..].char_indices().nth(chars).map_or(s.len(), |(i, _)| pos + i);
    &s[pos..end]
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn own_line(s: &str, start: usize, end: usize) -> bool {
    let line_start = s[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = s[end..].find('\n').map_or(s.len(), |i| end + i);
    s[line_start..start].trim().is_empty() && s[end..line_end].trim().is_empty()
}

fn year_like(d: &str) -> bool {
    d.len() == 4 && (d.starts_with("19") || d.starts_with("20"))
}

/// Score one candidate: 0 = not a code, 1 = weak cue, 2 = a code word nearby, 3 = a code word
/// directly in front.
fn score(hay: &str, m: &regex::Match<'_>) -> u8 {
    let tok = m.as_str();
    let (start, end) = (m.start(), m.end());
    let prev = hay[..start].chars().next_back();
    let next = hay[end..].chars().next();
    let next2 = hay[end..].chars().nth(1);
    let prev2 = hay[..start].chars().rev().nth(1);
    if matches!(prev, Some('#' | '€' | '$' | '£' | '+' | '/')) {
        return 0;
    }
    if matches!(prev, Some('.' | ',' | '-')) && prev2.is_some_and(|c| c.is_ascii_digit()) {
        return 0;
    }
    if matches!(next, Some('%' | '/')) || (matches!(next, Some('.' | ',' | ':' | '-')) && next2.is_some_and(|c| c.is_ascii_digit())) {
        return 0;
    }
    let digits: String = tok.trim_start_matches("G-").chars().filter(|c| c.is_ascii_digit()).collect();
    let letters = tok.trim_start_matches("G-").chars().filter(|c| c.is_ascii_alphabetic()).count();
    let pure_digits = letters == 0;
    // A bare run of capitals and digits must be a real mixed token or sit alone on its line.
    let alnum_ok = pure_digits || (letters > 0 && !digits.is_empty()) || own_line(hay, start, end);
    if !alnum_ok {
        return 0;
    }

    let neg_before = before(hay, start, NEGATIVE_BEFORE_CHARS).to_lowercase();
    let neg_after = after(hay, end, NEGATIVE_AFTER_CHARS).to_lowercase();
    if neg_re().is_match(&neg_before) || neg_re().is_match(&neg_after) {
        return 0;
    }
    let cue_window = collapse(&before(hay, start, CUE_WINDOW_CHARS).to_lowercase());
    let mut best = 0u8;
    // the last code word in the window, and what stands between it and the candidate
    if let Some(m) = code_word_re().find_iter(&cue_window).last() {
        let tail = &cue_window[m.end().saturating_sub(1)..];
        if !tail.chars().any(|c| c.is_ascii_digit()) && !sentence_end_re().is_match(tail) {
            let direct = tail.chars().count() <= 25;
            best = if direct { 3 } else { 2 };
        }
    }
    let wide = collapse(&format!("{}{}", before(hay, start, CUE_WINDOW_CHARS), after(hay, end, CUE_WINDOW_CHARS))).to_lowercase();
    if best == 0 && verb_re().is_match(&cue_window) && qual_re().is_match(&wide) {
        best = 1;
    }
    if best == 0 && after_re().is_match(&after(hay, end, 80).to_lowercase()) {
        best = 3;
    }
    if best == 0 {
        return 0;
    }
    let is_year = year_like(&digits) && pure_digits && tok.len() == 4;
    if is_year && best < 3 {
        return 0;
    }
    if month_re().is_match(&before(hay, start, 12).to_lowercase()) {
        return 0;
    }
    if !pure_digits && digits.is_empty() && best < 2 {
        return 0;
    }
    best
}

fn find_code(subject: &str, text: &str) -> Option<String> {
    let hay = format!("{subject}\n{text}");
    let mut best: Option<(u8, String)> = None;
    for m in cand_re().find_iter(&hay) {
        let s = score(&hay, &m);
        if s == 0 {
            continue;
        }
        let norm: String = m.as_str().trim_start_matches("G-").chars().filter(|c| *c != ' ' && *c != '-').collect();
        if best.as_ref().is_none_or(|(b, _)| s > *b) {
            best = Some((s, norm));
        }
    }
    best.map(|(_, c)| c)
}

// ---- links ----

fn anchor_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r#"(?is)<a\s[^>]*?href="([^"]*)"[^>]*>(.*?)</a>"#)
}

fn tag_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"<[^>]*>")
}

fn url_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r#"https?://[^\s<>"')\]]+"#)
}

fn url_signal_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)verif|confirm|magic|log-?in|sign-?in|activat|/auth\b|otp|token=|callback")
}

fn anchor_signal_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)(?:sign|log)[- ]?in|verif|confirm|activat|se connecter|vérifier|confirmer|inloggen|bevestig|anmelden|bestätigen")
}

fn excluded_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, r"(?i)unsubscribe|preferences|privacy|terms|optout|opt-out|view-in-browser|/help|/support")
}

fn token_re() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    re(&C, &format!(r"[A-Za-z0-9_-]{{{LINK_TOKEN_MIN_CHARS},}}"))
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&#x27;", "'").replace("&lt;", "<").replace("&gt;", ">")
}

fn has_token(url: &str) -> bool {
    let after_host = url.splitn(4, '/').nth(3).unwrap_or("");
    token_re().is_match(after_host)
}

fn find_links(text: &str, safe_html: Option<&str>) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut consider = |url: String, anchor: &str| {
        if url.starts_with("http") && !excluded_re().is_match(&url) && has_token(&url) && (url_signal_re().is_match(&url) || anchor_signal_re().is_match(anchor)) && !found.contains(&url) {
            found.push(url);
        }
    };
    if let Some(h) = safe_html {
        for c in anchor_re().captures_iter(h) {
            let anchor = decode_entities(&tag_re().replace_all(&c[2], ""));
            consider(decode_entities(&c[1]), &anchor);
        }
    }
    for m in url_re().find_iter(text) {
        let url = m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?']).to_string();
        let ctx = before(text, m.start(), 40);
        consider(url, ctx);
    }
    found.truncate(LINKS_MAX);
    found
}

/// Find the login code and sign-in links of one message.
/// `text` is the plain body; `safe_html` is the sanitised HTML body when there is one.
pub fn extract_login(subject: &str, text: &str, safe_html: Option<&str>) -> LoginFinding {
    LoginFinding { code: find_code(subject, text), links: find_links(text, safe_html) }
}
