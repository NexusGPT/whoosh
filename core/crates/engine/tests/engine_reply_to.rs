//! real-replyto (docs/lanes/real-replyto.md): the real engine keeps a mail's Reply-To and answers it: `ReplyAll` goes To the Reply-To
//! addresses (else as before), a plain Reply's recipients (`message/<id>/reply_recipients`), the stored Reply-To the app's door reads
//! (`message/<id>/reply_to`), the fill-in pass that reads the Reply-To in the same fetch as the threading headers, a restart, and the time
//! on 2,000 mails. No socket is opened. Not run by this lane (no cargo here): the lead runs `cargo test --release -p engine`.
//! RED-FIRST, one broken copy per behaviour (a line of core/crates/engine/src/compose_cmds.rs unless named):
//!   B1 `recipients_for_reply`: the first `return asked;` becomes `return vec![from.to_string()];`  -> a_reply_goes_to_the_reply_to_else_to_the_sender
//!   B2 `recipients_for_reply_all`: `if asked.is_empty() {` becomes `if true {`                      -> reply_all_goes_to_the_reply_to_and_copies_every_other_to_and_cc
//!   B3 `recipients_for_reply_all`: the `.filter(|a| !asked.iter().any(|x| same(x, a)))` removed        -> several_reply_to_addresses_are_all_in_to_none_twice
//!   B4 `unique_not_own`: the `own.iter().any(...)` test removed                                       -> a_reply_to_that_is_the_owners_own_address_is_ignored
//!   B5 `reply_all`: `store.reply_to(message_id)` becomes `vec![]`                                      -> reply_all_goes_to_the_reply_to_and_copies_every_other_to_and_cc
//!   B6 `recipients_for_reply`: the last line becomes `Vec::new()`                                     -> a_mail_the_owner_wrote_is_answered_to_its_recipients_or_its_reply_to
//!   B7 thread_headers_fill.rs `fill_folder`: the `reply_to.push(...)` line removed                    -> the_fill_in_reads_the_reply_to_in_the_same_call_and_keeps_it_when_the_source_has_none
//!   B8 thread_headers_fill.rs `write_held`: the `set_reply_to_quiet(reply_to)` call removed             -> a_held_page_reply_to_is_written_without_asking_the_server
//!   B9 imap_source.rs `page`: the line `self.bodies.headers.hold_reply_to(...)` removed (the sync's page path; the live-arrival path `junk_filter::store_arrivals` has no test of its own)  -> the_synced_mail_is_stored_with_its_reply_to
//!   B10 compose_cmds.rs `state`: the `"reply_to"` arm answers `None`                                    -> the_stored_reply_to_is_served_to_the_app
//!   B11 compose_cmds.rs `reply_all`: `store.own_addresses()...?;` followed by `std::thread::sleep(std::time::Duration::from_millis(80));` -> reply_and_reply_all_answer_in_under_50_ms_on_2000_mails
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::bodies::thread_headers_fill::{ThreadHeadersFill, ThreadHeadersSource};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use send::ParentHeaders;
use store::{NewMessage, ReplyTo, Store};

const USER: &str = "owner@example.test";
const SECOND: &str = "second@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
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

/// A store with two accounts of the owner and no network.
fn local() -> (RustCore, String, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let b = store.add_account(SECOND, "Second", MailProvider::Imap, "svc\u{1f}b", false, config(1, 1), 2).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), a, b, dir)
}

fn mail(account: &str, id: &str, from: &str, to: &[&str], cc: &[&str], at: i64) -> NewMessage {
    let list = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("t-{id}"), account_id: account.into(), sender_name: "Ann Sender".into(), from_address: from.into(), to: list(to),
            subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: list(cc), bcc: vec![], is_outgoing: false,
        },
        // reds-reply: no server UID. The store keeps one row per (account, folder, remote_id) and ignores a second insert, so the same "1" on
        // every mail kept only the first mail of each account (the state answered None); `stored_in` below sets each mail's own UID.
        remote_id: None, body_text: Some(format!("Body of {id}")), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn rt(name: &str, address: &str) -> ReplyTo { ReplyTo { name: name.into(), address: address.into() } }
