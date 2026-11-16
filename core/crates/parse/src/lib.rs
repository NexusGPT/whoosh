//! Whoosh parse crate: pure functions over bytes. No network, no store, no clock.
//!
//! Built on `mail-parser` (MIME and headers, Apache-2.0 OR MIT) and `ammonia` (allow-list HTML
//! sanitiser, MIT OR Apache-2.0); `icalendar`'s parser reads invite parts. What this crate adds:
//! remote images replaced and counted, login-code and sign-in-link extraction, calendar-invite
//! reading and quoted-text boundaries, plus the typed question the decision contract expects.
mod canon;
mod code;
mod css;
mod display;
mod dom;
mod engine;
mod nfc;
mod pagecode;
mod segment;
mod entities;
mod invite;
mod mime;
mod quote;
mod sanitise;
pub mod tuning;

use std::collections::HashMap;

use contracts::{DecisionQuestion, DecisionState};

pub use code::{extract_login, LoginFinding};
pub use entities::{decode_entities, plain_text};
pub use invite::{parse_invite, EventTime, Invite};
// fix-realcore round 7: the composed form (NFC) of a text, for file names too (send, engine draft files)
pub use nfc::nfc;
pub use mime::{attachments_of, attachments_with_bytes, decode_transfer, parse_message, Address, Attachment, ParseError, ParsedMessage};
pub use quote::{html_quote_start, text_quote_start};
pub use sanitise::{keep_picture_addresses, safe_html_to_text, sanitise_html, SafeHtml, BLOCKED_IMAGE_SRC};
// body-r4: the mail content engine
pub use canon::{canonical, decode_text, quote_depth, unflow, windows_1252, CharsetDowngrade, UTF8_REPLACEMENT_MAX_SHARE};
pub use css::{blocked_property, sanitize_style, Inlined, Skipped, ELEMENTS_MAX, MATCHES_MAX, SELECTORS_MAX, STYLE_BYTES_MAX};
pub use display::{
    decide_layout, first_pass as display_first_pass, is_tracker, remote_address, KEPT_SRC_ATTR, PICTURE_MIN_PX, page_hides_text, plain_to_html, shown, shown_words, Shown, plain_document, written_document, PAGE_HIDES_TEXT_MIN_WORDS, PAGE_HIDES_TEXT_SHARE, Layout, LayoutFeatures, LayoutReason, DISPLAY_DEPTH_MAX, LAYOUT_CELL_BACKGROUNDS_MIN,
    LAYOUT_FIXED_WIDTH_MAX_PX, LAYOUT_FIXED_WIDTH_MIN_PX, LAYOUT_PICTURES_MIN, LAYOUT_STYLE_RULES_MIN, LAYOUT_TABLE_TEXT_SHARE_MIN,
    LAYOUT_TEXT_CHARS_PER_PICTURE_MAX, LAYOUT_WRAPPER_MIN_WIDTH_PERCENT, LAYOUT_WRAPPER_MIN_WIDTH_PX, PLAIN_QUOTE_DEPTH_MAX,
};
pub use pagecode::{close_cut_tags, page_code_as_page, page_code_words, right_parts, starts_with_page_tag, text_is_page_code, PAGE_HOLDS_TEXT_PERCENT, TEXT_PAGE_CODE_MIN_TAGS};
pub use engine::{nesting_estimate, BODY_BYTES_MAX, HTML_NESTING_MAX, parts_of, prepare, prepare_in_background, prepare_in_background_stalled, prepare_message, Options, Parts, Prepared, Warning, PREPARE_RETRY_TIME_LIMIT_MS, PREPARE_TIME_LIMIT_MS};
pub use segment::{
    band, fold_from, says_something, Reason, Segment, SegmentKind, Visibility, ALIGN_CANDIDATE_SHARE, ALIGN_MAX_TOKENS, ALIGN_MIN_SHARE, BOTTOM_POST_MIN_LINES,
    COLLAPSE_MIN_CONFIDENCE, FOOTER_MAX_BLOCKS, HEADER_BLOCK_MIN_LINES, HIDE_MIN_CONFIDENCE, PARTS_DISAGREE_FACTOR, SHINGLE_MIN_SHARE,
    SHINGLE_WORDS, SIGNATURE_MAX_LINES, THREAD_MATCH_MIN_CHARS, THREAD_MATCH_MIN_WORDS,
};

