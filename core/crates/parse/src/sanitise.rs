//! HTML to an allow-list (`ammonia`), remote images replaced and counted.
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use ammonia::{Builder, UrlRelative};

use crate::display::{is_tracker, remote_address, KEPT_SRC_ATTR};
use crate::quote::html_quote_start;

/// round 5: every `<img>` tag of the source loses any `data-whoosh-src` it carries, and a picture (not a tracker: `display::is_tracker`)
/// with an http or https `src` gains `data-whoosh-src` holding that address, escaped. A linear scan of the tags; the rest of the text is
/// copied as it is, and ammonia parses the result.
pub fn keep_picture_addresses(html: &str) -> String { mark_pictures(html).0 }

/// `keep_picture_addresses` and the two counts of its remote images (http, https, `//`): (text, pictures, trackers).
fn mark_pictures(html: &str) -> (String, u32, u32) {
    let (mut pictures, mut trackers) = (0u32, 0u32);
    let b = html.as_bytes();
    let mut out = String::with_capacity(html.len() + 64);
    let (mut i, mut copied) = (0usize, 0usize);
    while let Some(off) = html[i..].find('<') {
        let at = i + off;
        let is_img = b.len() >= at + 4 && b[at + 1..at + 4].eq_ignore_ascii_case(b"img") && b.get(at + 4).is_none_or(|c| c.is_ascii_whitespace() || *c == b'/' || *c == b'>');
        if !is_img { i = at + 1; continue; }
        // the attributes, up to the tag's end outside quotes
        let mut j = at + 4;
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut drop: Vec<(usize, usize)> = Vec::new();
        let mut end = None;
        while j < b.len() {
            while j < b.len() && (b[j].is_ascii_whitespace() || b[j] == b'/') { j += 1; }
            if j >= b.len() { break; }
            if b[j] == b'>' { end = Some(j); break; }
            let ns = j;
            while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'/' && b[j] != b'>' && b[j] != b'=' { j += 1; }
            let name = html[ns..j].to_ascii_lowercase();
            let mut value = String::new();
            let mut k = j;
            while k < b.len() && b[k].is_ascii_whitespace() { k += 1; }
            if k < b.len() && b[k] == b'=' {
                k += 1;
                while k < b.len() && b[k].is_ascii_whitespace() { k += 1; }
                if k < b.len() && (b[k] == b'"' || b[k] == b'\'') {
                    let q = b[k];
                    let vs = k + 1;
                    let ve = html[vs..].find(q as char).map_or(b.len(), |x| vs + x);
                    value = html[vs..ve].to_string();
                    j = (ve + 1).min(b.len());
                } else {
                    let vs = k;
                    while k < b.len() && !b[k].is_ascii_whitespace() && b[k] != b'>' { k += 1; }
                    value = html[vs..k].to_string();
                    j = k;
                }
            }
            if name == KEPT_SRC_ATTR { drop.push((ns, j)); }
            if !attrs.iter().any(|(n, _)| *n == name) { attrs.push((name, crate::entities::decode_entities(&value))); }
            if j == ns { j += 1; }
        }
        let Some(end) = end else { break };
        out.push_str(&html[copied..at]);
        let mut pos = at;
        for (s, e) in &drop { out.push_str(&html[pos..*s]); pos = *e; }
        let close = if end > at && b[end - 1] == b'/' && pos < end { end - 1 } else { end };
        out.push_str(&html[pos..close.max(pos)]);
        let src = attrs.iter().find(|(n, _)| n == "src").map(|(_, v)| v.as_str()).unwrap_or("");
        if is_remote(src) { if is_tracker(&attrs) { trackers += 1 } else { pictures += 1 } }
        if let Some(a) = remote_address(src).filter(|_| !is_tracker(&attrs)) {
            out.push(' '); out.push_str(KEPT_SRC_ATTR); out.push_str("=\""); crate::dom::escape_attr(a, &mut out); out.push('"');
        }
        out.push_str(&html[close.max(pos)..=end]);
        copied = end + 1;
        i = end + 1;
    }
    out.push_str(&html[copied..]);
    (out, pictures, trackers)
}

