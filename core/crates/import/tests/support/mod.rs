#![allow(dead_code)]
//! Fixtures the tests make themselves: synthetic messages, an Apple Mail tree, an mbox, an Outlook archive. No real mail.
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use import::{AccountAddress, ImportError, MailVisitor, MessageSink, RawMail};
use store::NewMessage;

pub const IMPORTED: &str = "acc-imported";

/// Two accounts. Ben's address is mixed-case on purpose: the match must not care.
pub fn accounts() -> Vec<AccountAddress> {
    vec![
        AccountAddress { id: "acc-a".into(), address: "anna@example.test".into() },
        AccountAddress { id: "acc-b".into(), address: "Ben@Example.Test".into() },
    ]
}

/// A plain RFC 822 message with LF line ends (the way Apple Mail and mbox files hold them).
pub fn msg(id: &str, from: &str, to: &str, subject: &str, body: &str) -> String {
    format!("Message-ID: <{id}>\nFrom: {from}\nTo: {to}\nSubject: {subject}\nDate: Tue, 02 Jan 2024 10:11:12 +0000\n\n{body}\n")
}

/// The same without a Message-ID.
pub fn msg_no_id(from: &str, to: &str, subject: &str, body: &str) -> String {
    format!("From: {from}\nTo: {to}\nSubject: {subject}\nDate: Tue, 02 Jan 2024 10:11:12 +0000\n\n{body}\n")
}

/// An .emlx file: the byte count, the message, the plist Mail appends.
pub fn emlx(message: &str, flags: u64, received_s: i64) -> Vec<u8> {
    let mut out = format!("{}\n", message.len()).into_bytes();
    out.extend_from_slice(message.as_bytes());
    out.extend_from_slice(
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n\t<key>conversation-id</key>\n\t<integer>7</integer>\n\t<key>date-received</key>\n\t<integer>{received_s}</integer>\n\
             \t<key>flags</key>\n\t<integer>{flags}</integer>\n</dict>\n</plist>\n"
        )
        .as_bytes(),
    );
    out
}

/// The Messages folder of a mailbox in an Apple Mail tree: `<dir>/Mail/V10/<account>/<chain>.mbox/.../GUID/Data/1/Messages`.
/// `chain` is the mailbox path, nested mailboxes as separate names. The files are written into it.
pub fn apple_mailbox(dir: &Path, account: &str, chain: &[&str], files: &[(&str, Vec<u8>)]) -> PathBuf {
    let mut p = dir.join("Mail").join("V10").join(account);
    for c in chain {
        p = p.join(format!("{c}.mbox"));
    }
    let messages = p.join("GUID").join("Data").join("1").join("Messages");
    fs::create_dir_all(&messages).unwrap();
    for (name, bytes) in files {
        fs::write(messages.join(name), bytes).unwrap();
    }
    messages
}

/// A file of a partial message's attachments: `<Data>/Attachments/<number>/<part>/<name>`.
pub fn partial_attachment(messages_dir: &Path, number: &str, part: &str, name: &str, bytes: &[u8]) {
    let d = messages_dir.parent().unwrap().join("Attachments").join(number).join(part);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join(name), bytes).unwrap();
}

pub fn mail_dir(dir: &Path) -> PathBuf {
    dir.join("Mail")
}

