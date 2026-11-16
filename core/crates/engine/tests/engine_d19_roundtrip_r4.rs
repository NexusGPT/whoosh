//! engine-act-r4 (delta 19's server moves, round-tripped on a real IMAP server): Unarchive puts the archived message back in the SERVER's INBOX,
//! MarkSpam then MarkNotSpam takes a message to the server's Junk folder and back, and MarkNotSpam moves a message the server itself holds in
//! Junk. The fake server's boxes are read (not the pending rows). Nothing leaves this Mac. The mutants are named under each test.
//! The three `finding_` tests were `#[ignore]`d in engine-act-r4 (red on main55's sources: docs/lanes/engine-act-r4.md "## Findings"); engine-act-r5
//! fixed them (the server's place is recorded after a MOVE: docs/lanes/engine-act-r5.md) and they run with the rest.
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::writeback::{backoff_ms, Ledger, Worker};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn mailbox(m: Mailbox) -> InboxQuery { InboxQuery { mailbox: Some(m), ..tab("primary") } }
fn ids(rows: &[InboxRow]) -> Vec<String> { let mut v: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect(); v.sort(); v }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn not_built(r: &Result<CommandOutcome, CoreError>) -> bool {
    matches!(r, Err(CoreError::Action { failure }) if failure.code.as_deref() == Some("engine.not_built_yet"))
}

// ---------------------------------------------------------------- an SMTP server that accepts mail

struct Smtp { port: u16, got: Arc<Mutex<Vec<String>>> }
fn smtp() -> Smtp {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let got = Arc::new(Mutex::new(Vec::new()));
    let g = got.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let g = g.clone();
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                let mut r = BufReader::new(s);
                let _ = w.write_all(b"220 accept.test ESMTP\r\n");
                let mut line = String::new();
                loop {
                    line.clear();
                    if r.read_line(&mut line).unwrap_or(0) == 0 { return; }
                    let up = line.trim_end().to_ascii_uppercase();
                    let reply: &[u8] = if up.starts_with("EHLO") || up.starts_with("HELO") { b"250-accept.test\r\n250-AUTH PLAIN LOGIN\r\n250 8BITMIME\r\n" }
                        else if up.starts_with("AUTH") { b"235 2.7.0 ok\r\n" }
                        else if up.starts_with("DATA") {
                            let _ = w.write_all(b"354 go\r\n");
                            let mut body = String::new();
                            loop { let mut l2 = String::new(); if r.read_line(&mut l2).unwrap_or(0) == 0 { return; } if l2 == ".\r\n" { break; } body.push_str(&l2); }
                            g.lock().unwrap().push(body);
                            b"250 2.0.0 queued\r\n"
                        }
                        else if up.starts_with("QUIT") { let _ = w.write_all(b"221 bye\r\n"); return; }
                        else { b"250 ok\r\n" };
                    if w.write_all(reply).is_err() { return; }
                }
            });
        }
    });
    Smtp { port, got }
}

