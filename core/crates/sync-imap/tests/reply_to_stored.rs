//! real-replyto (migration 0027): the IMAP sync reads a mail's Reply-To header (every address with its name, header order) from the header
//! block the fetch already holds, and `thread_headers_reply_to_many` reads it for stored mail in the SAME single fetch that reads the
//! threading headers (one more field name, no extra round trip). Everything runs against the in-process fake on 127.0.0.1 (tests/support),
//! every call under GUARD.
//!
//! RED-FIRST: each test names, in a comment, the one-line edit of the source that turns it red.
mod support;

use std::time::Duration;

use support::*;
use sync_imap::*;

const GUARD: Duration = Duration::from_secs(20);

async fn server() -> FakeImap {
    let srv = FakeImap::start().await;
    srv.with(|s| { s.qresync = false; s.cap_condstore = false; });
    srv.seed("INBOX", 0);
    srv
}

/// A mail with the given header lines (each ending in CRLF) into INBOX; its UID.
fn put(srv: &FakeImap, headers: &str) -> u32 {
    let raw = format!("From: A <a@example.test>\r\nTo: owner@example.test\r\nSubject: s\r\n{headers}\r\nBody.\r\n").into_bytes();
    srv.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
        uid
    })
}

fn pair(name: &str, address: &str) -> (String, String) { (name.to_string(), address.to_string()) }

#[tokio::test(flavor = "multi_thread")]
async fn a_mail_with_a_reply_to_header_is_fetched_with_every_name_and_address_in_order() {
    // RED (a): envelope.rs `build`, the field `reply_to_named: parsed.as_ref().map(|m| named_list(m.reply_to())).unwrap_or_default()`
    //   -> `reply_to_named: Vec::new()`
    // RED (b, order): envelope.rs `named_list`, `a.iter()` -> `a.iter().rev()`
    // RED (c, names): envelope.rs `named_list`, `x.name().unwrap_or_default().to_string()` -> `String::new()`
    let srv = server().await;
    let list = put(&srv, "Reply-To: \"Dev List\" <dev-list@lists.example.test>, Second <second@example.test>, bare@example.test\r\n");
    let plain = put(&srv, "Message-ID: <plain@example.test>\r\n");
    let folded = put(&srv, "Reply-To: =?UTF-8?Q?Andr=C3=A9?= <andre@example.test>,\r\n other@example.test\r\n");
    let a = adapter(&srv, "rt1");
    let mut sink = Sink::default();
    tokio::time::timeout(GUARD, a.initial_sync("INBOX", 50, 50, OPTS, &mut sink)).await.expect("initial sync hung").expect("initial sync");
    let of = |u: u32| sink.msgs.iter().find(|m| m.uid == u).map(|m| m.reply_to_named.clone()).expect("delivered");
    assert_eq!(of(list), vec![pair("Dev List", "dev-list@lists.example.test"), pair("Second", "second@example.test"), pair("", "bare@example.test")], "every address, with its name, in header order");
    assert!(of(plain).is_empty(), "no Reply-To header: nothing");
    assert_eq!(of(folded), vec![pair("André", "andre@example.test"), pair("", "other@example.test")], "a folded header and an encoded name");
    // the junk filter's own field keeps its lower-case addresses
    let m = sink.msgs.iter().find(|m| m.uid == list).unwrap();
    assert_eq!(m.reply_to, vec!["dev-list@lists.example.test".to_string(), "second@example.test".into(), "bare@example.test".into()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sync_fetch_asks_for_no_extra_item_to_read_the_reply_to() {
    // RED: adapter.rs `fetch_query`, add ` BODY.PEEK[HEADER.FIELDS (REPLY-TO)]` before the closing parenthesis
    let srv = server().await;
    put(&srv, "Reply-To: x@example.test\r\n");
    let a = adapter(&srv, "rt2");
    srv.clear_log();
    let mut sink = Sink::default();
    tokio::time::timeout(GUARD, a.initial_sync("INBOX", 50, 50, OPTS, &mut sink)).await.expect("hung").expect("initial sync");
    assert!(!srv.log().iter().any(|l| l.contains("HEADER.FIELDS")), "the Reply-To comes from BODY.PEEK[HEADER], which the fetch already asks for");
}

#[tokio::test(flavor = "multi_thread")]
async fn thread_headers_reply_to_many_reads_the_reply_to_in_the_same_one_fetch() {
    // RED (a, one more field): adapter.rs thread_headers_reply_to_many, in the command
    //   `(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO REPLY-TO)])` -> `(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO)])`
    // RED (b, no Message-ID): adapter.rs thread_headers_reply_to_many
    //   `if let Ok(p) = crate::blocking::parent_headers_from(block) { out.push((uid, p, crate::envelope::reply_to_named(block))); }`
    //   -> `out.push((uid, crate::blocking::parent_headers_from(block).unwrap_or(send::ParentHeaders { message_id: String::new(), references: Vec::new() }), crate::envelope::reply_to_named(block)));`
    // RED (c, blocking): blocking.rs AccountAdapters::thread_headers_reply_to_many `self.runtime.block_on(adapter.thread_headers_reply_to_many(folder, uids))` -> `Ok(Vec::new())`
    let srv = server().await;
    let with = put(&srv, "Message-ID: <w@example.test>\r\nReply-To: \"Dev List\" <dev-list@lists.example.test>, two@example.test\r\n");
    let without = put(&srv, "Message-ID: <n@example.test>\r\n");
    let no_id = put(&srv, "Reply-To: lost@example.test\r\n");
    let a = adapter(&srv, "rt3");
    srv.clear_log();
    let mut got = tokio::time::timeout(GUARD, a.thread_headers_reply_to_many("INBOX", &[without, no_id, with])).await.expect("hung").expect("never an error");
    got.sort_by_key(|(u, _, _)| *u);
    assert_eq!(got.len(), 2, "a mail with no Message-ID is absent, as in thread_headers_many");
    assert_eq!((got[0].0, got[0].1.message_id.as_str(), got[0].2.clone()), (with, "<w@example.test>", vec![pair("Dev List", "dev-list@lists.example.test"), pair("", "two@example.test")]));
    assert_eq!((got[1].0, got[1].1.message_id.as_str(), got[1].2.clone()), (without, "<n@example.test>", vec![]));
    let log = srv.log();
    let fetches: Vec<&String> = log.iter().filter(|l| l.contains("UID FETCH")).collect();
    assert_eq!(fetches.len(), 1, "one command for three UIDs, Reply-To included: {fetches:?}");
    assert!(fetches[0].contains("REPLY-TO)"), "{}", fetches[0]);
    assert!(tokio::time::timeout(GUARD, a.thread_headers_reply_to_many("INBOX", &[])).await.expect("hung").expect("empty").is_empty());
}
