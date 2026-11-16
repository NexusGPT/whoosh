//! Header bytes to the contract's Message record. Decoding and threading beyond this belong to
//! core-parse; the adapter keeps only what an inbox row needs at first paint.
use contracts::Message;
use mail_parser::MessageParser;

use crate::{port::{FetchedMessage, GmailIdentity}, roles::FolderRole};

pub(crate) struct Raw<'a> {
    pub account_id: &'a str,
    pub folder: &'a str,
    pub role: FolderRole,
    pub uid: u32,
    pub uid_validity: u32,
    pub modseq: Option<u64>,
    pub size: Option<u32>,
    pub flags: Vec<String>,
    pub internal_ms: i64,
    pub header: &'a [u8],
    pub text: &'a [u8],
    pub gmail: Option<GmailIdentity>,
}

/// Identity of a message: (account, folder, uidvalidity, uid). The UID alone means nothing: it is
/// per folder and dies with the folder's UIDVALIDITY.
pub(crate) fn message_id(account_id: &str, folder: &str, uid_validity: u32, uid: u32) -> String {
    format!("{account_id}:{folder}:{uid_validity}:{uid}")
}

/// Gmail shows one message under several label-folders with different UIDs; X-GM-MSGID is the same
/// in all of them, so on Gmail the id is built from it and the message is stored once.
pub(crate) fn gmail_message_id(account_id: &str, msg_id: u64) -> String {
    format!("{account_id}:gm:{msg_id:x}")
}

/// real-replyto: the Reply-To entries of a header block as (display name, address), header order; empty when the header is absent.
pub(crate) fn reply_to_named(block: &[u8]) -> Vec<(String, String)> {
    let Some(m) = MessageParser::default().parse_headers(block) else { return Vec::new() };
    named_list(m.reply_to())
}

fn named_list(a: Option<&mail_parser::Address<'_>>) -> Vec<(String, String)> {
    a.map(|a| a.iter().filter_map(|x| x.address().map(|ad| (x.name().unwrap_or_default().to_string(), ad.to_string()))).collect()).unwrap_or_default()
}

fn addr_list(a: Option<&mail_parser::Address<'_>>) -> Vec<String> {
    a.map(|a| a.iter().filter_map(|x| x.address().map(str::to_string)).collect()).unwrap_or_default()
}

/// The list row's preview: the header block and the first bytes of the body (`BODY.PEEK[TEXT]<0.n>`) parsed as one truncated
/// message by the parse crate (mail-parser underneath, lenient on an unterminated last part), so a multipart message shows its
/// text part decoded (quoted-printable, base64, html made text), never its boundary line, its part headers or a soft break.
/// The first `PREVIEW_CHARS` of the visible (unquoted) text, whitespace collapsed; empty when nothing readable is there yet.
#[cfg(test)]
pub(crate) fn preview(header: &[u8], text: &[u8]) -> String { head_of(header, text).0 }

/// junk-r3: the preview and the HTML the first body bytes hold (None when the slice reaches no HTML part), from ONE parse of the
/// header block plus the slice. The HTML is what the spam filter's link rule reads on an arrival: the body is not fetched yet.
pub(crate) fn head_of(header: &[u8], text: &[u8]) -> (String, Option<String>) {
    let mut raw = Vec::with_capacity(header.len() + text.len() + 2);
    raw.extend_from_slice(header);
    // a header block without its blank line (or none at all) would read the body's first lines as headers
    if !(header.ends_with(b"\r\n\r\n") || header.ends_with(b"\n\n")) {
        raw.extend_from_slice(if header.is_empty() || header.ends_with(b"\n") { b"\r\n" } else { b"\r\n\r\n" });
    }
    raw.extend_from_slice(text);
    match parse::parse_message(&raw) {
        Ok(m) => (visible_preview(&m.text), m.html.filter(|h| !h.trim().is_empty())),
        Err(_) => (String::new(), None),
    }
}

/// The first `PREVIEW_CHARS` of a body's visible text: the quote cut off (unless the whole text is quoted), whitespace collapsed,
/// and a quoted-printable escape the byte limit cut in half (`=`, `=A`) dropped from the end.
pub(crate) fn visible_preview(text: &str) -> String {
    let cut = parse::text_quote_start(text).filter(|&q| !text[..q].trim().is_empty()).unwrap_or(text.len());
    // the parse crate's own preview: a drawn line ("---", "_____") is no word; a text of nothing else falls back to the whole text
    let words = { let p = parse::preview_of(&text[..cut]); if p.is_empty() { parse::preview_or_raw(text) } else { p } };
    let tail = words.rfind('=').filter(|&i| words.len() - i <= 2 && words[i + 1..].chars().all(|c| c.is_ascii_hexdigit()));
    match tail { Some(i) => words[..i].trim_end().to_string(), None => words }
}