/// Where a blocked image points. The renderer draws its own "image blocked" box for this value;
/// it is a `cid:` reference, so no scheme but `cid:` is ever allowed on an image.
pub const BLOCKED_IMAGE_SRC: &str = "cid:whoosh-blocked";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeHtml {
    pub html: String,
    /// Remote images replaced by the blocked placeholder; each one is a tracker.
    pub trackers_blocked: u32,
    /// Byte offset in `html` where quoted text starts, when there is visible text before it.
    pub quote_offset: Option<usize>,
    /// round 5: the two kinds of the remote images above, as the tag scan counts them: pictures (address kept in `data-whoosh-src` when
    /// http or https) and trackers that say they are invisible (`display::is_tracker`). `trackers_blocked` keeps its meaning (every
    /// remote image, as the 200 fixtures' labels count it).
    pub pictures_blocked: u32,
    pub pixel_trackers: u32,
}

fn is_remote(src: &str) -> bool {
    let s = src.trim().to_ascii_lowercase();
    s.starts_with("http://") || s.starts_with("https://") || s.starts_with("//")
}

pub fn sanitise_html(html: &str) -> SafeHtml {
    let blocked = Arc::new(AtomicU32::new(0));
    let counter = blocked.clone();
    let mut b = Builder::default();
    // allow-list
    b.url_schemes(HashSet::from(["http", "https", "mailto", "tel", "cid"]));
    b.url_relative(UrlRelative::PassThrough);
    b.add_clean_content_tags(["title", "noscript", "template"]);
    b.add_allowed_classes("div", ["gmail_quote", "gmail_attr", "yahoo_quoted"]);
    b.add_allowed_classes("blockquote", ["gmail_quote"]);
    b.add_tag_attributes("img", [KEPT_SRC_ATTR]);
    b.attribute_filter(move |element, attribute, value| {
        if attribute == KEPT_SRC_ATTR {
            return if element == "img" && remote_address(value).is_some() { Some(value.into()) } else { None };
        }
        if attribute == "href" {
            let v = value.trim().to_ascii_lowercase();
            let safe = ["http:", "https:", "mailto:", "tel:", "#"].iter().any(|p| v.starts_with(p));
            return if safe { Some(value.into()) } else { None };
        }
        if element == "img" && attribute == "src" {
            if is_remote(value) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            if !value.trim().to_ascii_lowercase().starts_with("cid:") {
                return Some(BLOCKED_IMAGE_SRC.into());
            }
        }
        Some(value.into())
    });
    let (marked, pictures_blocked, pixel_trackers) = mark_pictures(html);
    let clean = b.clean(&marked).to_string();
    let quote_offset = html_quote_start(&clean);
    SafeHtml { html: clean, trackers_blocked: blocked.load(Ordering::Relaxed), quote_offset, pictures_blocked, pixel_trackers }
}

/// Plain text of sanitised HTML, block by block: every block-level end, `<br>` and `<hr>` starts
/// a new line, so a code in a table cell stays on its own line instead of gluing to the next
/// sentence. Input must be the output of `sanitise_html` (well-formed, no scripts).
pub fn safe_html_to_text(safe_html: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    static CELL: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let block = BLOCK.get_or_init(|| Regex::new(r"(?i)</(?:p|div|tr|table|h[1-6]|li|ul|ol|blockquote|pre)>|<br\s*/?>|<hr\s*/?>").expect("static"));
    let cell = CELL.get_or_init(|| Regex::new(r"(?i)</t[dh]>").expect("static"));
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]*>").expect("static"));
    let with_breaks = block.replace_all(safe_html, "\n");
    let with_cells = cell.replace_all(&with_breaks, " ");
    let stripped = tag.replace_all(&with_cells, "");
    // body-r2: every character reference (the serialiser writes U+00A0 as `&nbsp;`), in one pass: `&amp;nbsp;` stays the text `&nbsp;`
    let decoded = crate::entities::decode_entities(&stripped).replace('\u{a0}', " ");
    let mut out = String::new();
    let mut blank = 0;
    for line in decoded.lines() {
        let l = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if l.is_empty() {
            blank += 1;
            if blank == 1 && !out.is_empty() {
                out.push('\n');
            }
        } else {
            blank = 0;
            out.push_str(&l);
            out.push('\n');
        }
    }
    out
}
