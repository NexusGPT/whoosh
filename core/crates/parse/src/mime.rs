//! MIME and headers over `mail-parser`.
use mail_parser::{MessageParser, MessagePart, PartKind};

use crate::sanitise::{safe_html_to_text, sanitise_html};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The bytes held no header block, so there is no message to read.
    NotAMessage,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NotAMessage => write!(f, "these bytes are not an email message"),
        }
    }
}

impl std::error::Error for ParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub name: Option<String>,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub name: Option<String>,
    pub content_type: String,
    /// The decoded size in bytes: what the file weighs once saved.
    pub size: usize,
    pub content_id: Option<String>,
    pub inline: bool,
    /// body-r3: where the part sits in the message, as IMAP numbers it (`BODY.PEEK[2]`, `BODY.PEEK[1.2]`): the root's children are 1, 2, 3
    /// (a message that is not multipart has one part, `1`), a multipart's children are `<its number>.1`, `<its number>.2`.
    pub part: String,
    /// body-r3: the part's Content-Transfer-Encoding as `decode_transfer` reads it: `base64`, `quoted-printable` or `none`.
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMessage {
    pub message_id: Option<String>,
    pub in_reply_to: Vec<String>,
    pub references: Vec<String>,
    pub subject: String,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    /// contracts delta 12: present only on the owner's own sent copies (a delivered message carries no Bcc header)
    pub bcc: Vec<Address>,
    pub date_ms: Option<i64>,
    /// URLs of the List-Unsubscribe header.
    pub list_unsubscribe: Vec<String>,
    /// The plain-text body; made from the HTML when the message has no text part.
    pub text: String,
    /// The HTML body, untouched. Only real `text/html` parts; never made from text.
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
    /// Raw iCalendar texts of every calendar part (`text/calendar`, `application/ics`, `.ics`).
    pub calendar_parts: Vec<String>,
}

/// body-r3: the IMAP section number of every part of the message (the number `BODY.PEEK[<section>]` takes), by the part's id.
fn section_numbers(msg: &mail_parser::Message) -> std::collections::HashMap<mail_parser::PartId, String> {
    fn walk(part: MessagePart<'_>, path: String, out: &mut std::collections::HashMap<mail_parser::PartId, String>) {
        out.insert(part.id(), path.clone());
        for (i, child) in part.children().enumerate() {
            walk(child, if path.is_empty() { (i + 1).to_string() } else { format!("{path}.{}", i + 1) }, out);
        }
    }
    let mut out = std::collections::HashMap::new();
    let root = msg.root_part();
    let start = if root.is_multipart() { String::new() } else { "1".to_string() };
    walk(root, start, &mut out);
    out
}

/// body-r3: the attachments of a parsed message in the order mail-parser lists them, each with its part (still borrowed from the message).
fn attachment_list<'a>(msg: &'a mail_parser::Message) -> Vec<(Attachment, MessagePart<'a>)> {
    let sections = section_numbers(msg);
    msg.attachments()
        .map(|p| {
            let a = Attachment {
                // fix-realcore round 7: a name sent decomposed (macOS file names: "e" + U+0301) is kept composed, as the tiles, the gallery
                // and a forward compare it
                name: p.attachment_name().map(crate::nfc::nfc),
                content_type: match p.content_type() {
                    Some(ct) => format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or("")),
                    None => "application/octet-stream".to_string(),
                },
                size: p.decoded_len(),
                content_id: p.content_id().map(str::to_string),
                inline: p.is_inline() || is_invitation_data(&p),
                part: sections.get(&p.id()).cloned().unwrap_or_default(),
                // the parser's own numbering (0 none, 1 quoted-printable, 2 base64: "stable" in its docs)
                encoding: match p.encoding() as u8 { 2 => "base64", 1 => "quoted-printable", _ => "none" }.to_string(),
            };
            (a, p)
        })
        .collect()
}

/// body-r3: the attachments of one raw message (names decoded per RFC 2231 and 2047 by the parser, sizes decoded), with no HTML sanitising:
/// cheap enough to run for every body that is fetched. The same list, in the same order, as `parse_message(raw).attachments`.
pub fn attachments_of(raw: &[u8]) -> Result<Vec<Attachment>, ParseError> {
    let msg = MessageParser::new().parse(raw).ok_or(ParseError::NotAMessage)?;
    Ok(attachment_list(&msg).into_iter().map(|(a, _)| a).collect())
}

/// body-r3: `attachments_of` with each part's decoded bytes (a 10 MB mail holds about 10 MB of them: the caller keeps what it wants).
pub fn attachments_with_bytes(raw: &[u8]) -> Result<Vec<(Attachment, Vec<u8>)>, ParseError> {
    let msg = MessageParser::new().parse(raw).ok_or(ParseError::NotAMessage)?;
    Ok(attachment_list(&msg).into_iter().map(|(a, p)| (a, p.decoded().into_owned())).collect())
}