pub(crate) fn build(r: Raw<'_>) -> FetchedMessage {
    let parsed = MessageParser::default().parse_headers(r.header);
    let id = match &r.gmail {
        Some(g) => gmail_message_id(r.account_id, g.msg_id),
        None => message_id(r.account_id, r.folder, r.uid_validity, r.uid),
    };
    let (sender_name, from_address, to, subject, mid, thread_root) = match &parsed {
        Some(m) => {
            let first = m.from().and_then(|a| a.first());
            let mid = m.message_id().map(str::to_string);
            let root = m.references().as_text_list().and_then(|l| l.first().map(|s| s.to_string()))
                .or_else(|| m.in_reply_to().as_text().map(str::to_string))
                .or_else(|| mid.clone());
            (
                first.and_then(|f| f.name()).unwrap_or_default().to_string(),
                first.and_then(|f| f.address()).unwrap_or_default().to_string(),
                addr_list(m.to()),
                m.subject().unwrap_or_default().to_string(),
                mid,
                root,
            )
        }
        None => Default::default(),
    };
    let read = r.flags.iter().any(|f| f.eq_ignore_ascii_case("\\Seen"));
    let (preview, html_head) = head_of(r.header, r.text);
    // contracts-d11: the List-Unsubscribe header and RFC 8058's one-click offer, read by the contract's one rule
    let (list_unsubscribe, list_unsubscribe_one_click) = parsed
        .as_ref()
        .map(|m| contracts::list_unsubscribe_from_headers(m.header_raw("List-Unsubscribe"), m.header_raw("List-Unsubscribe-Post")))
        .unwrap_or_default();
    // contracts delta 12: Cc and Bcc as the headers name them (a Bcc header is there only on the owner's own sent copies)
    let (cc, bcc) = parsed.as_ref().map(|m| (addr_list(m.cc()), addr_list(m.bcc()))).unwrap_or_default();
    let message = Message { list_unsubscribe, list_unsubscribe_one_click, cc, bcc, is_outgoing: false,
        id: id.clone(),
        thread_id: thread_root.map(|t| format!("{}:{t}", r.account_id)).unwrap_or_else(|| id.clone()),
        account_id: r.account_id.to_string(),
        sender_name,
        from_address,
        to,
        subject,
        preview,
        received_at_ms: r.internal_ms,
        read,
        archived: r.role == FolderRole::Archive,
        folder: r.folder.to_string(),
        attachment_names: Vec::new(),
        login_code: None,
        trackers_blocked: 0,
        reply_to_name: None,
    };
    // junk-r2: the two headers the spam filter reads and the contract's Message does not carry
    let reply_to = parsed.as_ref().map(|m| addr_list(m.reply_to()).into_iter().map(|a| a.to_ascii_lowercase()).collect()).unwrap_or_default();
    let authentication_results = header_values(r.header, "authentication-results");
    FetchedMessage { uid: r.uid, uid_validity: r.uid_validity, modseq: r.modseq, flags: r.flags, message_id_header: mid, size: r.size, gmail: r.gmail, message,
        authentication_results, reply_to, reply_to_named: parsed.as_ref().map(|m| named_list(m.reply_to())).unwrap_or_default(), html_head, references: Vec::new() } // A144: `adapter::from_fetch` fills the chain
}

/// Every value of one header in a raw header block, in the order the block holds them (topmost first), each unfolded (a line that
/// starts with a space or a tab continues the one before) and trimmed. The name matches in any case. Stops at the first blank line.
pub(crate) fn header_values(block: &[u8], name: &str) -> Vec<String> {
    let text = String::from_utf8_lossy(block);
    let mut out: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for raw in text.split('\n') {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() { break; }
        if line.starts_with(' ') || line.starts_with('\t') {
            if let Some(v) = current.as_mut() { v.push(' '); v.push_str(line.trim()); }
            continue;
        }
        if let Some(v) = current.take() { out.push(v); }
        if let Some((n, v)) = line.split_once(':') {
            if n.trim().eq_ignore_ascii_case(name) { current = Some(v.trim().to_string()); }
        }
    }
    if let Some(v) = current { out.push(v); }
    out
}

#[cfg(test)]
mod d11_tests {
    use super::*;

