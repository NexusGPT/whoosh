//! lane reply-headers (SPEC A144, migration 0023): a reply reads its parent's Message-ID and References from the store first, so a reply to
//! an archived or moved mail (no place on the server) is threaded offline. The owner's bug: "the message you reply to could not be read, so
//! the reply cannot be threaded yet: the server has no message with UID 143986". The server's bytes are asserted (FakeSmtp on the loopback).
//! Not run by this lane (no cargo here): the lead runs `cargo test -p outbox`.
//! RED-FIRST (one-line broken copy per test):
//!   R1 outbox/src/adapters/reply/mod.rs `execute`: `if let Some(found) = self.stored(thread_id, kind)? {` becomes
//!      `if let Some(found) = None::<ReplyContext> {`                                  -> a_reply_to_a_mail_with_stored_headers_and_no_server_place_is_threaded_from_the_store
//!   R2 outbox/src/adapters/reply/mod.rs `stored`: `parent_references: h.references` becomes `parent_references: vec![]`
//!                                                                                      -> a_reply_to_a_mail_with_stored_headers_and_no_server_place_is_threaded_from_the_store
//!   R3 outbox/src/adapters/reply/mod.rs `execute`: `let headers = self.headers.execute(&parent)?;` becomes
//!      `let headers = send::ParentHeaders { message_id: "<x@x>".into(), references: vec![] };` -> a_thread_with_no_stored_headers_still_asks_the_server
//!   R4 store/src/thread_headers.rs `newest_in_thread`: `AND rfc_message_id <> ''` dropped -> the_newest_message_with_stored_headers_is_answered_and_a_marked_one_is_skipped
mod support;

use outbox::DeviceRole;
use send::ParentHeaders;
use store::ThreadHeaders;
use support::*;

const T0: i64 = EIGHT - 3 * 60 * MINUTE;
const AFTER_WINDOW: i64 = T0 + 60_000;

/// The header block of a received message with folded lines joined, as (lower-case name, value).
fn headers(data: &str) -> Vec<(String, String)> {
    let head = data.split_once("\r\n\r\n").expect("header/body split").0;
    let unfolded = head.replace("\r\n ", " ").replace("\r\n\t", " ");
    unfolded.lines().filter_map(|l| l.split_once(':').map(|(n, v)| (n.to_ascii_lowercase(), v.trim().to_string()))).collect()
}
fn get(h: &[(String, String)], name: &str) -> Vec<String> {
    h.iter().filter(|(n, _)| n == name).map(|(_, v)| v.clone()).collect()
}

/// One message of `thread` (the thread row made on first use), at `remote` on the server or nowhere (None: archived or moved, no
/// location row), with stored headers or not; `Some("")` as the Message-ID writes the fill-in pass's not-answered mark.
fn put_mail(d: &Device, thread: &str, id: &str, at: i64, remote: Option<&str>, stored: Option<ThreadHeaders>) {
    let (t, m, r) = (thread.to_string(), id.to_string(), remote.map(str::to_string));
    d.store
        .write(move |ctx| {
            ctx.tx.execute("INSERT OR IGNORE INTO thread (id, account_id, subject) VALUES (?1, ?2, 'Quote for the Q1 print run')", rusqlite::params![t, ACCOUNT_ID])?;
            ctx.tx.execute(
                "INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) \
                 VALUES (?1, ?2, ?3, ?4, 'Linea Supply', 'orders@linea-example.com', 'Quote for the Q1 print run', ?5, 'INBOX')",
                rusqlite::params![m, t, ACCOUNT_ID, r, at],
            )?;
            Ok(())
        })
        .expect("insert the mail");
    match stored {
        Some(h) if h.message_id.is_empty() => { d.store.set_thread_headers_quiet(vec![(id.to_string(), None)]).expect("mark"); }
        Some(h) => { d.store.set_thread_headers_quiet(vec![(id.to_string(), Some(h))]).expect("store the headers"); }
        None => {}
    }
}
fn c3() -> ThreadHeaders {
    ThreadHeaders { message_id: "<c-3@linea-example.com>".into(), references: vec!["<a-1@linea-example.com>".into(), "<b-2@linea-example.com>".into()] }
}
fn sent_headers(smtp: &FakeSmtp, d: &Device) -> Vec<(String, String)> {
    d.clock.set(AFTER_WINDOW);
    d.outbox.tick().unwrap();
    let got = smtp.received();
    assert_eq!(got.len(), 1);
    headers(&got[0].data)
}