/// A stored mail with its Reply-To (as the sync writes it).
fn put(core: &RustCore, m: NewMessage, reply_to: Vec<ReplyTo>) {
    let id = m.message.id.clone();
    core.store().insert_messages(vec![m]).unwrap();
    core.store().set_reply_to_quiet(vec![(id, reply_to)]).unwrap();
}
fn reply_all(core: &RustCore, id: &str) -> (Vec<String>, Vec<String>) {
    let d = core.run(Command::ReplyAll { message_id: id.into() }).expect("reply all").created_id.expect("the draft id");
    let read = |leaf: &str| -> Vec<String> {
        let json = core.state(format!("draft/{d}/{leaf}")).unwrap_or_else(|| panic!("no state draft/{d}/{leaf}"));
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("{json}: {e}"))
    };
    (read("to"), read("cc"))
}
fn reply(core: &RustCore, id: &str) -> Vec<String> {
    let json = core.state(format!("message/{id}/reply_recipients")).unwrap_or_else(|| panic!("no state message/{id}/reply_recipients"));
    serde_json::from_str(&json).unwrap_or_else(|e| panic!("{json}: {e}"))
}

#[test]
fn a_reply_goes_to_the_reply_to_else_to_the_sender() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "plain", "ann@x.example", &[USER], &[], 1_000), vec![]);
    put(&core, mail(&a, "listed", "ann@x.example", &[USER], &[], 2_000), vec![rt("Dev List", "dev@lists.example.test")]);
    assert_eq!(reply(&core, "plain"), vec!["ann@x.example"], "no Reply-To: the sender");
    assert_eq!(reply(&core, "listed"), vec!["dev@lists.example.test"], "a Reply-To: its address, not the sender's");
    assert!(core.state("message/no-such/reply_recipients".into()).is_none(), "an unknown mail answers nothing");
}

#[test]
fn reply_all_goes_to_the_reply_to_and_copies_every_other_to_and_cc() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "ra", "ann@x.example", &[USER, "bob@x.example", SECOND], &["cy@x.example", "OWNER@example.test", "BOB@x.example"], 1_000), vec![rt("Dev List", "dev@lists.example.test")]);
    let (to, cc) = reply_all(&core, "ra");
    assert_eq!(to, vec!["dev@lists.example.test"], "To is the Reply-To address");
    assert_eq!(cc, vec!["bob@x.example", "cy@x.example"], "Cc is every other To and Cc address, none of the owner's two accounts, bob once");
    // without a Reply-To the rule is the contract's, unchanged (the sender, then the other To addresses; the Cc)
    put(&core, mail(&a, "rb", "ann@x.example", &[USER, "bob@x.example"], &["cy@x.example"], 2_000), vec![]);
    let (to, cc) = reply_all(&core, "rb");
    assert_eq!((to, cc), (vec!["ann@x.example".to_string(), "bob@x.example".into()], vec!["cy@x.example".to_string()]));
}

#[test]
fn several_reply_to_addresses_are_all_in_to_none_twice() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "sev", "ann@x.example", &[USER, "bob@x.example"], &["cy@x.example"], 1_000),
        vec![rt("L", "list@x.example"), rt("Bob", "Bob@X.example"), rt("dup", "LIST@x.example")]);
    let (to, cc) = reply_all(&core, "sev");
    assert_eq!(to, vec!["list@x.example", "Bob@X.example"], "every Reply-To address, in order, the case ignored for the duplicate");
    assert_eq!(cc, vec!["cy@x.example"], "bob is in To already: not copied");
    assert_eq!(reply(&core, "sev"), vec!["list@x.example", "Bob@X.example"], "a plain reply goes to all of them too");
}

