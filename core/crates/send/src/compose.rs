//! The message: From is the sending account's address, the signature is present exactly once.
use crate::account::SendAccount;
use crate::attachments::{check_built, part, OutgoingFile};
use crate::ports::Envelope;
use crate::threading::ReplyContext;
use contracts::{CoreError, Draft, Signature};
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::Message;
use std::time::{Duration, UNIX_EPOCH};

/// The usual signature delimiter: dash dash space, then a line end.
pub const SIGNATURE_DELIMITER: &str = "-- \n";

pub struct Composed {
    pub message_id: String,
    pub envelope: Envelope,
    pub rfc822: Vec<u8>,
}

fn invalid(field: &str, words: &str) -> CoreError {
    CoreError::Invalid { field: field.to_string(), reason_words: words.to_string() }
}

fn mailbox(field: &str, raw: &str) -> Result<Mailbox, CoreError> {
    raw.parse::<Mailbox>().map_err(|e| invalid(field, &format!("'{raw}' is not an address: {e}")))
}

/// How many non-blank lines may follow the signature in a body and still leave it "already there" (the app's footer line goes after it).
/// The engine does not know the footer's words: it only knows the footer is short. The ledger's row SIGNATURE_TAIL_MAX_LINES (docs/CONSTANTS.md).
pub const SIGNATURE_TAIL_MAX_LINES: usize = contracts::constants::SIGNATURE_TAIL_MAX_LINES as usize;
/// The longest such following line, in characters. The ledger's row SIGNATURE_TAIL_LINE_MAX_CHARS.
pub const SIGNATURE_TAIL_LINE_MAX_CHARS: usize = contracts::constants::SIGNATURE_TAIL_LINE_MAX_CHARS as usize;

/// Where the signature already stands in the body: its whole text as a block that starts a line and ends a line (with or without the "-- " line
/// above it), and after it at most `SIGNATURE_TAIL_MAX_LINES` non-blank lines of at most `SIGNATURE_TAIL_LINE_MAX_CHARS` characters each (the
/// footer). None of those lines is part of the signature: the block is matched whole first. Returns the text before the block (its "-- " line
/// dropped, trailing blanks trimmed) and what follows it (trimmed; "" when nothing). The last such block wins.
pub(crate) fn split_signature<'a>(body: &'a str, sig_text: &str) -> Option<(&'a str, &'a str)> {
    let text = sig_text.trim_end();
    if text.is_empty() {
        return None;
    }
    let body = body.trim_end();
    for (at, _) in body.rmatch_indices(text) {
        let starts_line = at == 0 || body.as_bytes()[at - 1] == b'\n';
        let tail = &body[at + text.len()..];
        let ends_line = tail.is_empty() || tail.starts_with('\n') || tail.starts_with("\r\n");
        if !starts_line || !ends_line {
            continue;
        }
        let lines: Vec<&str> = tail.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.len() > SIGNATURE_TAIL_MAX_LINES || lines.iter().any(|l| l.chars().count() > SIGNATURE_TAIL_LINE_MAX_CHARS) {
            continue;
        }
        let head = &body[..at];
        let head = head.strip_suffix(SIGNATURE_DELIMITER).unwrap_or(head);
        return Some((head.trim_end(), tail.trim()));
    }
    None
}

/// The body followed by the signature block, unless the signature already stands in it (alone at the end, or followed by the footer line).
fn with_signature(body: &str, signature: Option<&Signature>) -> String {
    let Some(sig) = signature else { return body.to_string() };
    let text = sig.text.trim_end();
    if text.is_empty() {
        return body.to_string();
    }
    let trimmed = body.trim_end();
    if split_signature(trimmed, text).is_some() {
        return trimmed.to_string();
    }
    format!("{trimmed}\n\n{SIGNATURE_DELIMITER}{text}")
}

