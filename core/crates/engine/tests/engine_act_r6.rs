//! engine-act-r6: the Archive view (G E) lists archived mail at once, from the local store; ⇧E on its row returns the message to INBOX locally and
//! on the (fake) server; Not spam on the Junk view's row moves a junk message back to INBOX and the junk filter learns "not junk". Every read is the
//! core's own `inbox(query)` (the list the app shows); the server side is the fake's boxes, never the core's pending rows. Nothing leaves this Mac.
//! The single-edit mutant each test was seen red on is named under it (docs/lanes/engine-act-r6.md, logs in _captures/engine-act-r6).
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::{JunkLesson, NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);
/// The owner's bar: "The objective is <50ms for any operation" (docs/CONSTRAINTS.md). A view read is an operation.
const OPERATION_MS: u128 = 50;

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn mailbox(m: Mailbox) -> InboxQuery { InboxQuery { mailbox: Some(m), ..tab("primary") } }
fn subjects_of(rows: &[InboxRow]) -> Vec<String> { rows.iter().map(|r| r.subject.clone()).collect() }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16) -> AccountConfig {
    // no SMTP host (the sign-in check skips SMTP): nothing in these tests sends
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 25,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// The IMAP fake with `n` INBOX messages and the account added through the core; the sync is listening when it returns.
fn added(n: u32) -> (RustCore, FakeImap, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", n);
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    (core, imap, rt, dir)
}
fn by_subject(core: &RustCore, s: &str) -> InboxRow { core.inbox(tab("primary")).into_iter().find(|r| r.subject == s).unwrap_or_else(|| panic!("no inbox row {s}")) }
fn box_subjects(imap: &FakeImap, name: &str) -> Vec<String> {
    imap.with(|s| s.boxes.iter().find(|b| b.name == name).unwrap().msgs.iter().map(|m| String::from_utf8_lossy(&m.raw).lines().find(|l| l.starts_with("Subject:")).unwrap_or("").trim_start_matches("Subject: ").to_string()).collect())
}
fn settled(core: &RustCore) -> bool { core.store().pending_actions().unwrap().is_empty() }
/// What the next sync of a server folder stores for the copy a core move put there: a row of its own (its id is the sync's, from the folder and
/// the UID), in that folder, under the UID the server gave it. The sync's own insert (`insert_messages`), not a hand-written row.
fn sync_stores_the_server_copy(core: &RustCore, of: &str) -> String {
    let place = core.store().message_place(of).unwrap().expect("the move recorded where the server put the copy");
    let mut m = core.store().message(of).unwrap().unwrap();
    m.id = format!("{}-{}-{}", m.account_id, place.folder, place.uid);
    m.folder = place.folder.clone();
    m.archived = false;
    let id = m.id.clone();
    let n = core.store().insert_messages(vec![NewMessage { message: m, remote_id: Some(place.uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None }]).unwrap();
    assert_eq!(n, 1, "the copy is a new row, as the sync stores it");
    id
}
/// Above this load (the first figure of `sysctl -n vm.loadavg`), a timing is printed and NOT JUDGED: the other lanes' builds on this
/// Mac add their wait to every read, and the real verdict on speed is the lead's run on a quiet machine (lead's ruling, round 4).
const QUIET_LOAD: f64 = 12.0;
fn machine_load() -> Option<f64> {
    // "{ 25,30 33,98 32,84 }" on a French Mac: the decimal comma is read as a point
    let out = std::process::Command::new("/usr/sbin/sysctl").args(["-n", "vm.loadavg"]).env("LC_ALL", "C").output().ok()?;
    String::from_utf8_lossy(&out.stdout).split_whitespace().find_map(|w| w.replace(',', ".").parse::<f64>().ok())
}
/// The owner's bar on 5 reads: the MEDIAN against OPERATION_MS (a best would hide the slow reads the owner feels), best, median and worst
/// printed. Above QUIET_LOAD the line says NOT JUDGED and nothing is asserted.
fn judge_speed(label: &str, mut ms: Vec<u128>) {
    ms.sort();
    let (best, median, worst) = (ms[0], ms[ms.len() / 2], ms[ms.len() - 1]);
    let load = machine_load();
    let quiet = load.is_some_and(|l| l <= QUIET_LOAD);
    let verdict = if !quiet { "NOT JUDGED (machine load above 12, or unreadable)".to_string() } else if median < OPERATION_MS { "PASS".into() } else { "FAIL".into() };
    eprintln!("{label}: best {best} ms, median {median} ms, worst {worst} ms of {} reads; load {}; bar median < {OPERATION_MS} ms: {verdict}", ms.len(), load.map(|l| format!("{l:.2}")).unwrap_or_else(|| "unreadable".into()));
    if quiet { assert!(median < OPERATION_MS, "{label}: median {median} ms (best {best}, worst {worst})"); }
}
/// A view read, timed: the rows and how long the core took.
fn timed(core: &RustCore, q: InboxQuery) -> (Vec<InboxRow>, u128) {
    let t0 = Instant::now();
    let rows = core.inbox(q);
    (rows, t0.elapsed().as_millis())
}

// ---------------------------------------------------------------- 1. archive, then the Archive view lists it at once

#[test]
fn archive_then_the_archive_view_lists_it_at_once_and_once_after_the_next_sync_of_archive() {
    let (core, imap, _rt, _d) = added(3);
    let m3 = by_subject(&core, "Message 3");
    assert!(core.inbox(mailbox(Mailbox::Archive)).is_empty(), "nothing is archived yet");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive");
    // at once: no wait for the server, no sync of the Archive folder
    let (rows, _) = timed(&core, mailbox(Mailbox::Archive));
    assert_eq!(subjects_of(&rows), vec!["Message 3".to_string()], "the Archive view lists the archived message the moment E returns");
    assert_eq!(rows[0].message_id, m3.message_id, "the row is the message the owner archived (⇧E acts on this id)");
    judge_speed("archive view after E", (0..5).map(|_| timed(&core, mailbox(Mailbox::Archive)).1).collect());
    assert!(!subjects_of(&core.inbox(tab("primary"))).contains(&"Message 3".to_string()), "and it left the inbox");
    // the server moves its copy; the next sync of Archive stores that copy as a row of its own
    wait_until("the server holds it in Archive", || box_subjects(&imap, "Archive") == vec!["Message 3".to_string()] && settled(&core));
    let copy = sync_stores_the_server_copy(&core, &m3.message_id);
    let rows = core.inbox(mailbox(Mailbox::Archive));
    assert_eq!(subjects_of(&rows), vec!["Message 3".to_string()], "one row, not the message and its server copy: {:?}", rows.iter().map(|r| &r.message_id).collect::<Vec<_>>());
    assert_ne!(rows[0].message_id, copy, "the listed row is still the owner's, the one whose way back is recorded");
}
// mutant "archive-view-reads-folders-only": reads.rs, `let flagged = (role == FolderRole::Archive).then_some(in_scope.as_slice());` -> `let flagged = None;` (red: the view is empty after E)
// mutant "server-copy-listed-twice": realcore.rs `mailbox_rows`, the NOT EXISTS clause's `l.message_id <> m.id` -> `l.message_id = m.id` (red: two rows after the sync)

// ---------------------------------------------------------------- 2. ⇧E on the Archive view's row: back in INBOX here and on the server

#[test]
fn shift_e_on_the_archive_views_row_returns_it_to_the_inbox_locally_and_on_the_server() {
    let (core, imap, _rt, _d) = added(3);
    let m3 = by_subject(&core, "Message 3");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive");
    wait_until("the server holds it in Archive", || box_subjects(&imap, "Archive") == vec!["Message 3".to_string()] && settled(&core));
    sync_stores_the_server_copy(&core, &m3.message_id);
    // the owner opens the Archive view and presses ⇧E on its row (TriageKeys.markNotDone sends Unarchive with the focused row's id)
    let row = core.inbox(mailbox(Mailbox::Archive)).into_iter().next().expect("the Archive view lists the archived message");
    core.run(Command::Unarchive { message_ids: vec![row.message_id.clone()] }).expect("unarchive");
    // locally, at once
    assert!(subjects_of(&core.inbox(tab("primary"))).contains(&"Message 3".to_string()), "back in the inbox the moment ⇧E returns");
    assert!(core.inbox(mailbox(Mailbox::Archive)).is_empty(), "and gone from the Archive view: {:?}", subjects_of(&core.inbox(mailbox(Mailbox::Archive))));
    // on the server
    wait_until("the server holds it in INBOX again", || box_subjects(&imap, "Archive").is_empty() && box_subjects(&imap, "INBOX").contains(&"Message 3".to_string()) && settled(&core));
    assert_eq!(box_subjects(&imap, "INBOX").len(), 3, "INBOX {:?}", box_subjects(&imap, "INBOX"));
    // after the way back is applied (the recorded place is dropped), the stored server copy is not listed either: it was the same message
    assert!(core.inbox(mailbox(Mailbox::Archive)).is_empty(), "no copy left on the Archive view after the move back: {:?}", subjects_of(&core.inbox(mailbox(Mailbox::Archive))));
}
// mutant "server-copy-listed-twice" (above): red, the Archive view still lists the server copy after ⇧E
// mutant "copy-kept-after-the-way-back": rust_core.rs Unarchive arm, the `self.store.forget_server_copies(&message_ids)` line deleted (red: the copy is listed once the move back is applied)
// mutant "archive-view-reads-folders-only" (above): red, the Archive view is empty after the sync (the copy is the owner's row's, and that row is not read), so ⇧E has no row

// ---------------------------------------------------------------- 2b. Z after a core archive: the same way back

#[test]
fn z_after_a_core_archive_brings_it_back_and_drops_the_stored_server_copy() {
    let (core, imap, _rt, _d) = added(3);
    let m3 = by_subject(&core, "Message 3");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive");
    wait_until("the server holds it in Archive", || box_subjects(&imap, "Archive") == vec!["Message 3".to_string()] && settled(&core));
    sync_stores_the_server_copy(&core, &m3.message_id);
    assert_eq!(subjects_of(&core.inbox(mailbox(Mailbox::Archive))), vec!["Message 3".to_string()], "one row before Z");
    core.run(Command::Undo).expect("z");
    assert!(subjects_of(&core.inbox(tab("primary"))).contains(&"Message 3".to_string()), "back in the inbox the moment Z returns");
    assert!(core.inbox(mailbox(Mailbox::Archive)).is_empty(), "and no copy on the Archive view: {:?}", subjects_of(&core.inbox(mailbox(Mailbox::Archive))));
    wait_until("the server holds it in INBOX again", || box_subjects(&imap, "Archive").is_empty() && box_subjects(&imap, "INBOX").contains(&"Message 3".to_string()) && settled(&core));
    assert!(core.inbox(mailbox(Mailbox::Archive)).is_empty(), "still none once the move back is applied: {:?}", subjects_of(&core.inbox(mailbox(Mailbox::Archive))));
}
// mutant "copy-kept-after-z": rust_core.rs Undo arm, `self.store.forget_server_copies_of_undo(&e)` -> `Ok::<usize, store::StoreError>(0)` (red: the copy is listed after Z)

// ---------------------------------------------------------------- 3. Not spam: back to INBOX on the server, and the filter learns "not junk"

#[test]
fn not_spam_on_the_junk_views_row_moves_it_to_the_inbox_and_records_the_learn() {
    let (core, imap, _rt, _d) = added(3);
    let m2 = by_subject(&core, "Message 2");
    core.run(Command::MarkSpam { message_ids: vec![m2.message_id.clone()] }).expect("spam");
    assert_eq!(core.store().junk_lesson(&m2.message_id).unwrap(), Some(JunkLesson::Junk));
    wait_until("the server holds it in Junk", || box_subjects(&imap, "Junk") == vec!["Message 2".to_string()] && settled(&core));
    // the next sync of Junk stores the server copy as a row of its own: the Junk view still lists the message once
    sync_stores_the_server_copy(&core, &m2.message_id);
    let good_before = core.store().junk_model().unwrap().map(|m| m.trained_good).unwrap_or(0);
    // the owner opens the Junk view (G ⇧!) and runs the palette's Not spam on its row
    let (rows, _) = timed(&core, mailbox(Mailbox::Junk));
    assert_eq!(subjects_of(&rows), vec!["Message 2".to_string()], "the Junk view lists it from the store");
    judge_speed("junk view", (0..5).map(|_| timed(&core, mailbox(Mailbox::Junk)).1).collect());
    core.run(Command::MarkNotSpam { message_ids: vec![rows[0].message_id.clone()] }).expect("not spam");
    // locally, at once: in the inbox, out of Junk, the lesson is "not junk"
    assert!(subjects_of(&core.inbox(tab("primary"))).contains(&"Message 2".to_string()), "back in the inbox the moment Not spam returns");
    assert!(core.inbox(mailbox(Mailbox::Junk)).is_empty());
    assert_eq!(core.store().junk_lesson(&m2.message_id).unwrap(), Some(JunkLesson::NotJunk), "the junk filter learned 'not junk'");
    let good_after = core.store().junk_model().unwrap().map(|m| m.trained_good).unwrap_or(0);
    assert!(good_after > good_before, "the model counts one more good message ({good_before} -> {good_after})");
    // on the server
    wait_until("the server holds it in INBOX again", || box_subjects(&imap, "Junk").is_empty() && box_subjects(&imap, "INBOX").contains(&"Message 2".to_string()) && settled(&core));
    assert!(core.inbox(mailbox(Mailbox::Junk)).is_empty(), "no copy left on the Junk view after the move back: {:?}", subjects_of(&core.inbox(mailbox(Mailbox::Junk))));
}
// mutant "copy-kept-after-not-spam": rust_core.rs MarkNotSpam arm, the `self.store.forget_server_copies(&message_ids)` line deleted (red: the Junk copy is listed once the move back is applied)
// mutant "not-spam-learns-junk": rust_core.rs MarkNotSpam arm, `store::JunkLesson::NotJunk` -> `store::JunkLesson::Junk` (red: the lesson stays Junk)
// mutant "not-spam-never-moves-the-server-copy": actions/src/usecases/spam.rs `restore_from_spam`, `groups: groups_by_account(&placed, ..)` -> `groups: vec![]` (red: Junk keeps it)

// ---------------------------------------------------------------- the owner's bar on a mailbox of real size

#[test]
fn the_archive_view_answers_under_the_operation_bar_on_a_large_store() {
    // 20,000 inbox rows of which the oldest 3 are archived (the walk's worst case: the flag is rare), and 5,000 rows in the Archive folder
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1), 0).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let row = |i: u32, folder: &str, archived: bool| NewMessage {
        message: Message {
            id: format!("{folder}-{i}"), thread_id: format!("t-{folder}-{i}"), account_id: account.clone(), sender_name: "Sender".into(), from_address: "s@example.test".into(),
            to: vec![USER.into()], cc: vec![], bcc: vec![], subject: format!("{folder} {i}"), preview: String::new(), received_at_ms: 1_000_000 + i as i64 * 1000, read: true,
            archived, folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, is_outgoing: false,
        },
        remote_id: Some(i.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    };
    let inbox: Vec<NewMessage> = (1..=20_000).map(|i| row(i, "INBOX", i <= 3)).collect();
    let archive: Vec<NewMessage> = (1..=5_000).map(|i| row(i, "Archive", false)).collect();
    core.store().insert_messages(inbox).unwrap();
    core.store().insert_messages(archive).unwrap();
    let mut ms = Vec::new();
    for _ in 0..5 {
        let (rows, t) = timed(&core, mailbox(Mailbox::Archive));
        assert_eq!(rows.len(), store::INBOX_PAGE_ROWS);
        ms.push(t);
    }
    let (rows, _) = timed(&core, mailbox(Mailbox::Archive));
    assert!(rows.iter().all(|r| r.subject.starts_with("Archive")), "newest first: the Archive folder's newest rows are newer than the 3 archived inbox rows");
    judge_speed("archive view on 25,000 rows", ms);
}