#[test]
fn a_reply_to_that_is_the_owners_own_address_is_ignored() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "own", "ann@x.example", &[USER, "bob@x.example"], &["cy@x.example"], 1_000), vec![rt("Me", "Owner@Example.test")]);
    assert_eq!(reply(&core, "own"), vec!["ann@x.example"], "the owner is not answered: the sender is");
    let (to, cc) = reply_all(&core, "own");
    assert_eq!((to, cc), (vec!["ann@x.example".to_string(), "bob@x.example".into()], vec!["cy@x.example".to_string()]), "as if there were no Reply-To");
    // the owner's other account is the owner too, and a foreign address next to it still counts
    put(&core, mail(&a, "own2", "ann@x.example", &[USER], &[], 2_000), vec![rt("", SECOND), rt("", "real@x.example")]);
    assert_eq!(reply(&core, "own2"), vec!["real@x.example"]);
}

#[test]
fn a_mail_the_owner_wrote_is_answered_to_its_recipients_or_its_reply_to() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "mine", USER, &["bob@x.example", "cy@x.example"], &[], 1_000), vec![]);
    assert_eq!(reply(&core, "mine"), vec!["bob@x.example", "cy@x.example"], "the owner wrote it: the reply goes to who it went to");
    put(&core, mail(&a, "mine2", USER, &["bob@x.example"], &[], 2_000), vec![rt("", "desk@x.example")]);
    assert_eq!(reply(&core, "mine2"), vec!["desk@x.example"], "an own mail with a foreign Reply-To: that address");
}

#[test]
fn the_stored_reply_to_is_served_to_the_app() {
    let (core, a, _b, _d) = local();
    put(&core, mail(&a, "srv", "ann@x.example", &[USER], &[], 1_000), vec![rt("Karen Bosch", "karen@x.example"), rt("", "two@x.example")]);
    put(&core, mail(&a, "none", "ann@x.example", &[USER], &[], 2_000), vec![]);
    let json = core.state("message/srv/reply_to".into()).expect("the stored Reply-To");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v, serde_json::json!([{"name": "Karen Bosch", "address": "karen@x.example"}, {"name": "", "address": "two@x.example"}]), "{json}");
    assert_eq!(core.state("message/none/reply_to".into()).as_deref(), Some("[]"), "a mail with none: an empty list");
    assert!(core.state("message/no-such/reply_to".into()).is_none());
    // the contract's own field: the first Reply-To name's first word
    assert_eq!(core.store().message("srv").unwrap().unwrap().reply_to_name.as_deref(), Some("Karen"));
}

#[test]
fn the_reply_to_survives_a_restart_and_a_mail_stored_before_reads_as_none() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whoosh.sqlite");
    let a = {
        let store = Store::open(&path).unwrap();
        let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
        store.insert_messages(vec![mail(&a, "old", "ann@x.example", &[USER], &[], 1_000), mail(&a, "new", "ann@x.example", &[USER], &[], 2_000)]).unwrap();
        store.set_reply_to_quiet(vec![("new".into(), vec![rt("L", "list@x.example")])]).unwrap();
        a
    };
    let _ = a;
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert_eq!(reply(&core, "new"), vec!["list@x.example"], "kept across a restart");
    assert_eq!(reply(&core, "old"), vec!["ann@x.example"], "a mail whose Reply-To was never read goes to the sender");
}

// ---------------------------------------------------------------- the fill-in pass

