//! A Thunderbird or Unix mbox file: messages one after another, each opened by a `From ` line (RFC 4155, the "mboxrd" form,
//! https://www.rfc-editor.org/rfc/rfc4155). A body line that begins with `>From ` was escaped when the file was written, and
//! the reader takes one `>` off it (`>>From ` becomes `>From `, `>From ` becomes `From `). The file is read line by line;
//! only the message being assembled is in memory.
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use crate::consts::IMPORT_MAX_MESSAGE_BYTES;
use crate::error::{classify_io_error, ImportError};
use crate::model::{MailFlags, MailVisitor, RawMail};
use crate::rawhdr::first_header;
use crate::timeutil::parse_asctime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MboxMessage {
    pub bytes: Vec<u8>,
    /// The date on the message's From_ line (when the mail was delivered), read as UTC.
    pub received_ms: Option<i64>,
    pub flags: MailFlags,
    /// The message grew past the size limit; `bytes` is empty and the rest of it was read and dropped.
    pub oversize: bool,
}

pub struct MboxMessages<R: BufRead> {
    reader: R,
    max_bytes: u64,
    line: Vec<u8>,
    pending_from: Option<Vec<u8>>,
    done: bool,
}

impl<R: BufRead> MboxMessages<R> {
    pub fn new(reader: R, max_bytes: u64) -> Self {
        MboxMessages { reader, max_bytes, line: Vec::new(), pending_from: None, done: false }
    }

    fn read_message(&mut self) -> io::Result<Option<MboxMessage>> {
        let from_line = match self.pending_from.take() {
            Some(l) => l,
            None => loop {
                self.line.clear();
                if self.reader.read_until(b'\n', &mut self.line)? == 0 {
                    return Ok(None);
                }
                if is_from_line(&self.line) {
                    break self.line.clone();
                }
                if !is_blank(&self.line) {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "the file does not start with a \"From \" line, so it is not an mbox"));
                }
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        let mut oversize = false;
        // The one blank line that closes a message belongs to the separator, not to the message: it is kept back until a
        // later line shows it was an ordinary blank line.
        let mut held: Option<Vec<u8>> = None;
        loop {
            self.line.clear();
            if self.reader.read_until(b'\n', &mut self.line)? == 0 {
                break;
            }
            if is_from_line(&self.line) {
                self.pending_from = Some(std::mem::take(&mut self.line));
                break;
            }
            if is_blank(&self.line) {
                if let Some(prev) = held.replace(self.line.clone()) {
                    append(&mut buf, &mut oversize, self.max_bytes, &prev);
                }
                continue;
            }
            if let Some(prev) = held.take() {
                append(&mut buf, &mut oversize, self.max_bytes, &prev);
            }
            append(&mut buf, &mut oversize, self.max_bytes, unescape_from(&self.line));
        }
        let received_ms = String::from_utf8_lossy(&from_line).get(5..).and_then(parse_asctime);
        let flags = if oversize { MailFlags::default() } else { flags_from_headers(&buf) };
        Ok(Some(MboxMessage { bytes: buf, received_ms, flags, oversize }))
    }
}

impl<R: BufRead> Iterator for MboxMessages<R> {
    type Item = io::Result<MboxMessage>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.read_message() {
            Ok(Some(m)) => Some(Ok(m)),
            Ok(None) => {
                self.done = true;
                None
            }
            // an error ends the file: later calls find nothing more to read
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

fn append(buf: &mut Vec<u8>, oversize: &mut bool, max_bytes: u64, data: &[u8]) {
    if *oversize {
        return;
    }
    if buf.len() as u64 + data.len() as u64 > max_bytes {
        *oversize = true;
        *buf = Vec::new();
        return;
    }
    buf.extend_from_slice(data);
}

fn is_blank(line: &[u8]) -> bool {
    line == b"\n" || line == b"\r\n"
}

/// "From <sender> <date with a four-digit year>": the shape RFC 4155 gives the separator. A body line that merely starts with
/// the word From does not pass.
fn is_from_line(line: &[u8]) -> bool {
    if !line.starts_with(b"From ") {
        return false;
    }
    let text = String::from_utf8_lossy(line);
    let rest = text[5..].trim();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let _sender = parts.next();
    parts.next().unwrap_or("").split_whitespace().any(|w| w.len() == 4 && w.bytes().all(|b| b.is_ascii_digit()))
}

/// mboxrd: a line of one or more `>` then `From ` had one `>` added when it was written.
fn unescape_from(line: &[u8]) -> &[u8] {
    let gt = line.iter().take_while(|b| **b == b'>').count();
    if gt > 0 && line[gt..].starts_with(b"From ") {
        &line[1..]
    } else {
        line
    }
}

// Mozilla's X-Mozilla-Status bits (hex): 1 read, 2 replied, 4 marked, 8 expunged.
const MOZ_READ: u32 = 0x1;
const MOZ_REPLIED: u32 = 0x2;
const MOZ_MARKED: u32 = 0x4;
const MOZ_EXPUNGED: u32 = 0x8;

/// Read state from the headers mail programs add: X-Mozilla-Status (Thunderbird), Status and X-Status (mutt, Pine and others).
/// A file that carries none of them says nothing, so the message counts as read.
pub fn flags_from_headers(message: &[u8]) -> MailFlags {
    let mut f = MailFlags::default();
    let moz = first_header(message, "x-mozilla-status").and_then(|v| u32::from_str_radix(v.trim(), 16).ok());
    if let Some(bits) = moz {
        f.read = bits & MOZ_READ != 0;
        f.answered = bits & MOZ_REPLIED != 0;
        f.flagged = bits & MOZ_MARKED != 0;
        f.deleted = bits & MOZ_EXPUNGED != 0;
    } else if let Some(status) = first_header(message, "status") {
        f.read = status.to_ascii_uppercase().contains('R');
    }
    if let Some(x) = first_header(message, "x-status") {
        let x = x.to_ascii_uppercase();
        f.flagged |= x.contains('F');
        f.answered |= x.contains('A');
        f.deleted |= x.contains('D');
        f.draft |= x.contains('T');
    }
    f
}

/// Hand every message of the file to the visitor, one at a time.
pub fn walk_mbox(file: &Path, visitor: &mut dyn MailVisitor) -> Result<(), ImportError> {
    let handle = File::open(file).map_err(|e| classify_io_error(file, &e))?;
    let mailbox = file.file_stem().and_then(|s| s.to_str()).unwrap_or("Imported").to_string();
    for (index, item) in MboxMessages::new(BufReader::new(handle), IMPORT_MAX_MESSAGE_BYTES).enumerate() {
        let origin = format!("{} message {}", file.display(), index + 1);
        match item {
            Err(e) => return Err(classify_io_error(file, &e)),
            Ok(m) if m.oversize => visitor.skip(&mailbox, origin, format!("is over the {IMPORT_MAX_MESSAGE_BYTES} byte limit for one message")),
            Ok(m) => visitor.mail(RawMail { mailbox: mailbox.clone(), bytes: m.bytes, flags: m.flags, received_ms: m.received_ms, attachments: Vec::new(), origin })?,
        }
    }
    Ok(())
}