/// The same draft always gets the same Message-ID, so a retry is the same message.
fn message_id(draft_id: &str, address: &str) -> Result<String, CoreError> {
    let domain = address.rsplit_once('@').map(|(_, d)| d).filter(|d| !d.is_empty());
    let domain = domain.ok_or_else(|| invalid("address", "the sending address has no domain"))?;
    let local: String = draft_id.chars().filter(|c| c.is_ascii_alphanumeric() || "-._".contains(*c)).collect();
    if local.is_empty() {
        return Err(invalid("draft.id", "the draft has no usable id"));
    }
    Ok(format!("<{local}@{domain}>"))
}

pub fn compose(
    account: &SendAccount,
    draft: &Draft,
    signature: Option<&Signature>,
    now_ms: i64,
    reply: Option<&ReplyContext>,
    files: &[OutgoingFile],
) -> Result<Composed, CoreError> {
    if draft.from_account_id != account.id {
        return Err(invalid("draft.from_account_id", "the draft names another account than the one sending"));
    }
    if let Some(sig) = signature {
        if sig.address != account.address {
            return Err(invalid("signature", "the signature belongs to another address than the sending account"));
        }
    }
    // delta 22: a draft that names files goes with every one of them, never without one (the caller reads them when the message is built)
    if files.len() != draft.attachment_ids.len() {
        return Err(invalid("draft.attachment_ids", "the files of this message could not all be read, so it was not sent"));
    }
    check_built(files)?;
    if draft.to.is_empty() {
        return Err(invalid("draft.to", "a message needs at least one recipient"));
    }

    let from = mailbox("account.address", &account.address)?;
    let id = message_id(&draft.id, &account.address)?;
    let date = UNIX_EPOCH + Duration::from_millis(u64::try_from(now_ms).map_err(|_| invalid("clock", "time before 1970"))?);

    let mut builder = Message::builder().from(from.clone()).subject(draft.subject.clone()).message_id(Some(id.clone())).date(date);
    if let Some(ctx) = reply {
        // RFC 5322 3.6.4: In-Reply-To names the parent (replies only); References is the chain, oldest first.
        if let Some(parent) = ctx.in_reply_to() {
            builder = builder.in_reply_to(parent.to_string());
        }
        builder = builder.references(ctx.references().join(" "));
    }
    let mut to_addrs = Vec::new();
    for raw in &draft.to {
        let mb = mailbox("draft.to", raw)?;
        to_addrs.push(mb.email.to_string());
        builder = builder.to(mb);
    }
    for raw in &draft.cc {
        let mb = mailbox("draft.cc", raw)?;
        to_addrs.push(mb.email.to_string());
        builder = builder.cc(mb);
    }
    // delta 12: Bcc recipients are in the envelope only; no header names them
    for raw in &draft.bcc {
        let mb = mailbox("draft.bcc", raw)?;
        to_addrs.push(mb.email.to_string());
    }
    let text = with_signature(&draft.body, signature);
    // gap-composer-rich: a signature with formatting or a picture in the text adds an HTML part (crate::rich); else the plain path below
    let domain = account.address.rsplit_once('@').map(|(_, d)| d).unwrap_or("");
    let built = if let Some(rich) = crate::rich::rich_body(&draft.body, signature, &draft.attachment_ids, files, domain) {
        builder.multipart(rich)
    } else if files.is_empty() {
        builder.body(text)
    } else {
        // delta 22: multipart/mixed, the body first, then one base64 part per file in the draft's order
        let mut mixed = MultiPart::mixed().singlepart(SinglePart::plain(text));
        for f in files {
            mixed = mixed.singlepart(part(f));
        }
        builder.multipart(mixed)
    };
    let message = built.map_err(|e| invalid("draft", &format!("the message could not be built: {e}")))?;

    Ok(Composed {
        message_id: id,
        envelope: Envelope { from: from.email.to_string(), to: to_addrs },
        rfc822: message.formatted(),
    })
}
