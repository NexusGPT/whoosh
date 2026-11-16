//! gap-remind (docs/lanes/gap-remind.md): "Remind me if no reply" returns the thread to the INBOX list marked with the reminder. The real
//! core over a seeded store; nothing leaves this Mac. Each test names its broken copy under it (one edit, red first).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(20);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn msg(account: &str, folder: &str, id: &str, thread: &str, from: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: folder.into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
    (RustCore::with_parts(store, kc, detector(), TestTransport::PlainLoopback), id, dir)
}
fn draft(core: &RustCore, id: &'static str, thread: &'static str) {
    core.store().write(move |c| { c.tx.execute("INSERT INTO draft (id, from_account_id, reply_to_thread_id, updated_at_ms) VALUES (?1, (SELECT id FROM account LIMIT 1), ?2, 0)", rusqlite::params![id, thread])?; Ok(()) }).unwrap();
}
fn reminder_state(core: &RustCore, thread: &str) -> String { core.state(format!("thread/{thread}/reminder")).expect("a reminder read") }
fn row(core: &RustCore, id: &str) -> Option<InboxRow> { core.inbox(tab("primary")).into_iter().find(|r| r.message_id == id) }

/// Thread t2: the owner received `q1`, answered it (`s2`, with a reminder `delay_ms` ahead). Returns the sent time.
fn answered_with_reminder(core: &RustCore, account: &str, delay_ms: i64) -> i64 {
    let received = now() - 5_000;
    core.store().insert_messages(vec![msg(account, "INBOX", "q1", "t2", "them@ext.test", received)]).unwrap();
    draft(core, "d2", "t2");
    core.run(Command::RemindIfNoReply { draft_id: "d2".into(), at_ms: now() + delay_ms }).expect("set");
    let sent_at = now();
    let mut sent = msg(account, "Sent", "s2", "t2", USER, sent_at);
    sent.message.read = true;
    core.store().insert_messages(vec![sent]).unwrap();
    engine::follow_sent(&core.store(), "d2", "s2", sent_at).unwrap();
    sent_at
}

#[test]
fn a_fired_reminders_thread_sits_in_the_inbox_with_the_mark_and_a_waiting_one_has_none() {
    let (core, account, _d) = seeded();
    answered_with_reminder(&core, &account, 400);
    let early = row(&core, "q1").expect("the received mail is listed");
    assert!(!early.reminder_fired, "a reminder not yet due leaves the row unmarked");
    wait_until("the reminder is due", || reminder_state(&core, "t2").ends_with(r#""state":"fired"}"#));
    let fired = row(&core, "q1").expect("the thread is in the inbox list");
    assert!(fired.reminder_fired, "the fired reminder marks the row of its thread");
    assert_eq!(fired.thread_id, "t2");
}
// broken copy "projection-false": store/src/read.rs, in `reminder_fired_threads`, the line `out.insert(t);` is deleted (the set stays empty, no row is marked).

#[test]
fn a_reply_that_arrived_in_time_leaves_no_mark() {
    let (core, account, _d) = seeded();
    let sent_at = answered_with_reminder(&core, &account, 500);
    core.store().insert_messages(vec![msg(&account, "INBOX", "r1", "t2", "them@ext.test", sent_at + 100)]).unwrap();
    std::thread::sleep(Duration::from_millis(900));
    assert!(reminder_state(&core, "t2").ends_with(r#""state":"answered"}"#), "{}", reminder_state(&core, "t2"));
    // a list row is a conversation (13 Nov): the thread is one row, at its newest mail (the reply), and that row carries no mark
    assert!(row(&core, "q1").is_none(), "the older mail of the conversation is not a row of its own");
    assert!(!row(&core, "r1").expect("the conversation's row is its newest mail").reminder_fired, "an answered reminder marks nothing");
    assert!(core.fire_due_reminders().is_empty());
}

#[test]
fn a_thread_put_away_comes_back_to_the_inbox_when_its_reminder_fires_and_done_on_the_row_clears_it() {
    let (core, account, _d) = seeded();
    answered_with_reminder(&core, &account, 400);
    core.run(Command::Archive { message_ids: vec!["q1".into()] }).expect("done");
    assert!(row(&core, "q1").is_none(), "put away: not in the inbox yet");
    let undo_entries = || -> i64 { core.store().write(|c| Ok(c.tx.query_row("SELECT count(*) FROM undo_entry", [], |r| r.get::<_, i64>(0))?)).unwrap() };
    let before = undo_entries();
    assert!(before >= 1, "the owner's own Archive left an undo entry");
    wait_until("the reminder is due", || reminder_state(&core, "t2").ends_with(r#""state":"fired"}"#));
    assert_eq!(core.fire_due_reminders(), vec!["s2".to_string()], "the firing goes out once, and returns the thread");
    assert_eq!(undo_entries(), before, "the automatic return leaves no undo entry: Z never archives a thread the owner did not put away by that action");
    let back = row(&core, "q1").expect("the thread returned to the inbox");
    assert!(back.reminder_fired, "marked with the reminder");
    // done on the row: it leaves, and the one firing does not bring it back
    core.run(Command::Archive { message_ids: vec!["q1".into()] }).expect("done again");
    assert!(row(&core, "q1").is_none(), "done clears the row from the inbox");
    assert!(core.fire_due_reminders().is_empty());
    assert!(row(&core, "q1").is_none(), "and it stays away");
}
// broken copy "no-return": engine/src/reminders.rs, in `fire_due_and_return`, the line `if !away.is_empty() {` becomes `if false {`.
// broken copy "return-records-undo": engine/src/reminders.rs, in `fire_due_and_return`, `run_recording(store, roles, away, false)` becomes `run_recording(store, roles, away, true)`.

#[test]
fn the_inbox_read_with_a_fired_reminder_on_2000_mails_stays_under_50_ms() {
    let (core, account, _d) = seeded();
    let base = now() - 10_000_000;
    let batch: Vec<NewMessage> = (0..2000).map(|i| msg(&account, "INBOX", &format!("m{i}"), &format!("tm{i}"), "them@ext.test", base + i as i64)).collect();
    core.store().insert_messages(batch).unwrap();
    answered_with_reminder(&core, &account, 300);
    wait_until("the reminder is due", || reminder_state(&core, "t2").ends_with(r#""state":"fired"}"#));
    let mut best = u128::MAX;
    for _ in 0..5 {
        let t0 = Instant::now();
        let rows = core.inbox(tab("primary"));
        best = best.min(t0.elapsed().as_millis());
        assert!(rows.iter().any(|r| r.message_id == "q1" && r.reminder_fired));
    }
    println!("reminder-inbox read on 2002 mails: best of 5 = {best} ms");
    assert!(best < 50, "the inbox read took {best} ms (limit 50)");
}