/// body-r3: the bytes of a part fetched by its number (`BODY.PEEK[<part>]`: the body still in its transfer encoding) decoded as the
/// message says (`Attachment::encoding`).
pub fn decode_transfer(encoding: &str, body: &[u8]) -> Vec<u8> {
    if encoding != "base64" && encoding != "quoted-printable" { return body.to_vec(); }
    // the parser's own decoder, reached through a one-part message that declares the encoding
    let mut raw = format!("Content-Type: application/octet-stream\r\nContent-Transfer-Encoding: {encoding}\r\n\r\n").into_bytes();
    raw.extend_from_slice(body);
    match MessageParser::new().parse(&raw) { Some(m) => m.root_part().decoded().into_owned(), None => body.to_vec() }
}

/// The calendar data of a meeting invitation: a `text/calendar` (or `application/ics`) part the sender gave no file name and did not declare
/// an attachment. It is another form of the message itself (Mail and Outlook draw it as the invitation), never a file the sender attached,
/// so it is listed with `inline` set: no tile, no paperclip, no "has attachment" (owner's picture 212, 13 Nov 2026: "fantom attachment",
/// a tile "1 attachment" on a forwarded invitation). An .ics the sender named or attached stays a file.
fn is_invitation_data(p: &MessagePart<'_>) -> bool {
    (p.is_content_type("text", "calendar") || p.is_content_type("application", "ics"))
        && p.attachment_name().is_none_or(|n| n.trim().is_empty())
        && !p.content_disposition().is_some_and(|d| d.ctype().eq_ignore_ascii_case("attachment"))
}

fn is_calendar(p: &MessagePart<'_>) -> bool {
    p.is_content_type("text", "calendar")
        || p.is_content_type("application", "ics")
        || p.attachment_name().is_some_and(|n| n.to_ascii_lowercase().ends_with(".ics"))
}

fn addresses(list: Option<mail_parser::AddressList<'_>>) -> Vec<Address> {
    list.map(|l| {
        l.mailboxes()
            .filter_map(|m| m.address().map(|a| Address { name: m.name().map(str::to_string), email: a.to_string() }))
            .collect()
    })
    .unwrap_or_default()
}

fn texts(list: Option<mail_parser::TextList<'_>>) -> Vec<String> {
    list.map(|l| l.iter().map(str::to_string).collect()).unwrap_or_default()
}

pub fn parse_message(bytes: &[u8]) -> Result<ParsedMessage, ParseError> {
    let msg = MessageParser::new().parse(bytes).ok_or(ParseError::NotAMessage)?;

    let mut text_parts: Vec<String> = Vec::new();
    // owner's picture 216 (14 Nov 2026): a text part of page code is read as its words, and is the page of a mail that has none
    let mut code_pages: Vec<String> = Vec::new();
    for p in msg.text_body().filter(|p| !is_calendar(p)) {
        match p.kind() {
            PartKind::Text => text_parts.extend(p.text().map(|t| {
                if !crate::pagecode::text_is_page_code(&t) { return crate::entities::plain_text(&t); }
                let page = crate::pagecode::page_code_as_page(&t);
                let words = crate::pagecode::page_code_words(&page);
                code_pages.push(page);
                words
            })),
            PartKind::Html => text_parts.extend(p.text().map(|h| safe_html_to_text(&sanitise_html(&h).html))),
            _ => {}
        }
    }
    let html_parts: Vec<String> =
        msg.html_body().filter(|p| matches!(p.kind(), PartKind::Html)).filter_map(|p| p.text().map(|t| crate::pagecode::close_cut_tags(&t).into_owned())).collect();
    let html_parts = if html_parts.is_empty() { code_pages } else { html_parts };

    let attachments = attachment_list(&msg).into_iter().map(|(a, _)| a).collect();

    let calendar_parts = msg
        .parts()
        .filter(|p| !p.is_multipart() && !p.is_message() && is_calendar(p))
        .map(|p| String::from_utf8_lossy(&p.decoded()).into_owned())
        .collect();

    Ok(ParsedMessage {
        message_id: msg.message_id().map(str::to_string),
        in_reply_to: texts(msg.in_reply_to()),
        references: texts(msg.references()),
        subject: msg.subject().unwrap_or("").to_string(),
        from: addresses(msg.from()).into_iter().next(),
        to: addresses(msg.to()),
        cc: addresses(msg.cc()),
        bcc: addresses(msg.bcc()),
        date_ms: msg.date().map(|d| d.to_timestamp() * 1000),
        list_unsubscribe: msg
            .list_unsubscribe()
            .map(|l| l.mailboxes().filter_map(|m| m.address().map(str::to_string)).collect())
            .unwrap_or_default(),
        text: text_parts.join("\n"),
        html: if html_parts.is_empty() { None } else { Some(html_parts.join("\n")) },
        attachments,
        calendar_parts,
    })
}
