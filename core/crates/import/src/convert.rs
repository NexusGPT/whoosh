//! One message as a reader found it, to the row the store's insert takes. The RFC 822 bytes go through `parse::analyse`, the
//! same parser and sanitiser the rest of the app uses. Imported mail has no server to fetch a body from later, so its text and
//! its sanitised HTML are stored with it.
use contracts::Message;
use sha2::{Digest, Sha256};
use store::{NewMessage, INBOX_FOLDER};

use crate::matching::{account_for, candidate_addresses, is_sent_mailbox};
use crate::model::{AccountAddress, RawMail};

pub struct Converted {
    pub new: NewMessage,
    pub account_id: String,
    /// The Message-ID, or the content hash when there is none.
    pub key: String,
    /// The source had the message flagged.
    pub flagged: bool,
}

/// Hex of the first 16 bytes of the SHA-256 of the message bytes, prefixed `h:`.
pub fn content_key(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("h:{hex}")
}

/// The identity of a message across files and across runs: its Message-ID, else a hash of its bytes.
pub fn message_key(message_id: Option<&str>, bytes: &[u8]) -> String {
    match message_id.map(|m| m.trim().trim_matches(['<', '>'])).filter(|m| !m.is_empty()) {
        Some(m) => m.to_string(),
        None => content_key(bytes),
    }
}

/// The row's folder and whether it is filed away. The inbox stays "INBOX" (the name sync uses); other mailboxes keep their name.
pub fn folder_for(mailbox: &str) -> (String, bool) {
    if mailbox.eq_ignore_ascii_case("inbox") {
        return (INBOX_FOLDER.to_string(), false);
    }
    let last = mailbox.rsplit('/').next().unwrap_or(mailbox);
    let archived = last.eq_ignore_ascii_case("archive") || last.eq_ignore_ascii_case("all mail");
    (mailbox.to_string(), archived)
}

/// Err carries the words for a message left out.
pub fn convert(mail: &RawMail, accounts: &[AccountAddress], imported_account_id: &str) -> Result<Converted, String> {
    if mail.flags.deleted {
        return Err("is marked deleted in the mail program it came from".to_string());
    }
    let a = parse::analyse(&mail.bytes).map_err(|e| e.to_string())?;
    let m = &a.message;
    let from = m.from.as_ref().map(|f| f.email.as_str());
    let to: Vec<String> = m.to.iter().map(|x| x.email.clone()).collect();
    let cc: Vec<String> = m.cc.iter().map(|x| x.email.clone()).collect();
    let bcc: Vec<String> = m.bcc.iter().map(|x| x.email.clone()).collect();
    let candidates = candidate_addresses(&mail.bytes, &to, &cc, &bcc, from, is_sent_mailbox(&mail.mailbox));
    let account_id = account_for(accounts, &candidates).map(|acc| acc.id.clone()).unwrap_or_else(|| imported_account_id.to_string());

    let key = message_key(m.message_id.as_deref(), &mail.bytes);
    let id = format!("imp:{account_id}:{key}");
    let root = m.references.first().or_else(|| m.in_reply_to.first()).cloned().or_else(|| m.message_id.clone());
    let thread_id = root.map(|r| format!("{account_id}:{r}")).unwrap_or_else(|| id.clone());
    let (folder, archived) = folder_for(&mail.mailbox);
    let mut attachment_names: Vec<String> = m.attachments.iter().filter_map(|x| x.name.clone()).collect();
    attachment_names.extend(mail.attachments.iter().map(|x| x.name.clone()));
    let has_list_header = !m.list_unsubscribe.is_empty();

    let message = Message {
        id,
        thread_id,
        account_id: account_id.clone(),
        sender_name: m.from.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(),
        from_address: from.unwrap_or("").to_string(),
        to,
        subject: m.subject.clone(),
        preview: a.preview.clone(),
        received_at_ms: mail.received_ms.or(m.date_ms).unwrap_or(0),
        read: mail.flags.read,
        archived,
        folder,
        attachment_names,
        login_code: a.login.code.clone(),
        trackers_blocked: a.trackers_blocked,
        list_unsubscribe: Vec::new(),
        list_unsubscribe_one_click: false,
        reply_to_name: None,
        cc,
        bcc,
        is_outgoing: false,
    };
    let new = NewMessage {
        remote_id: Some(format!("import:{key}")),
        body_text: Some(m.text.clone()),
        body_html: a.safe_html.as_ref().map(|s| s.html.clone()),
        has_list_header,
        size_bytes: Some(mail.bytes.len() as i64),
        tab_id: None,
        message,
    };
    Ok(Converted { new, account_id, key, flagged: mail.flags.flagged })
}
