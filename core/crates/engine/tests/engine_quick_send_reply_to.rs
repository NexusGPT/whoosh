//! fix-replydoors: a plain quick send (`QuickSend`, the box under a thread, and its retry) is addressed to the Reply-To of the newest mail of the
//! thread that the owner did not send, else to its sender, as Reply is (compose_cmds.rs `recipients_for_reply`). No socket is opened.
//! Not run by this lane (no cargo here): the lead runs `cargo test --release -p engine --test engine_quick_send_reply_to`.
//! RED-FIRST, one broken copy per behaviour (a line of core/crates/engine/src/outbox_tick.rs `quick_reply_draft`):
//!   Q1 `if reply_to.is_empty() { vec![to] } else {` becomes `if true { vec![to] } else {`        -> a_quick_send_goes_to_the_reply_to_of_the_mail_it_answers
//!   Q2 `let reply_to = store.reply_to(&id).unwrap_or_default();` becomes `let reply_to = vec![store::ReplyTo { name: String::new(), address: "x@lists.example.test".into() }];`
//!                                                                                             -> a_mail_with_no_reply_to_is_answered_to_its_sender
//!   Q3 `let own = store.own_addresses().unwrap_or_default();` becomes `let own: Vec<String> = vec![];` -> a_reply_to_that_is_the_owners_own_address_is_ignored
//!   Q4 `let incoming = rows.iter().find(...)` becomes `rows.first()` (the Reply-To of the newest mail, even the owner's own) -> the_reply_to_read_is_the_newest_mail_from_another_person
//!   Q5 after `let own = ...unwrap_or_default();` add `std::thread::sleep(std::time::Duration::from_millis(80));` -> a_quick_send_answers_in_under_50_ms_on_2000_mails
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use contracts::*;
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, ReplyTo, Store, ThreadHeaders};

/// Each mail gets its own server UID: the queue refuses a reply to a mail with no place on the server, and the store keeps one row per
/// (account, folder, remote_id). The counter is global, so no two mails of any test share one.
static UID: AtomicU32 = AtomicU32::new(1);

const USER: &str = "owner@example.test";

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn local() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let det: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    (RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), det, TestTransport::PlainLoopback), a, dir)
}
/// One mail in thread `thread`, with its own UID.
fn mail(account: &str, id: &str, thread: &str, from: &str, outgoing: bool, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "Ann Sender".into(), from_address: from.into(), to: vec![USER.into()],
            subject: "Question".into(), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: outgoing,
        },
        remote_id: Some(UID.fetch_add(1, Ordering::SeqCst).to_string()), body_text: Some(format!("Body of {id}")), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
/// The stored threading headers of a mail (as the sync writes them): with them the queue builds the reply from the store and asks no server
/// (`StoreReplyContext::stored` answers before `parent_of` and its header fetch), so no socket is opened and no sync runs.
fn headers(core: &RustCore, id: &str) {
    core.store().set_thread_headers_quiet(vec![(id.to_string(), Some(ThreadHeaders { message_id: format!("<{id}@example.test>"), references: vec![] }))]).unwrap();
}
fn put(core: &RustCore, m: NewMessage, reply_to: Vec<(&str, &str)>) {
    let id = m.message.id.clone();
    core.store().insert_messages(vec![m]).unwrap();
    headers(core, &id);
    let rows = reply_to.into_iter().map(|(n, a)| ReplyTo { name: n.into(), address: a.into() }).collect();
    core.store().set_reply_to_quiet(vec![(id, rows)]).unwrap();
}
/// Queue a quick send on the thread and read who the outbox entry goes to.
fn quick_send_to(core: &RustCore, thread: &str) -> Vec<String> {
    let entry = core.run(Command::QuickSend { thread_id: thread.into(), body: "Thanks.".into() }).expect("queued").created_id.expect("the entry");
    core.outbox().into_iter().find(|e| e.id == entry).expect("open in the outbox").draft.to
}

#[test]
fn a_quick_send_goes_to_the_reply_to_of_the_mail_it_answers() {
    let (core, a, _d) = local();
    put(&core, mail(&a, "m1", "t1", "ann@x.example", false, 1_000), vec![("Dev List", "dev@lists.example.test")]);
    assert_eq!(quick_send_to(&core, "t1"), vec!["dev@lists.example.test"], "the Reply-To, not the sender");
}

#[test]
fn a_mail_with_no_reply_to_is_answered_to_its_sender() {
    let (core, a, _d) = local();
    put(&core, mail(&a, "m1", "t1", "ann@x.example", false, 1_000), vec![]);
    assert_eq!(quick_send_to(&core, "t1"), vec!["ann@x.example"]);
    core.store().insert_messages(vec![mail(&a, "m2", "t2", "bob@x.example", false, 2_000)]).unwrap();
    headers(&core, "m2");   // never read: NULL, as a mail stored before 0027
    assert_eq!(quick_send_to(&core, "t2"), vec!["bob@x.example"]);
}

#[test]
fn a_reply_to_that_is_the_owners_own_address_is_ignored() {
    let (core, a, _d) = local();
    put(&core, mail(&a, "m1", "t1", "ann@x.example", false, 1_000), vec![("Me", "OWNER@example.test")]);
    assert_eq!(quick_send_to(&core, "t1"), vec!["ann@x.example"], "only the owner's address in Reply-To says nothing");
    put(&core, mail(&a, "m2", "t2", "ann@x.example", false, 1_000), vec![("Me", USER), ("List", "list@lists.example.test")]);
    assert_eq!(quick_send_to(&core, "t2"), vec!["list@lists.example.test"], "the owner's address is left out, the other kept");
}

#[test]
fn the_reply_to_read_is_the_newest_mail_from_another_person() {
    let (core, a, _d) = local();
    put(&core, mail(&a, "old", "t1", "ann@x.example", false, 1_000), vec![("Old List", "old@lists.example.test")]);
    put(&core, mail(&a, "new", "t1", "bob@x.example", false, 2_000), vec![("New List", "new@lists.example.test")]);
    put(&core, mail(&a, "mine", "t1", USER, true, 3_000), vec![("Wrong", "wrong@lists.example.test")]);
    assert_eq!(quick_send_to(&core, "t1"), vec!["new@lists.example.test"]);
}

#[test]
fn a_quick_send_answers_in_under_50_ms_on_2000_mails() {
    let (core, a, _d) = local();
    let rows: Vec<NewMessage> = (0..2000).map(|i| mail(&a, &format!("bulk-{i}"), &format!("tb-{i}"), "bulk@x.example", false, 10_000 + i)).collect();
    core.store().insert_messages(rows).unwrap();
    put(&core, mail(&a, "target", "tt", "ann@x.example", false, 50_000), vec![("List", "list@lists.example.test")]);
    let t0 = Instant::now();
    let to = quick_send_to(&core, "tt");
    let ms = t0.elapsed().as_millis();
    println!("fix-replydoors quick send timing on 2000 mails: {ms} ms");
    assert_eq!(to, vec!["list@lists.example.test"]);
    assert!(ms < 50, "the quick send took {ms} ms");
}
