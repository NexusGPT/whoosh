//! Contracts delta 22 in the send crate: a message with files is multipart/mixed, each file base64 with its name in RFC 2231 and RFC 2047
//! form, and arrives byte for byte; the Sent copy is the very bytes SMTP got; the caps are checked on the built files. Against the local
//! fake SMTP server only. The single-edit mutant each test was seen red on is named under it (docs/lanes/engine-act-r6.md).
mod support;

use contracts::{CoreError, Draft, ATTACHMENT_FILE_MAX_BYTES, ATTACHMENT_TOO_LARGE_CODE, ATTACHMENT_TOTAL_MAX_BYTES, ATTACHMENT_TOTAL_TOO_LARGE_CODE};
use send::{LettreDelivery, OutgoingFile, Security, SendAccount, SendInput, SendMessage, SmtpSettings};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use support::{FakeSmtp, FixedClock, Mode, StubSent};

const ADDRESS: &str = "assem@northlight-example.de";
const NOW_MS: i64 = 1_794_000_000_000;
const ACCENTED: &str = "Devis été 2026 final.pdf";

fn account(port: u16) -> SendAccount {
    SendAccount { id: "acc-nordlicht".into(), name: "Nordlicht".into(), address: ADDRESS.into(), smtp: SmtpSettings { host: "127.0.0.1".into(), port, security: Security::Plaintext, credentials: None } }
}
fn draft(ids: &[&str]) -> Draft {
    Draft {
        id: "draft-files-01".into(), from_account_id: "acc-nordlicht".into(), to: vec!["Linea Supply <orders@linea-example.com>".into()], cc: vec![], subject: "The files".into(),
        body: "Here they are.".into(), attachment_ids: ids.iter().map(|s| s.to_string()).collect(), ai_marks: vec![], reply_to_thread_id: None, bcc: vec![],
    }
}
fn file(name: &str, media: &str, bytes: Vec<u8>) -> OutgoingFile { OutgoingFile { file_name: name.into(), media_type: media.into(), bytes } }
/// Bytes no text encoding survives: every byte value, NUL, CR and LF alone, in a pseudo-random order.
fn binary(n: usize) -> Vec<u8> {
    let mut x: u32 = 0x9E37_79B9;
    (0..n).map(|_| { x ^= x << 13; x ^= x >> 17; x ^= x << 5; (x & 0xff) as u8 }).collect()
}
fn send(server: &FakeSmtp, sent: Arc<StubSent>, ids: &[&str], files: Vec<OutgoingFile>) -> Result<send::SendReceipt, CoreError> {
    SendMessage::new(Arc::new(LettreDelivery), sent, Arc::new(FixedClock(NOW_MS))).execute(&SendInput { account: account(server.port), draft: draft(ids), signature: None, reply: None, files })
}
fn code(r: &Result<send::SendReceipt, CoreError>) -> Option<String> { match r { Err(CoreError::Invalid { field, .. }) => Some(field.clone()), _ => None } }