/// mboxrd writer, kept apart from the reader: a body line of `>`s then `From ` gets one more `>`.
pub fn mbox(messages: &[&str]) -> Vec<u8> {
    let mut out = String::new();
    for m in messages {
        out.push_str("From MAILER-DAEMON Tue Jan  2 10:11:12 2024\n");
        for line in m.split_inclusive('\n') {
            if line.trim_start_matches('>').starts_with("From ") {
                out.push('>');
            }
            out.push_str(line);
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.into_bytes()
}

/// An Outlook message XML as Outlook for Mac writes it (the elements the importer reads).
pub struct OlmMsg<'a> {
    pub id: &'a str,
    pub subject: &'a str,
    pub from_name: &'a str,
    pub from: &'a str,
    pub to: &'a str,
    pub sent: &'a str,
    pub text: &'a str,
    pub html: &'a str,
    pub read: bool,
    /// (name, content type, zip entry)
    pub attachments: &'a [(&'a str, &'a str, &'a str)],
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn olm_xml(m: &OlmMsg<'_>) -> String {
    let atts: String = m
        .attachments
        .iter()
        .map(|(n, t, u)| format!("<messageAttachment OPFAttachmentName=\"{n}\" OPFAttachmentContentType=\"{t}\" OPFAttachmentURL=\"{u}\" OPFAttachmentContentID=\"\"/>"))
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<emails><email>\n\
         <OPFMessageCopyMessageID>{id}</OPFMessageCopyMessageID>\n\
         <OPFMessageCopySubject>{subject}</OPFMessageCopySubject>\n\
         <OPFMessageCopyFromAddresses><emailAddress OPFContactEmailAddressAddress=\"{from}\" OPFContactEmailAddressName=\"{from_name}\" OPFContactEmailAddressType=\"1\"/></OPFMessageCopyFromAddresses>\n\
         <OPFMessageCopyToAddresses><emailAddress OPFContactEmailAddressAddress=\"{to}\" OPFContactEmailAddressName=\"\" OPFContactEmailAddressType=\"1\"/></OPFMessageCopyToAddresses>\n\
         <OPFMessageCopySentTime>{sent}</OPFMessageCopySentTime>\n\
         <OPFMessageCopyReceivedTime>{sent}</OPFMessageCopyReceivedTime>\n\
         <OPFMessageCopyBody>{text}</OPFMessageCopyBody>\n\
         <OPFMessageCopyHTMLBody>{html}</OPFMessageCopyHTMLBody>\n\
         <OPFMessageGetIsRead>{read}</OPFMessageGetIsRead>\n\
         <OPFMessageCopyAttachmentList>{atts}</OPFMessageCopyAttachmentList>\n\
         </email></emails>\n",
        id = m.id,
        subject = esc(m.subject),
        from = m.from,
        from_name = esc(m.from_name),
        to = m.to,
        sent = m.sent,
        text = esc(m.text),
        html = esc(m.html),
        read = if m.read { 1 } else { 0 },
        atts = atts,
    )
}

/// A zip file (stored, no compression) with the given entries.
pub fn olm_file(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let file = fs::File::create(path).unwrap();
    let mut w = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        w.start_file(*name, options.clone()).unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap();
}

/// Collects what a reader hands over.
#[derive(Default)]
pub struct Collect {
    pub mails: Vec<RawMail>,
    /// (mailbox, origin, words)
    pub skips: Vec<(String, String, String)>,
}

impl MailVisitor for Collect {
    fn mail(&mut self, mail: RawMail) -> Result<(), ImportError> {
        self.mails.push(mail);
        Ok(())
    }
    fn skip(&mut self, mailbox: &str, origin: String, words: String) {
        self.skips.push((mailbox.to_string(), origin, words));
    }
}

/// A sink that behaves like the store's `INSERT OR IGNORE` on the message id: a known id is not stored again.
/// (The real store is exercised in tests/store.rs.)
#[derive(Default)]
pub struct VecSink {
    pub ids: HashSet<String>,
    pub rows: Vec<NewMessage>,
    pub batches: Vec<usize>,
}

impl MessageSink for VecSink {
    fn insert(&mut self, batch: Vec<NewMessage>) -> Result<usize, ImportError> {
        self.batches.push(batch.len());
        let mut new = 0;
        for n in batch {
            if self.ids.insert(n.message.id.clone()) {
                self.rows.push(n);
                new += 1;
            }
        }
        Ok(new)
    }
}

/// A sink whose database refuses the write.
pub struct FailingSink;

impl MessageSink for FailingSink {
    fn insert(&mut self, _batch: Vec<NewMessage>) -> Result<usize, ImportError> {
        Err(ImportError::Store { words: "the local database refused the imported mail: disk full".into() })
    }
}
