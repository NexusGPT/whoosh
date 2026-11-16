//! engine-act-r4: `Command::OpenThread` in the real core (Enter on a row in the reader). Every message of the thread is marked read through
//! the actions crate's MarkRead, no undo entry is recorded, and an unknown thread is `NotFound`. Seeded store, no network: the server's
//! `\Seen` write-back is read as the `pending_action` rows it becomes. Each test was seen red with the `OpenThread` arm of
//! `RustCore::run` removed (the logs are in _captures/engine-act-r4/).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn msg(account: &str, id: &str, thread: &str, uid: &str, at: i64, read: bool) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "x".into(), from_address: "x@ext.test".into(), to: vec![USER.into()],
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.into()), body_text: Some(format!("body of {id}")), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), id, dir)
}
fn read(core: &RustCore, id: &str) -> bool { core.message(id.into()).expect("a stored message").read }
fn num(core: &RustCore, q: &'static str) -> i64 { core.store().write(move |c| Ok(c.tx.query_row(q, [], |r| r.get::<_, i64>(0))?)).unwrap() }

#[test]
fn opening_a_thread_marks_every_message_of_it_read_and_no_other_thread() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "m1", "t1", "1", 100, false), msg(&a, "m2", "t1", "2", 200, false), msg(&a, "m3", "t2", "3", 300, false)]).unwrap();
    let out = core.run(Command::OpenThread { thread_id: "t1".into() }).expect("opened");
    assert_eq!((out.undo_entry_id, out.created_id), (None, None), "an open writes no undo entry and creates nothing");
    assert_eq!((read(&core, "m1"), read(&core, "m2"), read(&core, "m3")), (true, true, false), "the thread's messages are read; another thread is not touched");
    assert_eq!(num(&core, "SELECT count(*) FROM undo_entry"), 0, "Z has nothing to undo after an open");
}
// mutant "open-thread-arm-removed": core/crates/engine/src/rust_core.rs, the `Command::OpenThread { thread_id } => self.act.open_thread(&thread_id),` line is deleted (the answer is "does not do this yet").

#[test]
fn opening_a_thread_queues_the_servers_seen_flag_for_the_messages_that_changed_only() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "m1", "t1", "1", 100, true), msg(&a, "m2", "t1", "2", 200, false)]).unwrap();
    core.run(Command::OpenThread { thread_id: "t1".into() }).expect("opened");
    let rows: Vec<(String, String)> = core.store().pending_actions().unwrap().into_iter().map(|r| (r.kind, r.message_id.unwrap_or_default())).collect();
    assert!(rows.contains(&("set_read".into(), "m2".into())), "the unread message has a server row: {rows:?}");
    assert!(!rows.iter().any(|(_, m)| m == "m1"), "the message that was already read has none: {rows:?}");
}

#[test]
fn opening_a_thread_that_is_already_read_is_ok_and_changes_nothing() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "m1", "t1", "1", 100, true)]).unwrap();
    let out = core.run(Command::OpenThread { thread_id: "t1".into() }).expect("opened");
    assert_eq!(out.undo_entry_id, None);
    assert_eq!(num(&core, "SELECT count(*) FROM pending_action"), 0, "nothing changed, nothing to write back");
}

#[test]
fn an_unknown_thread_is_not_found_in_words_and_not_the_not_built_yet_refusal() {
    let (core, a, _d) = seeded();
    core.store().insert_messages(vec![msg(&a, "m1", "t1", "1", 100, false)]).unwrap();
    match core.run(Command::OpenThread { thread_id: "nope".into() }) {
        Err(CoreError::NotFound { kind, id }) => assert_eq!((kind.as_str(), id.as_str()), ("thread", "nope")),
        other => panic!("expected NotFound thread, got {other:?}"),
    }
    assert!(!read(&core, "m1"), "nothing was marked read");
}