/// The id of the one question this crate builds for the decision contract.
pub const SIGN_IN_QUESTION_ID: &str = "sign-in-code";
/// The question's words, exactly as docs/DECISION-MODEL.md ships them.
pub const SIGN_IN_QUESTION_PROMPT: &str = "Does this email carry a one-time code, a login link or a verification number?";

/// Everything the thread screen, the notch and the list need from one message.
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub message: ParsedMessage,
    /// The sanitised HTML body, when the message has an HTML part.
    pub safe_html: Option<SafeHtml>,
    pub login: LoginFinding,
    /// The first calendar invite found in the message.
    pub invite: Option<Invite>,
    /// Byte offset in `message.text` where quoted text starts.
    pub text_quote_start: Option<usize>,
    /// Remote images replaced; 0 when there is no HTML part.
    pub trackers_blocked: u32,
    /// First characters of the visible (unquoted) text, whitespace collapsed.
    pub preview: String,
}

/// The list's preview of a text: its words on one line, the first `PREVIEW_CHARS` of them. A run of rule characters ("---", "__________",
/// "=====") is a drawn line, not a word: it is left out, so the preview starts at the first word (owner's store, 13 Nov 2026: the preview of
/// a forwarded mail began with 81 characters of dashes). Empty when the text holds nothing but such lines.
pub fn preview_of(visible: &str) -> String {
    visible.split_whitespace().filter(|w| !is_drawn_line(w)).collect::<Vec<_>>().join(" ").chars().take(tuning::PREVIEW_CHARS).collect()
}

/// `preview_of`; a text that is nothing but drawn lines shows them as they are (a mail that says "---" has that preview).
pub fn preview_or_raw(text: &str) -> String {
    let p = preview_of(text);
    if !p.is_empty() { return p; }
    text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(tuning::PREVIEW_CHARS).collect()
}

/// A drawn line: three or more characters, each a dash, an underscore, an equals sign, a star, a tilde, a hash, a bullet or a box stroke.
/// A full stop is not one ("..." is an answer), nor is "???" or "!!!".
fn is_drawn_line(word: &str) -> bool {
    word.chars().count() >= 3
        && word.chars().all(|c| matches!(c, '-' | '_' | '=' | '*' | '~' | '#' | '\u{00AF}' | '\u{00B7}' | '\u{2010}'..='\u{2015}' | '\u{2022}' | '\u{2500}'..='\u{257F}'))
}

/// Parse one RFC 5322 message and run every extraction over it.
pub fn analyse(bytes: &[u8]) -> Result<Analysis, ParseError> {
    let message = parse_message(bytes)?;
    let safe_html = message.html.as_deref().map(sanitise_html);
    let login = extract_login(&message.subject, &message.text, safe_html.as_ref().map(|s| s.html.as_str()));
    let invite = message.calendar_parts.iter().find_map(|p| parse_invite(p));
    let text_quote_start = text_quote_start(&message.text);
    let visible = &message.text[..text_quote_start.unwrap_or(message.text.len())];
    let preview = preview_of(visible);
    let trackers_blocked = safe_html.as_ref().map_or(0, |s| s.trackers_blocked);
    Ok(Analysis { message, safe_html, login, invite, text_quote_start, trackers_blocked, preview })
}

/// The decision-contract input for "does this message carry a sign-in code?": the message as
/// state, one `Noul` (yes/no) question in the words of docs/DECISION-MODEL.md. The cheap rule
/// step (subject words) and the extractor's own finding ride in `fields` for the rules layer.
pub fn sign_in_decision(a: &Analysis) -> (DecisionState, Vec<DecisionQuestion>) {
    let visible = &a.message.text[..a.text_quote_start.unwrap_or(a.message.text.len())];
    let text: String = format!("{}\n\n{}", a.message.subject, visible).chars().take(tuning::DECISION_TEXT_CHARS).collect();
    let mut fields = HashMap::new();
    fields.insert("subject".to_string(), a.message.subject.clone());
    if let Some(from) = &a.message.from {
        fields.insert("from".to_string(), from.email.clone());
    }
    fields.insert("candidate_code".to_string(), a.login.code.clone().unwrap_or_default());
    fields.insert("has_sign_in_link".to_string(), (!a.login.links.is_empty()).to_string());
    let q = DecisionQuestion::Noul { id: SIGN_IN_QUESTION_ID.to_string(), prompt: SIGN_IN_QUESTION_PROMPT.to_string() };
    (DecisionState { text, fields }, vec![q])
}
