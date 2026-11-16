//! A144 (docs/SPEC.md): a server without CONDSTORE (Microsoft 365 / Exchange Online) keeps no change list. `resync` used to answer
//! `Reset` there, and the engine only re-read the newest page, so a mail that left the folder on the server stayed in the app and a
//! reply to it was refused ("the server has no message with UID 143986"). Now `resync` compares the UIDs the store knows with what the
//! server lists, `FetchedMessage.references` carries each mail's own chain, and `thread_headers_many` reads the threading headers of
//! many mails at once. Everything runs against the in-process fake on 127.0.0.1 (tests/support), every call under GUARD.
//!
//! RED-FIRST: each test names, in a comment, the one-line edit of the source that turns it red (the lead runs these broken copies).
//! Line texts are quoted from core/crates/sync-imap/src as committed on lane/sync-vanish.
mod support;

use std::time::Duration;

use contracts::SyncCursor;
use support::*;
use sync_imap::*;

/// Every call is bounded: a hang is a red test, never a stuck run.
const GUARD: Duration = Duration::from_secs(20);
/// RFC 7162 section 4: a client keeps a command line under 8192 octets.
const LINE_OCTETS: usize = 8192;

/// A fake that does not advertise CONDSTORE (nor QRESYNC), with `n` plain mails in INBOX (UIDs 1..=n, UIDVALIDITY 1000).
async fn plain_server(n: u32) -> FakeImap {
    let srv = FakeImap::start().await;
    srv.with(|s| { s.qresync = false; s.cap_condstore = false; });
    srv.seed("INBOX", n);
    srv
}

