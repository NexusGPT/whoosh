//! gap-composer-rich: a message with an HTML part beside its plain part (multipart/alternative), built only when the draft needs one:
//! a signature with formatting, or a picture inside the text. Anything else goes out exactly as before (`compose` keeps its plain path).
//!
//! What the draft carries (no contract change):
//! - a signature with formatting is the account's `Signature.text` with a small mark set: `**bold**`, `*italic*`, `[words](https://...)`
//!   (http, https or mailto only), line breaks as they are. A signature with none of these marks is plain and changes nothing.
//! - a picture inside the text is the marker `[picture:<attachment id>]` in the body, naming one of the draft's own `attachment_ids`
//!   whose file is an image. It goes out as an inline part (Content-ID) the HTML part references; the plain part reads `[picture: <name>]`.
//!   A marker naming anything else stays literal text; a file not named by a marker stays an attachment as before.
//!
//! The HTML is built here from the allowed tags only (`b`, `i`, `a href`, `br`, `img src=cid:`, `div`), every word escaped. No markup
//! the owner typed reaches the HTML as markup.
use crate::attachments::{part, OutgoingFile};
use crate::compose::SIGNATURE_DELIMITER;
use contracts::Signature;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};

/// The largest picture that goes inside the text (the ledger's INLINE_PICTURE_MAX_BYTES, docs/CONSTANTS.md); a larger one stays a file
/// attachment, as the composer already decides before it writes the marker (a second guard here: the message is built in memory).
pub const INLINE_PICTURE_MAX_BYTES: u64 = contracts::constants::INLINE_PICTURE_MAX_BYTES;
/// The marker of a picture in the text: `[picture:<attachment id>]`.
pub const PICTURE_MARK_OPEN: &str = "[picture:";
pub const PICTURE_MARK_CLOSE: char = ']';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mark {
    Plain,
    Bold,
    Italic,
    Link(String),
}

/// One stretch of a signature and its mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigRun {
    pub text: String,
    pub mark: Mark,
}

/// Only web and mail targets make a link; anything else (`javascript:`, `data:`) stays text.
pub fn safe_url(u: &str) -> bool {
    let l = u.trim().to_ascii_lowercase();
    (l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:")) && !u.chars().any(|c| c.is_whitespace() || c == '"' || c == '<' || c == '>')
}

fn find(c: &[char], from: usize, pat: &[char]) -> Option<usize> {
    if pat.is_empty() || from > c.len() {
        return None;
    }
    (from..c.len().saturating_sub(pat.len() - 1)).find(|&i| c[i..i + pat.len()] == *pat)
}

fn flush(plain: &mut String, runs: &mut Vec<SigRun>) {
    if !plain.is_empty() {
        runs.push(SigRun { text: std::mem::take(plain), mark: Mark::Plain });
    }
}

fn no_break(c: &[char]) -> bool {
    !c.is_empty() && !c.contains(&'\n')
}

/// The signature read as runs. Marks never cross a line break; an unclosed mark is plain text.
pub fn signature_runs(text: &str) -> Vec<SigRun> {
    let c: Vec<char> = text.chars().collect();
    let mut runs: Vec<SigRun> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '*' && c.get(i + 1) == Some(&'*') {
            if let Some(end) = find(&c, i + 2, &['*', '*']) {
                if no_break(&c[i + 2..end]) {
                    flush(&mut plain, &mut runs);
                    runs.push(SigRun { text: c[i + 2..end].iter().collect(), mark: Mark::Bold });
                    i = end + 2;
                    continue;
                }
            }
        } else if c[i] == '*' {
            if let Some(end) = find(&c, i + 1, &['*']) {
                if no_break(&c[i + 1..end]) {
                    flush(&mut plain, &mut runs);
                    runs.push(SigRun { text: c[i + 1..end].iter().collect(), mark: Mark::Italic });
                    i = end + 1;
                    continue;
                }
            }
        } else if c[i] == '[' {
            if let Some(close) = find(&c, i + 1, &[']']) {
                if c.get(close + 1) == Some(&'(') {
                    if let Some(end) = find(&c, close + 2, &[')']) {
                        let url: String = c[close + 2..end].iter().collect();
                        if no_break(&c[i + 1..close]) && safe_url(&url) {
                            flush(&mut plain, &mut runs);
                            runs.push(SigRun { text: c[i + 1..close].iter().collect(), mark: Mark::Link(url.trim().to_string()) });
                            i = end + 1;
                            continue;
                        }
                    }
                }
            }
        }
        plain.push(c[i]);
        i += 1;
    }
    flush(&mut plain, &mut runs);
    runs
}

