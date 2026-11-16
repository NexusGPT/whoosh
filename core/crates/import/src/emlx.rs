//! One Apple Mail message file. Layout (see docs/lanes/core-import.md for the sources): a first line with the byte count of the
//! message, then exactly that many bytes of RFC 822 message, then an XML property list with the state Mail keeps (`flags`,
//! `date-received`). A `.partial.emlx` is the same layout for a message whose attachments Mail kept as separate files under a
//! sibling `Attachments/<message number>/<part>/` folder.
use std::path::Path;

use crate::model::{AttachmentRef, MailFlags};

#[derive(Debug)]
pub struct Emlx {
    pub message: Vec<u8>,
    pub flags: MailFlags,
    pub received_ms: Option<i64>,
}

// The `flags` bit mask Mail keeps in the plist (bit 0 value 1 is Read, 2 Deleted, 4 Answered, 16 Flagged, 64 Draft).
const FLAG_READ: u64 = 1;
const FLAG_DELETED: u64 = 2;
const FLAG_ANSWERED: u64 = 4;
const FLAG_FLAGGED: u64 = 16;
const FLAG_DRAFT: u64 = 64;

/// Split the file into the message and its plist. The byte count decides where the message ends. The message keeps the file's
/// own buffer (one copy of the message in memory, not two).
pub fn parse_emlx(mut data: Vec<u8>) -> Result<Emlx, String> {
    let nl = data.iter().position(|b| *b == b'\n').ok_or("the file has no first line with a byte count")?;
    let count_text = String::from_utf8_lossy(&data[..nl]).trim().to_string();
    let count: usize = count_text.parse().map_err(|_| format!("the first line is {count_text:?}, not a byte count"))?;
    let (start, held) = (nl + 1, data.len() - (nl + 1));
    if held < count {
        return Err(format!("the first line says {count} bytes of message but the file holds {held}"));
    }
    let (flags, received_ms) = read_plist(&data[start + count..]);
    data.truncate(start + count);
    data.drain(..start);
    Ok(Emlx { message: data, flags, received_ms })
}

/// The flags and the arrival time out of the trailing plist. A missing or unreadable plist leaves the defaults (read, no date).
fn read_plist(trailer: &[u8]) -> (MailFlags, Option<i64>) {
    let mut flags = MailFlags::default();
    let text = String::from_utf8_lossy(trailer);
    let start = match text.find("<?xml").or_else(|| text.find("<plist")) {
        Some(i) => i,
        None => return (flags, None),
    };
    // a plist opens with a DOCTYPE line, which the reader refuses unless told otherwise
    let mut opts = roxmltree::ParsingOptions::default();
    opts.allow_dtd = true;
    let doc = match roxmltree::Document::parse_with_options(&text[start..], opts) {
        Ok(d) => d,
        Err(_) => return (flags, None),
    };
    let dict = match doc.descendants().find(|n| n.has_tag_name("dict")) {
        Some(d) => d,
        None => return (flags, None),
    };
    let mut received_ms = None;
    let mut pending_key: Option<String> = None;
    for node in dict.children().filter(|n| n.is_element()) {
        if node.has_tag_name("key") {
            pending_key = node.text().map(|t| t.trim().to_string());
            continue;
        }
        let value = node.text().unwrap_or("").trim();
        match pending_key.take().as_deref() {
            Some("flags") => {
                if let Ok(bits) = value.parse::<u64>() {
                    flags = MailFlags {
                        read: bits & FLAG_READ != 0,
                        deleted: bits & FLAG_DELETED != 0,
                        answered: bits & FLAG_ANSWERED != 0,
                        flagged: bits & FLAG_FLAGGED != 0,
                        draft: bits & FLAG_DRAFT != 0,
                    };
                }
            }
            Some("date-received") => {
                received_ms = value.parse::<f64>().ok().map(|s| (s * 1000.0) as i64).filter(|ms| *ms > 0);
            }
            _ => {}
        }
    }
    (flags, received_ms)
}

/// The message number a file name carries: "12345.emlx" and "12345.partial.emlx" are both 12345.
pub fn message_number(file_name: &str) -> &str {
    file_name.strip_suffix(".partial.emlx").or_else(|| file_name.strip_suffix(".emlx")).unwrap_or(file_name)
}

/// The attachment files a partial message left beside it: `<Messages dir>/../Attachments/<number>/<part>/<file>`.
/// Read from the folder listing only; no attachment is opened.
pub fn partial_attachments(messages_dir: &Path, number: &str) -> Vec<AttachmentRef> {
    let mut out = Vec::new();
    let Some(data_dir) = messages_dir.parent() else { return out };
    collect_files(&data_dir.join("Attachments").join(number), 0, &mut out);
    out.sort_by(|a, b| a.location.cmp(&b.location));
    out
}

/// Part folders are named by position ("1.2", "2"), so a file sits one or two folders down.
const ATTACHMENT_DEPTH: usize = 3;

fn collect_files(dir: &Path, depth: usize, out: &mut Vec<AttachmentRef>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            if depth < ATTACHMENT_DEPTH {
                collect_files(&path, depth + 1, out);
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            out.push(AttachmentRef { name: name.to_string(), content_type: guess_type(name).to_string(), size: meta.len(), content_id: None, location: path.display().to_string() });
        }
    }
}

fn guess_type(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("txt") => "text/plain",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

/// A date for a message file with no plist date: its modification time.
pub fn file_modified_ms(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    let d = t.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(d.as_millis() as i64)
}