/// A mail with the given header lines (each ending in CRLF) into INBOX; its UID.
fn put(srv: &FakeImap, headers: &str) -> u32 {
    let raw = format!("From: A <a@example.test>\r\nTo: owner@example.test\r\nSubject: threaded\r\n{headers}\r\nBody.\r\n").into_bytes();
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

/// The first sync the engine runs: its cursor and the UIDs the store then knows (sorted), and the mails it delivered.
async fn first_sync(a: &ImapAdapter) -> (SyncCursor, Vec<u32>, Vec<FetchedMessage>) {
    let mut sink = Sink::default();
    let snap = tokio::time::timeout(GUARD, a.initial_sync("INBOX", 50, 50, OPTS, &mut sink)).await.expect("initial sync hung").expect("initial sync");
    let mut known: Vec<u32> = sink.msgs.iter().map(|m| m.uid).collect();
    known.sort_unstable();
    (snap.cursor, known, sink.msgs)
}

async fn resync(a: &ImapAdapter, cursor: &SyncCursor, known: &[u32]) -> Resync {
    tokio::time::timeout(GUARD, a.resync("INBOX", cursor, known, OPTS)).await.expect("resync hung").expect("resync")
}

fn delta(r: Resync) -> Delta {
    match r {
        Resync::Delta(d) => d,
        Resync::Reset { reason_words } => panic!("a server without CONDSTORE must answer a delta, got a reset: {reason_words}"),
    }
}

/// The plain path never asks for a change list the server does not have.
fn assert_no_change_list(srv: &FakeImap) {
    let log = srv.log();
    assert!(!log.iter().any(|l| l.contains("CHANGEDSINCE") || l.contains("QRESYNC (")), "a CONDSTORE command reached a server without it: {log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_known_mail_removed_on_the_server_is_in_vanished() {
    // RED (a): adapter.rs resync_plain
    //   `let (still, vanished): (Vec<u32>, Vec<u32>) = known_sorted.iter().copied().partition(|u| live.contains(u));`
    //   -> `let (still, vanished): (Vec<u32>, Vec<u32>) = known_sorted.iter().copied().partition(|_| true);`
    // (on the base 4164510a this case answered `Reset`, adapter.rs:644-646; this file does not compile there, it names later additions)
    let srv = plain_server(5).await;
    let a = adapter(&srv, "a144a");
    let (cursor, known, _) = first_sync(&a).await;
    assert_eq!(known, vec![1, 2, 3, 4, 5]);
    srv.expunge("INBOX", &[3]); // the owner's case: the mail left INBOX on the server
    srv.clear_log();

    let d = delta(resync(&a, &cursor, &known).await);
    assert_eq!(d.vanished, vec![3], "the UID the server no longer lists");
    assert!(d.new.is_empty(), "{:?}", d.new.iter().map(|m| m.uid).collect::<Vec<_>>());
    assert!(!d.flag_changes.iter().any(|c| c.uid == 3), "a vanished UID has no flags");
    assert_eq!(d.cursor, SyncCursor::Imap { uid_validity: 1000, highest_mod_seq: 0 }, "UIDVALIDITY kept, no modseq exists here");
    let log = srv.log();
    assert_eq!(log.iter().filter(|l| l.contains("UID SEARCH")).count(), 1, "one UID SEARCH: {log:?}");
    assert_no_change_list(&srv);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_mail_above_the_known_ones_is_in_new() {
    // RED (b): adapter.rs resync_plain
    //   `let new_uids: Vec<u32> = live.iter().copied().filter(|u| *u > max_known).collect();`
    //   -> `let new_uids: Vec<u32> = live.iter().copied().filter(|_| false).collect();`
    let srv = plain_server(4).await;
    let a = adapter(&srv, "a144b");
    let (cursor, known, _) = first_sync(&a).await;
    let arrived = srv.add_message("INBOX");
    srv.clear_log();

    let d = delta(resync(&a, &cursor, &known).await);
    assert_eq!(d.new.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![arrived], "only the mail above every known UID");
    let m = &d.new[0];
    assert_eq!((m.uid_validity, m.message.id.as_str()), (1000, format!("a144b:INBOX:1000:{arrived}").as_str()));
    // fetched in full by fetch_set: the header block (Message-ID, Subject), the size and the first body bytes all came back.
    // `message_id_header` keeps the existing form, WITHOUT angle brackets (envelope.rs: mail-parser's `message_id()`).
    assert_eq!(m.message_id_header.as_deref(), Some(format!("{arrived}@example.test").as_str()), "the Message-ID from the header block");
    assert_eq!(m.message.subject, format!("Message {arrived}"), "the Subject from the header block");
    assert!(m.size.is_some_and(|n| n > 0), "RFC822.SIZE came back: {:?}", m.size);
    assert!(m.message.preview.contains(&format!("Body of message {arrived}")), "the first body bytes: {:?}", m.message.preview);
    assert!(d.vanished.is_empty(), "{:?}", d.vanished);
    assert_no_change_list(&srv);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_flag_set_on_the_server_is_in_flag_changes() {
    // RED (c): adapter.rs resync_plain `if chunk.binary_search(&uid).is_ok() {` -> `if false {`
    let srv = plain_server(5).await;
    let a = adapter(&srv, "a144c");
    let (cursor, known, _) = first_sync(&a).await;
    srv.change_flags("INBOX", &[2], "\\Flagged");
    srv.change_flags("INBOX", &[4], "\\Seen");
    srv.clear_log();

    let d = delta(resync(&a, &cursor, &known).await);
    let mut uids: Vec<u32> = d.flag_changes.iter().map(|c| c.uid).collect();
    uids.sort_unstable();
    assert_eq!(uids, known, "the current flags of every known UID still there");
    let flags = |u: u32| d.flag_changes.iter().find(|c| c.uid == u).map(|c| c.flags.clone()).unwrap_or_default();
    assert!(flags(2).iter().any(|f| f == "\\Flagged"), "{:?}", flags(2));
    assert!(flags(4).iter().any(|f| f == "\\Seen"), "{:?}", flags(4));
    assert!(flags(1).is_empty(), "an untouched mail reports no flag: {:?}", flags(1));
    let log = srv.log();
    assert!(log.iter().any(|l| l.contains("UID FETCH") && l.contains("(UID FLAGS)")), "{log:?}");
    assert_no_change_list(&srv);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_flag_read_of_many_known_uids_goes_in_chunks_that_keep_each_line_short() {
    // RED (c, chunks): adapter.rs resync_plain `for chunk in still.chunks(IMAP_UID_SET_CHUNK) {` -> `for chunk in still.chunks(usize::MAX) {`
    // 2000 known UIDs that are not consecutive (every odd one): as one set they take over 8192 octets.
    let srv = plain_server(4000).await;
    let a = adapter(&srv, "a144chunks");
    let known: Vec<u32> = (1..=3999).step_by(2).collect();
    let cursor = SyncCursor::Imap { uid_validity: 1000, highest_mod_seq: 0 };
    srv.clear_log();

    let d = delta(resync(&a, &cursor, &known).await);
    assert_eq!(d.flag_changes.len(), known.len(), "every known UID answered");
    assert!(d.vanished.is_empty());
    assert_eq!(d.new.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![4000], "only the UID above the highest known one");
    let log = srv.log();
    let lines: Vec<&String> = log.iter().filter(|l| l.contains("(UID FLAGS)")).collect();
    assert!(lines.len() >= 2, "the 2000 UIDs went out in more than one command: {} command(s)", lines.len());
    for l in &lines { assert!(l.len() < LINE_OCTETS, "a command line of {} octets", l.len()); }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_uidvalidity_or_no_known_uid_still_resets() {
    // RED (1a): adapter.rs resync_plain `if u64::from(uv) != want_uv {` -> `if false {`
    // RED (1f): adapter.rs resync_plain, inside `let Some(max_known) = .. else {`:
    //   `return Ok(Resync::Reset { reason_words: "the server has no CONDSTORE, so changes cannot be listed".into() });`
    //   -> `return Ok(Resync::Delta(Delta { new: vec![], flag_changes: vec![], vanished: vec![], cursor: imap_cursor(0, 0) }));`
    let srv = plain_server(3).await;
    let a = adapter(&srv, "a144uv");
    let (cursor, known, _) = first_sync(&a).await;

    srv.clear_log();
    match resync(&a, &cursor, &[]).await {
        Resync::Reset { reason_words } => assert!(reason_words.contains("CONDSTORE"), "{reason_words}"),
        other => panic!("no known UID: expected a reset, got {other:?}"),
    }
    assert!(!srv.log().iter().any(|l| l.contains("SELECT")), "nothing to compare: no command is sent");

    srv.set_uid_validity("INBOX", 2000);
    match resync(&a, &cursor, &known).await {
        Resync::Reset { reason_words } => assert!(reason_words.contains("UIDVALIDITY"), "{reason_words}"),
        other => panic!("a rebuilt folder: expected a reset, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn references_holds_the_mails_own_chain() {
    // RED (d): adapter.rs from_fetch `m.references = crate::blocking::thread_chain(header);` -> `m.references = Vec::new();`
    // RED (d, fallback): blocking.rs thread_chain
    //   `if references.is_empty() { header_ids(header_block, "In-Reply-To") } else { references }` -> `references`
    let srv = plain_server(0).await;
    let chained = put(&srv, "Message-ID: <c@example.test>\r\nIn-Reply-To: <r2@example.test>\r\nReferences: <r1@example.test>\r\n <r2@example.test>\r\n");
    let replied = put(&srv, "Message-ID: <i@example.test>\r\nIn-Reply-To: <p@example.test>\r\n");
    let alone = put(&srv, "Message-ID: <n@example.test>\r\n");
    let a = adapter(&srv, "a144d");
    let (cursor, known, msgs) = first_sync(&a).await;
    let refs = |u: u32| msgs.iter().find(|m| m.uid == u).map(|m| m.references.clone()).expect("delivered");
    assert_eq!(refs(chained), vec!["<r1@example.test>".to_string(), "<r2@example.test>".into()], "References, folded line read, in order");
    assert_eq!(refs(replied), vec!["<p@example.test>".to_string()], "no References: In-Reply-To");
    assert!(refs(alone).is_empty(), "neither header: empty");

    // the same chain on a mail the resync brings
    let later = put(&srv, "Message-ID: <l@example.test>\r\nReferences: <c@example.test>\r\n");
    let d = delta(resync(&a, &cursor, &known).await);
    let m = d.new.iter().find(|m| m.uid == later).expect("the new mail");
    assert_eq!(m.references, vec!["<c@example.test>".to_string()]);
    assert!(!srv.log().iter().any(|l| l.contains("HEADER.FIELDS")), "no fetch item was added: the chain comes from BODY.PEEK[HEADER]");
}

#[tokio::test(flavor = "multi_thread")]
async fn thread_headers_many_returns_the_answered_uids_only() {
    // RED (e, no Message-ID): adapter.rs thread_headers_many
    //   `if let Ok(p) = crate::blocking::parent_headers_from(block) { out.push((uid, p)); }`
    //   -> `out.push((uid, crate::blocking::parent_headers_from(block).unwrap_or(send::ParentHeaders { message_id: String::new(), references: Vec::new() })));`
    // RED (e, unanswered): adapter.rs thread_headers_many, the closure's last line
    //   `Ok(out)` -> `if out.len() < asked.len() { return Err(words("a UID was not answered")); } Ok(out)`
    // RED (e, chain): adapter.rs thread_headers_many, in the command
    //   `(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO)])` -> `(UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)])`
    let srv = plain_server(0).await;
    let parent = put(&srv, "Message-ID: <p@example.test>\r\n");
    let reply = put(&srv, "Message-ID: <r@example.test>\r\nIn-Reply-To: <p@example.test>\r\nReferences: <root@example.test>\r\n <p@example.test>\r\n");
    let no_id = put(&srv, "X-Note: this mail has no Message-ID\r\n");
    let gone = put(&srv, "Message-ID: <g@example.test>\r\n");
    srv.expunge("INBOX", &[gone]);
    let never = 99;
    let a = adapter(&srv, "a144e");
    srv.clear_log();

    let mut got = tokio::time::timeout(GUARD, a.thread_headers_many("INBOX", &[reply, never, gone, no_id, parent])).await.expect("hung").expect("never an error");
    got.sort_by_key(|(u, _)| *u);
    assert_eq!(got, vec![
        (parent, send::ParentHeaders { message_id: "<p@example.test>".into(), references: vec![] }),
        (reply, send::ParentHeaders { message_id: "<r@example.test>".into(), references: vec!["<root@example.test>".into(), "<p@example.test>".into()] }),
    ], "the answered UIDs with a Message-ID, nothing else");
    let log = srv.log();
    let fetches: Vec<&String> = log.iter().filter(|l| l.contains("UID FETCH")).collect();
    assert_eq!(fetches.len(), 1, "one command for five UIDs: {fetches:?}");
    assert!(fetches[0].contains("BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO)]"), "{}", fetches[0]);
    assert!(tokio::time::timeout(GUARD, a.thread_headers_many("INBOX", &[])).await.expect("hung").expect("empty").is_empty());
}

#[test]
fn the_blocking_form_answers_the_same_from_a_plain_thread() {
    // RED (e, blocking): blocking.rs AccountAdapters::thread_headers_many
    //   `self.runtime.block_on(adapter.thread_headers_many(folder, uids))` -> `Ok(Vec::new())`
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let srv = rt.block_on(plain_server(2));
    let a = rt.block_on(async { adapter(&srv, "a144blocking") });
    let ports = AccountAdapters::new(rt.handle().clone()).with("a144blocking", a);
    // called from this test's own thread (not a runtime worker), as the core's send path calls it
    let (tx, rx) = std::sync::mpsc::channel();
    let ports2 = ports.clone();
    std::thread::spawn(move || { let _ = tx.send(ports2.thread_headers_many("a144blocking", "INBOX", &[1, 2, 7])); });
    let got = rx.recv_timeout(GUARD).expect("the blocking call hung").expect("answered");
    assert_eq!(got.iter().map(|(u, p)| (*u, p.message_id.clone())).collect::<Vec<_>>(), vec![(1, "<1@example.test>".to_string()), (2, "<2@example.test>".into())]);
    assert!(ports.thread_headers_many("no-such-account", "INBOX", &[1]).is_err(), "an unknown account is an error in words");
}