/// True when the signature carries at least one mark (bold, italic or a link).
pub fn signature_is_rich(text: &str) -> bool {
    signature_runs(text).iter().any(|r| r.mark != Mark::Plain)
}

/// HTML text: `& < > " '` escaped, line breaks as `<br>`.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            '\n' => o.push_str("<br>"),
            '\r' => {}
            _ => o.push(ch),
        }
    }
    o
}

/// The signature's words without marks (a link keeps its address after its words).
pub fn signature_plain(runs: &[SigRun]) -> String {
    runs.iter()
        .map(|r| match &r.mark {
            Mark::Link(u) if *u != r.text => format!("{} <{}>", r.text, u),
            _ => r.text.clone(),
        })
        .collect()
}

pub fn signature_html(runs: &[SigRun]) -> String {
    runs.iter()
        .map(|r| match &r.mark {
            Mark::Plain => escape(&r.text),
            Mark::Bold => format!("<b>{}</b>", escape(&r.text)),
            Mark::Italic => format!("<i>{}</i>", escape(&r.text)),
            Mark::Link(u) => format!("<a href=\"{}\">{}</a>", escape(u), escape(&r.text)),
        })
        .collect()
}

/// One picture of the text: the attachment it names, its Content-ID and its file.
struct Inline<'a> {
    id: &'a str,
    cid: String,
    file: &'a OutgoingFile,
}

/// The ids the body names in a picture marker, in order of first appearance.
pub fn marked_ids(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find(PICTURE_MARK_OPEN) {
        let after = &rest[at + PICTURE_MARK_OPEN.len()..];
        match after.find(PICTURE_MARK_CLOSE) {
            Some(end) if end > 0 && !after[..end].contains('\n') => {
                let id = after[..end].to_string();
                if !out.contains(&id) {
                    out.push(id);
                }
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
    out
}

fn cid_for(id: &str, domain: &str) -> String {
    let local: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || "-._".contains(*c)).collect();
    format!("pic-{local}@{domain}")
}

/// The body with each known marker replaced: `html` gives `<img src="cid:...">`, else the plain fallback line.
fn render_body(body: &str, inline: &[Inline], html: bool) -> String {
    let mut out = String::new();
    let mut rest = body;
    let lit = |s: &str| if html { escape(s) } else { s.to_string() };
    while let Some(at) = rest.find(PICTURE_MARK_OPEN) {
        let after = &rest[at + PICTURE_MARK_OPEN.len()..];
        let hit = after.find(PICTURE_MARK_CLOSE).and_then(|end| inline.iter().find(|p| p.id == &after[..end]).map(|p| (end, p)));
        match hit {
            Some((end, p)) => {
                out.push_str(&lit(&rest[..at]));
                if html {
                    out.push_str(&format!("<img src=\"cid:{}\" alt=\"{}\">", escape(&p.cid), escape(&p.file.file_name)));
                } else {
                    out.push_str(&format!("[picture: {}]", p.file.file_name));
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&lit(&rest[..at + PICTURE_MARK_OPEN.len()]));
                rest = after;
            }
        }
    }
    out.push_str(&lit(rest));
    out
}

/// The body without the signature the composer appended to it, and what the composer put after the signature (the footer line; "" when
/// nothing): the rule is `compose::split_signature`. A body without the signature is returned whole with no tail.
fn strip_signature<'a>(body: &'a str, sig: &str) -> (&'a str, &'a str) {
    match crate::compose::split_signature(body, sig) {
        Some(parts) => parts,
        None => (body.trim_end(), ""),
    }
}

/// The two texts of a rich message and the pictures the HTML references (attachment id, Content-ID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichTexts {
    pub plain: String,
    pub html: String,
    pub inline: Vec<(String, String)>,
}