#[test]
fn a_reply_to_a_mail_with_stored_headers_and_no_server_place_is_threaded_from_the_store() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = Device::new("mac", DeviceRole::Primary, T0, smtp.port, None);
    // the owner archived the mail: no remote id, no location row; its headers were stored at arrival. The server port fails if called
    // (`Parents` with no answer refuses).
    put_mail(&d, "thr-1", "thr-1-m1", 1000, None, Some(c3()));
    d.put_draft_in("draft-1", "Yes, 12,000 units works.", Some("thr-1"));
    d.outbox.send_draft("draft-1").expect("a reply to a stored mail is queued with its headers, not refused");
    assert!(d.parents.asked.lock().unwrap().is_empty(), "the server was not asked");
    let h = sent_headers(&smtp, &d);
    assert_eq!(get(&h, "in-reply-to"), vec!["<c-3@linea-example.com>".to_string()]);
    assert_eq!(get(&h, "references"), vec!["<a-1@linea-example.com> <b-2@linea-example.com> <c-3@linea-example.com>".to_string()], "the chain, then the parent, each once");
}

#[test]
fn a_thread_with_no_stored_headers_still_asks_the_server() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = Device::new("mac", DeviceRole::Primary, T0, smtp.port, None);
    // a mail stored before 0023 that the fill-in pass has not reached: no stored headers, a place on the server
    d.put_parent("thr-1", "INBOX", 42, ParentHeaders { message_id: "<s-9@linea-example.com>".into(), references: vec!["<s-1@linea-example.com>".into()] });
    d.put_draft_in("draft-1", "Thanks.", Some("thr-1"));
    d.outbox.send_draft("draft-1").expect("queued");
    let asked = d.parents.asked.lock().unwrap().clone();
    assert_eq!((asked.len(), asked[0].folder.as_str(), asked[0].remote_id.as_str()), (1, "INBOX", "42"), "the server was asked: {asked:?}");
    let h = sent_headers(&smtp, &d);
    assert_eq!(get(&h, "in-reply-to"), vec!["<s-9@linea-example.com>".to_string()]);
    assert_eq!(get(&h, "references"), vec!["<s-1@linea-example.com> <s-9@linea-example.com>".to_string()]);
}

#[test]
fn the_newest_message_with_stored_headers_is_answered_and_a_marked_one_is_skipped() {
    let smtp = FakeSmtp::start(Mode::Accept);
    let d = Device::new("mac", DeviceRole::Primary, T0, smtp.port, None);
    put_mail(&d, "thr-1", "thr-1-m1", 1000, None, Some(ThreadHeaders { message_id: "<old@linea-example.com>".into(), references: vec![] }));
    put_mail(&d, "thr-1", "thr-1-m2", 2000, Some("77"), Some(c3()));
    // the newest: the server was asked for it and gave nothing (the '' mark): never a parent
    put_mail(&d, "thr-1", "thr-1-m3", 3000, Some("78"), Some(ThreadHeaders { message_id: String::new(), references: vec![] }));
    d.put_draft_in("draft-1", "Thanks.", Some("thr-1"));
    d.outbox.send_draft("draft-1").expect("queued");
    assert!(d.parents.asked.lock().unwrap().is_empty(), "the store answered");
    let h = sent_headers(&smtp, &d);
    assert_eq!(get(&h, "in-reply-to"), vec!["<c-3@linea-example.com>".to_string()], "the newest message with a stored Message-ID");
}