/// The IMAP fake with `n` INBOX messages and the account added through the core; the sync is listening when it returns.
fn added(n: u32, without_archive: bool) -> (RustCore, String, FakeImap, Smtp, tokio::runtime::Runtime, tempfile::TempDir) { added_with(n, without_archive, |_| {}) }
/// `setup` runs on the fake BEFORE the account is added, so the first sync reads what it put in the other folders.
fn added_with(n: u32, without_archive: bool, setup: impl FnOnce(&FakeImap)) -> (RustCore, String, FakeImap, Smtp, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    if without_archive { imap.with(|s| s.boxes.retain(|b| b.name != "Archive")); }
    imap.seed("INBOX", n);
    setup(&imap);
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    (core, id, imap, smtp, rt, dir)
}
fn by_subject(core: &RustCore, s: &str) -> InboxRow { core.inbox(tab("primary")).into_iter().find(|r| r.subject == s).unwrap_or_else(|| panic!("no row {s}")) }
fn uid_of(core: &RustCore, id: &str) -> u32 { core.store().write({ let id = id.to_string(); move |c| Ok(c.tx.query_row("SELECT remote_id FROM message WHERE id = ?1", [&id], |r| r.get::<_, String>(0))?) }).unwrap().parse().unwrap() }
fn box_of(imap: &FakeImap, name: &str) -> usize { imap.with(|s| s.boxes.iter().find(|b| b.name == name).map(|b| b.msgs.len()).unwrap_or(0)) }
/// `wait_until`, and when it never happens the rows still waiting for the server (with its words) and the server's own log of moves.
fn wait_with(core: &RustCore, imap: &FakeImap, what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    let rows: Vec<String> = core.store().pending_actions().unwrap().into_iter().map(|r| format!("{} {:?} {} uid {:?} {} tries {} last_error {:?}", r.kind, r.message_id, r.folder, r.uid, r.payload_json, r.tries, r.last_error)).collect();
    let moves: Vec<String> = imap.log().into_iter().filter(|l| l.contains("MOVE") || l.contains("COPY") || l.contains("STORE")).collect();
    panic!("never reached within {WAIT:?}: {what}\n pending rows: {rows:#?}\n server commands: {moves:#?}");
}
fn id_of(core: &RustCore, subject: &'static str) -> String {
    core.store().write(move |c| Ok(c.tx.query_row("SELECT id FROM message WHERE subject = ?1", [subject], |r| r.get::<_, String>(0))?)).unwrap_or_else(|e| panic!("no stored message {subject}: {e:?}"))
}
fn folder_of(core: &RustCore, id: &str) -> String {
    let id = id.to_string();
    core.store().write(move |c| Ok(c.tx.query_row("SELECT folder FROM message WHERE id = ?1", [&id], |r| r.get::<_, String>(0))?)).unwrap()
}
/// One raw message in a mailbox of the fake (its own Message-ID, so it is no copy of an INBOX message).
fn deliver(imap: &FakeImap, mbox: &str, n: u32) -> u32 {
    let raw = format!("From: Spammer {n} <s{n}@junk.example>\r\nTo: {USER}\r\nSubject: Junk offer {n}\r\nMessage-ID: <junk-{n}@junk.example>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\nBuy now {n}.\r\n").into_bytes();
    imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(imap_fakes::Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
        uid
    })
}

fn subjects(imap: &FakeImap, name: &str) -> Vec<String> {
    imap.with(|s| s.boxes.iter().find(|b| b.name == name).unwrap().msgs.iter().map(|m| String::from_utf8_lossy(&m.raw).lines().find(|l| l.starts_with("Subject:")).unwrap_or("").trim_start_matches("Subject: ").to_string()).collect())
}
fn stored(core: &RustCore, subject: &'static str) -> bool {
    core.store().write(move |c| Ok(c.tx.query_row("SELECT count(*) FROM message WHERE subject = ?1", [subject], |r| r.get::<_, i64>(0))? == 1)).unwrap()
}

// ---------------------------------------------------------------- passing: the cases whose server copy the sync has seen

#[test]
fn unarchive_moves_a_message_the_server_holds_in_archive_back_to_the_servers_inbox() {
    // two messages already in the server's Archive when the account is added: the sync files them under "Archive" (their own UIDs 1 and 2 there)
    let (core, _id, imap, _smtp, _rt, _d) = added_with(1, false, |imap| { deliver(imap, "Archive", 5); deliver(imap, "Archive", 6); });
    wait_with(&core, &imap, "the sync stores both Archive messages", || stored(&core, "Junk offer 5") && stored(&core, "Junk offer 6"));
    // the one at Archive UID 1: the same number as the INBOX message's UID (UIDs are per folder), the case a careless move trips on
    let id5 = id_of(&core, "Junk offer 5");
    assert_eq!(folder_of(&core, &id5), "Archive");
    core.run(Command::Unarchive { message_ids: vec![id5.clone()] }).expect("unarchive");
    wait_with(&core, &imap, "the first one is in the server's INBOX and gone from Archive", || box_of(&imap, "Archive") == 1 && box_of(&imap, "INBOX") == 2);
    assert_eq!(subjects(&imap, "Archive"), vec!["Junk offer 6".to_string()], "only the other one stays archived");
    assert!(subjects(&imap, "INBOX").contains(&"Junk offer 5".to_string()), "{:?}", subjects(&imap, "INBOX"));
    assert_eq!(folder_of(&core, &id5), "INBOX");
    wait_with(&core, &imap, "the row is taken", || core.store().pending_actions().unwrap().is_empty());
}
// mutant "unarchive-never-moves": core/crates/engine/src/unarchive.rs, the `if !to_inbox.is_empty() { Ledger.execute(... WriteBack::MoveToFolder ...)? }` block is deleted.