struct Server { answers: HashMap<u32, (ParentHeaders, Option<Vec<(String, String)>>)>, asked: Mutex<usize> }
impl ThreadHeadersSource for Server {
    fn thread_headers_many(&self, _a: &str, _f: &str, _u: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError> { panic!("the pass must use the call that carries the Reply-To") }
    fn thread_headers_reply_to_many(&self, _a: &str, _f: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders, Option<Vec<(String, String)>>)>, CoreError> {
        *self.asked.lock().unwrap() += 1;
        Ok(uids.iter().filter_map(|u| self.answers.get(u).map(|(h, r)| (*u, h.clone(), r.clone()))).collect())
    }
}
/// A source that only knows the old call: the default of the new one answers no Reply-To.
struct OldServer;
impl ThreadHeadersSource for OldServer {
    fn thread_headers_many(&self, _a: &str, _f: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError> {
        Ok(uids.iter().map(|u| (*u, ParentHeaders { message_id: format!("<o{u}@x>"), references: vec![] })).collect())
    }
}
fn ph(id: &str) -> ParentHeaders { ParentHeaders { message_id: id.into(), references: vec![] } }
fn stored_in(dir: &tempfile::TempDir, uids: u32) -> (Arc<Store>, String) {
    let store = Arc::new(Store::open(&dir.path().join("whoosh.sqlite")).unwrap());
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages((1..=uids).map(|u| { let mut m = mail(&account, &format!("{account}:INBOX:1:{u}"), "s@ext.example", &[USER], &[], 1_000 + i64::from(u)); m.remote_id = Some(u.to_string()); m }).collect()).unwrap();
    (store, account)
}
fn runtime() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap() }

#[test]
fn the_fill_in_reads_the_reply_to_in_the_same_call_and_keeps_it_when_the_source_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored_in(&dir, 3);
    let rt_ = runtime();
    let fill = ThreadHeadersFill::new(store.clone(), rt_.handle().clone());
    let server = Arc::new(Server { answers: HashMap::from([
        (3, (ph("<m3@x>"), Some(vec![("Dev List".to_string(), "dev@lists.example.test".to_string()), (String::new(), "two@x.example".into())]))),
        (2, (ph("<m2@x>"), Some(vec![]))),
    ]), asked: Mutex::new(0) });
    fill.set_source(&account, server.clone());
    let f = fill.fill_folder(&account, "INBOX").unwrap();
    assert_eq!((f.asked, f.found, f.marked), (3, 2, 1));
    assert_eq!(*server.asked.lock().unwrap(), 1, "ONE call reads the threading headers and the Reply-To: no extra round trip");
    let id = |u: u32| format!("{account}:INBOX:1:{u}");
    assert_eq!(store.reply_to(&id(3)).unwrap(), vec![rt("Dev List", "dev@lists.example.test"), rt("", "two@x.example")]);
    assert_eq!(store.reply_to(&id(2)).unwrap(), vec![], "answered, none");
    assert_eq!(store.reply_to(&id(1)).unwrap(), vec![], "not answered: not written");
    // an old source (the call without Reply-To): the threading headers are filled and the Reply-To column is left as it was
    let dir2 = tempfile::tempdir().unwrap();
    let (store2, account2) = stored_in(&dir2, 2);
    let fill2 = ThreadHeadersFill::new(store2.clone(), rt_.handle().clone());
    store2.set_reply_to_quiet(vec![(format!("{account2}:INBOX:1:1"), vec![rt("Kept", "kept@x.example")])]).unwrap();
    fill2.set_source(&account2, Arc::new(OldServer));
    assert_eq!(fill2.fill_folder(&account2, "INBOX").unwrap().found, 2);
    assert_eq!(store2.reply_to(&format!("{account2}:INBOX:1:1")).unwrap(), vec![rt("Kept", "kept@x.example")], "a source with no Reply-To changes nothing");
    assert_eq!(store2.thread_headers(&format!("{account2}:INBOX:1:2")).unwrap().map(|h| h.message_id), Some("<o2@x>".to_string()));
}

#[test]
fn a_held_page_reply_to_is_written_without_asking_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored_in(&dir, 2);
    let rt_ = runtime();
    let fill = Arc::new(ThreadHeadersFill::new(store.clone(), rt_.handle().clone()));
    fill.hold_reply_to([(format!("{account}:INBOX:1:1"), vec![rt("L", "l@x.example")]), (format!("{account}:INBOX:1:2"), vec![])]);
    fill.kick(&account);
    assert!(fill.settle(WAIT), "the pass ends");
    wait_until("the held Reply-To is written", || store.reply_to(&format!("{account}:INBOX:1:1")).map(|r| !r.is_empty()).unwrap_or(false));
    assert_eq!(store.reply_to(&format!("{account}:INBOX:1:1")).unwrap(), vec![rt("L", "l@x.example")]);
}