fn inline_of<'a>(body: &str, attachment_ids: &'a [String], files: &'a [OutgoingFile], domain: &str) -> Vec<Inline<'a>> {
    let marked = marked_ids(body);
    attachment_ids
        .iter()
        .zip(files.iter())
        .filter(|(id, f)| marked.contains(*id) && f.media_type.trim().to_ascii_lowercase().starts_with("image/") && f.bytes.len() as u64 <= INLINE_PICTURE_MAX_BYTES)
        .map(|(id, f)| Inline { id: id.as_str(), cid: cid_for(id, domain), file: f })
        .collect()
}

fn texts(body: &str, sig_text: &str, inline: &[Inline]) -> RichTexts {
    let (text, tail) = strip_signature(body, sig_text);
    let runs = signature_runs(sig_text);
    let mut plain = render_body(text, inline, false);
    let mut html = format!("<!DOCTYPE html><html><body><div>{}</div>", render_body(text, inline, true));
    if !sig_text.is_empty() {
        plain.push_str(&format!("\n\n{SIGNATURE_DELIMITER}{}", signature_plain(&runs)));
        html.push_str(&format!("<br><div>-- <br>{}</div>", signature_html(&runs)));
    }
    if !tail.is_empty() {
        // the footer the composer put after the signature stays after it
        plain.push_str(&format!("\n\n{}", render_body(tail, inline, false)));
        html.push_str(&format!("<br><div>{}</div>", render_body(tail, inline, true)));
    }
    html.push_str("</body></html>");
    RichTexts { plain, html, inline: inline.iter().map(|p| (p.id.to_string(), p.cid.clone())).collect() }
}

/// The plain and HTML texts of a draft, or None when it needs no HTML part (a plain signature and no picture in the text).
pub fn rich_texts(body: &str, signature: Option<&Signature>, attachment_ids: &[String], files: &[OutgoingFile], domain: &str) -> Option<RichTexts> {
    let sig_text = signature.map(|s| s.text.trim_end().to_string()).unwrap_or_default();
    let inline = inline_of(body, attachment_ids, files, domain);
    if !signature_is_rich(&sig_text) && inline.is_empty() {
        return None;
    }
    Some(texts(body, &sig_text, &inline))
}

/// The rich body of a draft, or None when the draft needs none (a plain signature and no picture in the text: the caller's plain path).
/// `files` are the draft's files in the order of `attachment_ids`; `domain` is the sending address's domain (the Content-IDs end with it).
pub fn rich_body(body: &str, signature: Option<&Signature>, attachment_ids: &[String], files: &[OutgoingFile], domain: &str) -> Option<MultiPart> {
    let sig_text = signature.map(|s| s.text.trim_end().to_string()).unwrap_or_default();
    let inline = inline_of(body, attachment_ids, files, domain);
    if !signature_is_rich(&sig_text) && inline.is_empty() {
        return None;
    }
    let RichTexts { plain, html, .. } = texts(body, &sig_text, &inline);
    let html_part = SinglePart::html(html);
    let alt = if inline.is_empty() {
        MultiPart::alternative().singlepart(SinglePart::plain(plain)).singlepart(html_part)
    } else {
        let mut related = MultiPart::related().singlepart(html_part);
        for p in &inline {
            let ct = ContentType::parse(p.file.media_type.trim()).unwrap_or_else(|_| ContentType::parse("application/octet-stream").expect("a fixed media type"));
            related = related.singlepart(Attachment::new_inline_with_name(p.cid.clone(), p.file.file_name.clone()).body(p.file.bytes.clone(), ct));
        }
        MultiPart::alternative().singlepart(SinglePart::plain(plain)).multipart(related)
    };
    let others: Vec<&OutgoingFile> =
        attachment_ids.iter().zip(files.iter()).filter(|(id, _)| !inline.iter().any(|p| p.id == id.as_str())).map(|(_, f)| f).collect();
    if others.is_empty() {
        return Some(alt);
    }
    let mut mixed = MultiPart::mixed().multipart(alt);
    for f in others {
        mixed = mixed.singlepart(part(f));
    }
    Some(mixed)
}