#[test]
fn mark_not_spam_moves_a_message_the_server_itself_holds_in_junk_back_to_the_inbox() {
    // two in Junk (UIDs 1 and 2); the INBOX holds one message (UID 1). The one with UID 2 clashes with nothing in INBOX.
    let (core, _id, imap, _smtp, _rt, _d) = added_with(1, false, |imap| { deliver(imap, "Junk", 7); deliver(imap, "Junk", 8); });
    wait_with(&core, &imap, "the sync stores the Junk messages", || stored(&core, "Junk offer 7") && stored(&core, "Junk offer 8"));
    let id = id_of(&core, "Junk offer 8");
    assert_eq!(folder_of(&core, &id), "Junk", "the sync filed it under the server's own Junk folder");
    core.run(Command::MarkNotSpam { message_ids: vec![id.clone()] }).expect("not spam");
    wait_with(&core, &imap, "it is in the server's INBOX and gone from Junk", || box_of(&imap, "Junk") == 1 && box_of(&imap, "INBOX") == 2);
    assert!(subjects(&imap, "INBOX").contains(&"Junk offer 8".to_string()));
    assert_eq!(subjects(&imap, "Junk"), vec!["Junk offer 7".to_string()], "only the other one stays in Junk");
    assert_eq!(folder_of(&core, &id), "INBOX");
    wait_with(&core, &imap, "the row is taken", || core.store().pending_actions().unwrap().is_empty());
}
// mutant "not-spam-never-moves": core/crates/engine/src/junk_filter.rs `mark_not_spam`, the `rows.extend(pending_rows(&WriteBack::MoveToFolder { ... }))` line is deleted.

// ---------------------------------------------------------------- the core's own moves, then the way back (the findings of engine-act-r4, fixed in engine-act-r5)
// What the owner meets: E (archive) or the junk key, then the way back a moment later. The core commits the local change and the server MOVE,
// but keeps the message under its OLD local place: folder INBOX with archived = 1 (or folder "Spam"), remote id = the INBOX UID, and the sync
// never re-reads Archive or Junk to learn the new place. The way back then names the wrong place on the server (see the notes in docs/lanes/engine-act-r4.md).

#[test]
fn finding_mark_not_spam_of_a_junk_message_with_the_same_uid_as_an_inbox_message_is_refused_in_database_words() {
    // UIDs are per folder: Junk UID 1 and INBOX UID 1 are two messages. Unarchive clears the moved message's UID; mark_not_spam does not.
    let (core, _id, imap, _smtp, _rt, _d) = added_with(1, false, |imap| { deliver(imap, "Junk", 7); });
    wait_with(&core, &imap, "the sync stores the Junk message", || stored(&core, "Junk offer 7"));
    let id = id_of(&core, "Junk offer 7");
    core.run(Command::MarkNotSpam { message_ids: vec![id.clone()] }).expect("not spam");
    wait_with(&core, &imap, "it is in the server's INBOX and gone from Junk", || box_of(&imap, "Junk") == 0 && box_of(&imap, "INBOX") == 2);
}

#[test]
fn finding_archive_then_unarchive_by_the_core_leaves_the_server_copy_in_archive() {
    let (core, _id, imap, _smtp, _rt, _d) = added(3, false);
    let (m1, m3) = (by_subject(&core, "Message 1"), by_subject(&core, "Message 3"));
    // two archived in turn: their Archive UIDs (1 and 2) differ from their INBOX UIDs (3 and 1), so a move that names the wrong UID is seen
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive m3");
    wait_with(&core, &imap, "m3 moved to Archive", || box_of(&imap, "Archive") == 1);
    core.run(Command::Archive { message_ids: vec![m1.message_id.clone()] }).expect("archive m1");
    wait_with(&core, &imap, "m1 moved to Archive", || box_of(&imap, "Archive") == 2);
    wait_with(&core, &imap, "the archive rows are taken", || core.store().pending_actions().unwrap().is_empty());
    core.run(Command::Unarchive { message_ids: vec![m3.message_id.clone()] }).expect("unarchive m3");
    wait_with(&core, &imap, "m3 is back in the server's INBOX", || box_of(&imap, "Archive") == 1 && box_of(&imap, "INBOX") == 2);
    assert!(subjects(&imap, "INBOX").contains(&"Message 3".to_string()), "INBOX {:?}", subjects(&imap, "INBOX"));
    assert_eq!(subjects(&imap, "Archive"), vec!["Message 1".to_string()], "only the one still archived stays");
}