// ---------------------------------------------------------------- the real sync

#[test]
fn the_synced_mail_is_stored_with_its_reply_to() {
    let rtm = tokio::runtime::Runtime::new().unwrap();
    let imap = rtm.block_on(FakeImap::start());
    imap.seed("INBOX", 0);
    imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        for (uid, extra) in [(1u32, "Reply-To: \"Dev List\" <dev@lists.example.test>, two@x.example\r\n"), (2, "")] {
            b.highest_modseq += 1;
            b.next_uid = uid + 1;
            let raw = format!("From: Ann <ann@x.example>\r\nTo: owner@example.test\r\nSubject: s{uid}\r\nMessage-ID: <s{uid}@x.example>\r\n{extra}\r\nBody.\r\n").into_bytes();
            b.msgs.push(imap_fakes::Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw, gm: None });
        }
        b.version += 1;
    });
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    // reds-reply: adding an account checks its sending server too; port 1 refused it ("Cannot reach the sending server 127.0.0.1 on port 1").
    // The loopback SMTP fake of the IMAP support module answers the check, as in realcore.rs.
    let smtp = rtm.block_on(imap_fakes::smtp::FakeSmtp::start(USER, PW));
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    let mid = |uid: u32| format!("{id}:INBOX:{}:{uid}", imap.with(|s| s.boxes.iter().find(|b| b.name == "INBOX").unwrap().uid_validity));
    wait_until("the Reply-To is stored", || core.store().reply_to(&mid(1)).map(|r| r.len() == 2).unwrap_or(false));
    assert_eq!(core.store().reply_to(&mid(1)).unwrap(), vec![rt("Dev List", "dev@lists.example.test"), rt("", "two@x.example")]);
    assert_eq!(core.store().reply_to(&mid(2)).unwrap(), vec![], "a mail with no Reply-To header");
    assert_eq!(reply(&core, &mid(1)), vec!["dev@lists.example.test", "two@x.example"], "the reply, from the store, with no server call");
}

// ---------------------------------------------------------------- the time

#[test]
fn reply_and_reply_all_answer_in_under_50_ms_on_2000_mails() {
    let (core, a, _b, _d) = local();
    let batch: Vec<NewMessage> = (0..2000).map(|i| mail(&a, &format!("bulk{i}"), "ann@x.example", &[USER, "bob@x.example"], &["cy@x.example"], 10_000 + i as i64)).collect();
    core.store().insert_messages(batch).unwrap();
    core.store().set_reply_to_quiet((0..2000).map(|i| (format!("bulk{i}"), vec![rt("List", &format!("list{i}@x.example")), rt("", "b@x.example")])).collect()).unwrap();
    let t0 = Instant::now();
    let r = reply(&core, "bulk1999");
    let reply_ms = t0.elapsed().as_millis();
    let t1 = Instant::now();
    let (to, cc) = reply_all(&core, "bulk1999");
    let all_ms = t1.elapsed().as_millis();
    let t2 = Instant::now();
    let shown = core.state("message/bulk1999/reply_to".into()).is_some();
    let state_ms = t2.elapsed().as_millis();
    println!("real-replyto timing on 2000 mails: Reply recipients {reply_ms} ms, ReplyAll {all_ms} ms, reply_to state {state_ms} ms");
    assert_eq!((r.len(), to.len(), cc.len(), shown), (2, 2, 2, true));
    assert!(reply_ms < 50 && all_ms < 50 && state_ms < 50, "Reply {reply_ms} ms, ReplyAll {all_ms} ms, state {state_ms} ms: the bar is 50 ms");
}