    fn raw(header: &'static [u8]) -> Raw<'static> {
        Raw { account_id: "acc", folder: "INBOX", role: FolderRole::Inbox, uid: 1, uid_validity: 1, modseq: None, size: None, flags: vec![],
              internal_ms: 1, header, text: b"", gmail: None }
    }

    #[test]
    fn the_list_unsubscribe_header_reaches_the_message() {
        let f = build(raw(b"From: News <n@l.example>\r\nSubject: s\r\nList-Unsubscribe: <mailto:leave@l.example>,\r\n <https://l.example/u/1>\r\nList-Unsubscribe-Post: List-Unsubscribe=One-Click\r\n\r\n"));
        assert_eq!(f.message.list_unsubscribe, vec!["<mailto:leave@l.example>".to_string(), "<https://l.example/u/1>".into()]);
        assert!(f.message.list_unsubscribe_one_click);
        let plain = build(raw(b"From: A <a@x.example>\r\nSubject: s\r\n\r\n"));
        assert!(plain.message.list_unsubscribe.is_empty() && !plain.message.list_unsubscribe_one_click);
    }

    #[test]
    fn the_authentication_results_and_reply_to_reach_the_fetched_message() {
        let f = build(raw(b"Authentication-Results: mx.owner.example; spf=fail smtp.mailfrom=bank.example;\r\n dkim=fail header.d=bank.example; dmarc=fail\r\nFrom: \"Bank\" <alerts@bank-secure.example>\r\nReply-To: Collect <COLLECT@Elsewhere.example>, two@x.example\r\nAuthentication-Results: forged.example; spf=pass\r\nSubject: s\r\n\r\n"));
        assert_eq!(f.authentication_results, vec!["mx.owner.example; spf=fail smtp.mailfrom=bank.example; dkim=fail header.d=bank.example; dmarc=fail".to_string(), "forged.example; spf=pass".into()], "topmost first, unfolded");
        assert_eq!(f.reply_to, vec!["collect@elsewhere.example".to_string(), "two@x.example".into()], "lower-case addresses");
        let plain = build(raw(b"From: A <a@x.example>\r\nSubject: s\r\n\r\n"));
        assert!(plain.authentication_results.is_empty() && plain.reply_to.is_empty(), "absent headers give nothing");
    }

    #[test]
    fn cc_and_bcc_reach_the_message_in_header_order() {
        let f = build(raw(b"From: Me <me@x.example>\r\nTo: a@x.example\r\nCc: Tom <tom@y.example>, sara@z.example\r\nBcc: boss@x.example\r\nSubject: s\r\n\r\n"));
        assert_eq!(f.message.cc, vec!["tom@y.example".to_string(), "sara@z.example".into()]);
        assert_eq!(f.message.bcc, vec!["boss@x.example".to_string()]);
        let plain = build(raw(b"From: A <a@x.example>\r\nSubject: s\r\n\r\n"));
        assert!(plain.message.cc.is_empty() && plain.message.bcc.is_empty());
    }
}

#[cfg(test)]
mod body_r1_tests {
    use super::*;

    /// The owner's sweetgreen message as the sync fetched it on main56: the header block names a multipart body; the first body
    /// bytes start with the boundary, the part headers, then quoted-printable text with a soft break inside "almonds".
    const SWEETGREEN_HEADER: &[u8] = b"From: sweetgreen <hello@sweetgreen.example>\r\nTo: owner@example.test\r\nSubject: Omega Glow Bowl\r\nMIME-Version: 1.0\r\nContent-Type: multipart/alternative; boundary=\"--d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04\"\r\n\r\n";
    const SWEETGREEN_TEXT: &[u8] = b"----d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\nThe Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almo=\r\nnds over warm quinoa. Order now and pick it up on your way h=\r\nome.\r\n\r\n----d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/html; charset=us-ascii\r\n\r\n<html><body><p>The Omega Glow Bowl: miso glazed salmon, sweet co=\r\nrn salsa";

    fn assert_clean(p: &str) {
        assert!(!p.contains("--"), "no boundary line: {p:?}");
        assert!(!p.contains("Content-"), "no part header: {p:?}");
        assert!(!p.contains('='), "no soft break: {p:?}");
    }

    #[test]
    fn a_multipart_preview_is_its_decoded_text_part() {
        let p = preview(SWEETGREEN_HEADER, SWEETGREEN_TEXT);
        assert!(p.starts_with("The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almonds"), "{p:?}");
        assert_clean(&p);
        assert!(p.chars().count() <= parse::tuning::PREVIEW_CHARS);
    }

    #[test]
    fn a_preview_cut_inside_the_text_part_keeps_only_whole_text() {
        // the 2048-byte limit can end in the middle of the text part, even inside a quoted-printable escape
        let cut = SWEETGREEN_TEXT.windows(5).position(|w| w == b"almo=").unwrap() + 5;
        let p = preview(SWEETGREEN_HEADER, &SWEETGREEN_TEXT[..cut]);
        assert!(p.starts_with("The Omega Glow Bowl: miso glazed salmon"), "{p:?}");
        assert_clean(&p);
    }

    #[test]
    fn an_html_only_message_previews_its_visible_words_and_plain_text_stays_as_it_is() {
        let html = preview(b"Content-Type: text/html; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\n", b"PGh0bWw+PGJvZHk+PHA+SGVsbG8gPGI+dGhlcmU8L2I+LCBvd25lci48L3A+PC9ib2R5PjwvaHRtbD4=\r\n");
        assert_eq!(html, "Hello there, owner.");
        assert_eq!(preview(b"Subject: s\r\n\r\n", b"Just a line\r\nand another.\r\n"), "Just a line and another.");
        assert_eq!(preview(b"", b"No header at all."), "No header at all.");
    }

    #[test]
    fn the_fetched_message_carries_the_decoded_preview() {
        let f = build(Raw { account_id: "acc", folder: "INBOX", role: FolderRole::Inbox, uid: 143411, uid_validity: 1, modseq: None, size: None, flags: vec![],
            internal_ms: 1, header: SWEETGREEN_HEADER, text: SWEETGREEN_TEXT, gmail: None });
        assert!(f.message.preview.starts_with("The Omega Glow Bowl"), "{:?}", f.message.preview);
        assert_clean(&f.message.preview);
    }
}