#[test]
fn each_file_arrives_byte_for_byte_and_the_sent_copy_is_what_smtp_got() {
    let server = FakeSmtp::start(Mode::Accept);
    let sent = StubSent::ok();
    // a text file with bare LF line ends, 5 MiB of binary, a name with accents and spaces
    let text = b"line one\nline two\n\nthe end, no CR here\n".to_vec();
    let big = binary(5 * 1024 * 1024);
    let pdf = b"%PDF-1.4\n% \xe2\xe3\xcf\xd3\n1 0 obj\n<<>>\nendobj\n".to_vec();
    let files = vec![file("notes.txt", "text/plain", text.clone()), file("photo.raw", "application/octet-stream", big.clone()), file(ACCENTED, "application/pdf", pdf.clone())];
    send(&server, sent.clone(), &["a/1", "a/2", "a/3"], files).expect("sent");

    let got = server.received();
    assert_eq!(got.len(), 1, "one delivery");
    let data = got[0].data.as_bytes();
    for (name, bytes) in [("notes.txt", &text), ("photo.raw", &big), (ACCENTED, &pdf)] {
        let back = send::part_bytes(data, name, 0).unwrap_or_else(|| panic!("no part named {name:?} in what SMTP got"));
        assert!(back == *bytes, "{name}: {} bytes back, {} sent, first difference at {:?}", back.len(), bytes.len(), back.iter().zip(bytes.iter()).position(|(a, b)| a != b));
    }
    let head = String::from_utf8_lossy(&data[..data.len().min(4000)]).to_string();
    assert!(head.to_ascii_lowercase().contains("content-type: multipart/mixed"), "multipart/mixed:\n{head}");
    let whole = String::from_utf8_lossy(data);
    assert!(whole.contains("filename*=utf-8''Devis%20%C3%A9t%C3%A9%202026%20final.pdf") || whole.contains("filename*0*=utf-8''"), "RFC 2231 name in Content-Disposition");
    assert!(whole.contains("name=\"=?utf-8?B?"), "RFC 2047 name in Content-Type for the clients that read only that");
    assert!(whole.contains("Content-Transfer-Encoding: base64"), "the files are base64");
    assert!(whole.contains("Here they are."), "the body is the first part");
    // the Sent copy: the same bytes (the fake strips the CRLF that ends DATA)
    let appended = sent.appended.lock().unwrap();
    assert_eq!(appended.len(), 1);
    let copy = String::from_utf8(appended[0].rfc822.clone()).expect("ascii message");
    assert_eq!(copy.trim_end_matches("\r\n"), got[0].data.trim_end_matches("\r\n"), "the Sent copy is the message SMTP got, parts and all");
}
// mutant "a-file-loses-its-last-byte": attachments.rs `part`, `base64_lines(&file.bytes)` -> `base64_lines(&file.bytes[..file.bytes.len() - 1])` (red: each file one byte short)
// (seen SURVIVE and replaced: `.body(file.bytes.clone())` through lettre's own base64 also arrives byte for byte, so it is no defect)
// mutant "files-never-attached": compose.rs, `mixed = mixed.singlepart(part(f));` -> `mixed = mixed;` (red: no part named notes.txt)

#[test]
fn a_file_over_the_cap_or_files_over_the_total_are_refused_before_anything_leaves() {
    let server = FakeSmtp::start(Mode::Accept);
    let sent = StubSent::ok();
    // the caps are the contract's: pinned here so a silent change is red
    assert_eq!((ATTACHMENT_FILE_MAX_BYTES, ATTACHMENT_TOTAL_MAX_BYTES), (10 * 1024 * 1024, 14 * 1024 * 1024));
    let one_over = vec![file("big.bin", "application/octet-stream", vec![7u8; ATTACHMENT_FILE_MAX_BYTES as usize + 1])];
    assert_eq!(code(&send(&server, sent.clone(), &["a/1"], one_over)).as_deref(), Some(ATTACHMENT_TOO_LARGE_CODE));
    let half = (ATTACHMENT_TOTAL_MAX_BYTES / 2) as usize + 1;
    let two = vec![file("a.bin", "application/octet-stream", vec![1u8; half]), file("b.bin", "application/octet-stream", vec![2u8; half])];
    assert_eq!(code(&send(&server, sent.clone(), &["a/1", "a/2"], two)).as_deref(), Some(ATTACHMENT_TOTAL_TOO_LARGE_CODE));
    assert_eq!(server.connections.load(Ordering::SeqCst), 0, "nothing reached the server");
    assert_eq!(sent.count(), 0, "no Sent copy");
}
// mutant "caps-not-checked-on-the-built-files": compose.rs, `check_built(files)?;` deleted (red: both sent)

#[test]
fn a_draft_naming_a_file_that_was_not_read_is_never_sent_without_it() {
    let server = FakeSmtp::start(Mode::Accept);
    let r = send(&server, StubSent::ok(), &["a/1", "a/2"], vec![file("only.txt", "text/plain", b"one".to_vec())]);
    assert_eq!(code(&r).as_deref(), Some("draft.attachment_ids"), "{r:?}");
    assert!(server.received().is_empty());
}
