//! One Outlook for Mac message XML (`message_NNNNN.xml`, an `emails` root holding one `email`) to the RFC 822 message the rest of
//! the import reads. The element and attribute names are the ones Outlook for Mac writes (listed in docs/lanes/core-import.md).
//! Attachments are not copied into the message: they stay entries of the archive and travel as references.
use crate::model::{AttachmentRef, MailFlags};
use crate::timeutil::{parse_iso_naive, rfc2822};

pub struct Built {
    pub bytes: Vec<u8>,
    pub flags: MailFlags,
    pub received_ms: Option<i64>,
    pub attachments: Vec<AttachmentRef>,
}

const BOUNDARY: &str = "whoosh-import-boundary";

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, tag: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.has_tag_name(tag))
}

fn text_of(n: roxmltree::Node<'_, '_>, tag: &str) -> String {
    child(n, tag).and_then(|c| c.text()).unwrap_or("").to_string()
}

fn addresses(email: roxmltree::Node<'_, '_>, tag: &str) -> Vec<(String, String)> {
    let Some(list) = child(email, tag) else { return Vec::new() };
    list.descendants()
        .filter(|d| d.is_element() && d.has_tag_name("emailAddress"))
        .filter_map(|d| {
            let address = d.attribute("OPFContactEmailAddressAddress")?.trim();
            (!address.is_empty()).then(|| (d.attribute("OPFContactEmailAddressName").unwrap_or("").trim().to_string(), address.to_string()))
        })
        .collect()
}

/// A header value never carries a line break.
fn clean(s: &str) -> String {
    s.replace(['\r', '\n'], " ").trim().to_string()
}

/// RFC 2047 encoded word, for a header value that is not plain ASCII.
fn encoded_word(s: &str) -> String {
    format!("=?UTF-8?B?{}?=", base64(s.as_bytes()))
}

fn header_text(s: &str) -> String {
    let s = clean(s);
    if s.is_ascii() {
        s
    } else {
        encoded_word(&s)
    }
}

fn address_header(list: &[(String, String)]) -> String {
    list.iter()
        .map(|(name, address)| {
            let (name, address) = (clean(name), clean(address));
            if name.is_empty() || name == address {
                address
            } else if name.is_ascii() {
                format!("\"{}\" <{}>", name.replace('\\', "\\\\").replace('"', "\\\""), address)
            } else {
                format!("{} <{}>", encoded_word(&name), address)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn id_tokens(raw: &str) -> Vec<String> {
    raw.split_whitespace().map(|t| t.trim_matches(['<', '>', ',']).to_string()).filter(|t| !t.is_empty()).map(|t| format!("<{t}>")).collect()
}

pub fn build(xml: &str) -> Result<Built, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("is not readable XML: {e}"))?;
    let email = doc.descendants().find(|n| n.is_element() && n.has_tag_name("email")).ok_or("holds no email element")?;

    let sent_ms = parse_iso_naive(&text_of(email, "OPFMessageCopySentTime"));
    let received_ms = parse_iso_naive(&text_of(email, "OPFMessageCopyReceivedTime")).or(sent_ms);
    let text = text_of(email, "OPFMessageCopyBody");
    let html = text_of(email, "OPFMessageCopyHTMLBody");

    let mut head = String::new();
    for (name, tag) in [("From", "OPFMessageCopyFromAddresses"), ("To", "OPFMessageCopyToAddresses"), ("Cc", "OPFMessageCopyCCAddresses"), ("Bcc", "OPFMessageCopyBCCAddresses")] {
        let value = address_header(&addresses(email, tag));
        if !value.is_empty() {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
    }
    head.push_str(&format!("Subject: {}\r\n", header_text(&text_of(email, "OPFMessageCopySubject"))));
    if let Some(ms) = sent_ms {
        head.push_str(&format!("Date: {}\r\n", rfc2822(ms)));
    }
    for (name, tag) in [("Message-ID", "OPFMessageCopyMessageID"), ("In-Reply-To", "OPFMessageCopyInReplyTo"), ("References", "OPFMessageCopyReferences")] {
        let ids = id_tokens(&text_of(email, tag));
        if !ids.is_empty() {
            head.push_str(&format!("{name}: {}\r\n", if name == "Message-ID" { ids[0].clone() } else { ids.join(" ") }));
        }
    }
    head.push_str("MIME-Version: 1.0\r\n");
    let utf8 = "Content-Transfer-Encoding: 8bit\r\n";
    let body = if !html.is_empty() && !text.is_empty() {
        let mut boundary = BOUNDARY.to_string();
        while text.contains(&boundary) || html.contains(&boundary) {
            boundary.push('x');
        }
        format!(
            "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n\r\n--{boundary}\r\nContent-Type: text/plain; charset=utf-8\r\n{utf8}\r\n{text}\r\n--{boundary}\r\nContent-Type: text/html; charset=utf-8\r\n{utf8}\r\n{html}\r\n--{boundary}--\r\n"
        )
    } else if !html.is_empty() {
        format!("Content-Type: text/html; charset=utf-8\r\n{utf8}\r\n{html}\r\n")
    } else {
        format!("Content-Type: text/plain; charset=utf-8\r\n{utf8}\r\n{text}\r\n")
    };
    head.push_str(&body);

    let is_read = match child(email, "OPFMessageGetIsRead").and_then(|c| c.text()).map(str::trim) {
        Some("1") | Some("true") | None => true,
        Some(_) => false,
    };
    let attachments = child(email, "OPFMessageCopyAttachmentList")
        .map(|list| {
            list.descendants()
                .filter(|d| d.is_element() && d.has_tag_name("messageAttachment"))
                .map(|d| AttachmentRef {
                    name: d.attribute("OPFAttachmentName").unwrap_or("attachment").to_string(),
                    content_type: d.attribute("OPFAttachmentContentType").filter(|t| !t.is_empty()).unwrap_or("application/octet-stream").to_string(),
                    size: 0,
                    content_id: d.attribute("OPFAttachmentContentID").filter(|c| !c.is_empty()).map(str::to_string),
                    location: d.attribute("OPFAttachmentURL").unwrap_or("").to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Built { bytes: head.into_bytes(), flags: MailFlags { read: is_read, ..MailFlags::default() }, received_ms, attachments })
}

/// Standard base64 with padding (RFC 4648).
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}