#[test]
fn finding_mark_spam_then_mark_not_spam_by_the_core_leaves_the_server_copy_in_junk() {
    let (core, _id, imap, _smtp, _rt, _d) = added(3, false);
    let (m1, m2) = (by_subject(&core, "Message 1"), by_subject(&core, "Message 2"));
    core.run(Command::MarkSpam { message_ids: vec![m2.message_id.clone()] }).expect("spam m2");
    core.run(Command::MarkSpam { message_ids: vec![m1.message_id.clone()] }).expect("spam m1");
    wait_with(&core, &imap, "both are in the server's Junk", || box_of(&imap, "Junk") == 2 && box_of(&imap, "INBOX") == 1);
    wait_with(&core, &imap, "the spam rows are taken", || core.store().pending_actions().unwrap().is_empty());
    core.run(Command::MarkNotSpam { message_ids: vec![m2.message_id.clone()] }).expect("not spam m2");
    wait_with(&core, &imap, "m2 is back in the server's INBOX", || box_of(&imap, "Junk") == 1 && box_of(&imap, "INBOX") == 2);
    assert_eq!(subjects(&imap, "Junk"), vec!["Message 1".to_string()], "only the message still marked spam stays in Junk");
}

// ---------------------------------------------------------------- engine-act-r5: more ways the place matters

#[test]
fn z_after_the_cores_own_archive_puts_the_server_copy_back_in_the_inbox() {
    // Z reverses the flag locally (the row never left INBOX); the server's copy is in Archive under another UID, and the way back finds it there
    let (core, _id, imap, _smtp, _rt, _d) = added(3, false);
    let m3 = by_subject(&core, "Message 3");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive m3");
    wait_with(&core, &imap, "m3 moved to Archive and the row is taken", || box_of(&imap, "Archive") == 1 && core.store().pending_actions().unwrap().is_empty());
    core.run(Command::Undo).expect("z");
    wait_with(&core, &imap, "m3 is back in the server's INBOX", || box_of(&imap, "Archive") == 0 && box_of(&imap, "INBOX") == 3);
    assert!(subjects(&imap, "INBOX").contains(&"Message 3".to_string()), "INBOX {:?}", subjects(&imap, "INBOX"));
    wait_with(&core, &imap, "the row is taken", || core.store().pending_actions().unwrap().is_empty());
}
// mutant "way-back-ignores-the-recorded-place": writeback.rs `resolved` answers its input unchanged.

#[test]
fn archive_then_unarchive_at_once_leaves_the_server_copy_in_the_inbox() {
    // the second command is queued before the first move is applied: the way back is aimed when it is applied, after the move recorded its place
    let (core, _id, imap, _smtp, _rt, _d) = added(3, false);
    let m3 = by_subject(&core, "Message 3");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive m3");
    core.run(Command::Unarchive { message_ids: vec![m3.message_id.clone()] }).expect("unarchive m3");
    wait_with(&core, &imap, "both rows are taken", || core.store().pending_actions().unwrap().is_empty());
    wait_with(&core, &imap, "m3 is in the server's INBOX and Archive is empty", || box_of(&imap, "Archive") == 0 && box_of(&imap, "INBOX") == 3);
    assert!(subjects(&imap, "INBOX").contains(&"Message 3".to_string()));
}

#[test]
fn a_server_that_names_no_place_keeps_the_rows_place_and_loses_nothing() {
    // UIDPLUS is optional: with no COPYUID answer the archive still moves the server copy and the row stays as the core wrote it
    // (folder INBOX, archived); the way back is the flag aimed at INBOX as before (the sync's re-read is the fallback), and no message is lost
    let (core, _id, imap, _smtp, _rt, _d) = added(3, false);
    imap.with(|s| s.copyuid = false);
    let m3 = by_subject(&core, "Message 3");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive m3");
    wait_with(&core, &imap, "m3 moved to Archive and the row is taken", || box_of(&imap, "Archive") == 1 && core.store().pending_actions().unwrap().is_empty());
    assert_eq!(folder_of(&core, &m3.message_id), "INBOX", "no place named: the row keeps the place the core wrote");
    assert_eq!(core.store().message_place(&m3.message_id).unwrap(), None);
    core.run(Command::Unarchive { message_ids: vec![m3.message_id.clone()] }).expect("unarchive m3");
    wait_with(&core, &imap, "the row is taken", || core.store().pending_actions().unwrap().is_empty());
    assert_eq!(box_of(&imap, "Archive"), 1, "without a recorded place the server copy is not found (the sync's re-read is the fallback)");
    assert!(stored(&core, "Message 3"), "the message is still stored");
}
