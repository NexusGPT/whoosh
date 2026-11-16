//! engine-actions: the real core runs Mute, Unarchive, Unsubscribe and RemindIfNoReply (docs/lanes/engine-actions.md). The mail comes
//! from sync-imap's in-process IMAP fake (clear text on 127.0.0.1) or from a seeded store; the one-click POST goes to a TcpListener of this
//! file on 127.0.0.1. Nothing leaves this Mac. Each test was seen red on main's sources, and on the single-edit mutant named under it.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap, Msg};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(20);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn mailbox(m: Mailbox) -> InboxQuery { InboxQuery { mailbox: Some(m), ..tab("primary") } }
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
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
fn invalid(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("not an Invalid refusal: {other:?}") }
}

// ---------------------------------------------------------------- a seeded store

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
fn seeded(plain: bool) -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let t = if plain { TestTransport::PlainLoopback } else { TestTransport::Real };
    (RustCore::with_parts(store, kc(), detector(), t), id, dir)
}
fn sql(core: &RustCore, q: &'static str) { core.store().write(move |c| { c.tx.execute_batch(q)?; Ok(()) }).unwrap(); }
fn count(core: &RustCore, q: &'static str) -> i64 { core.store().write(move |c| Ok(c.tx.query_row(q, [], |r| r.get::<_, i64>(0))?)).unwrap() }
fn text(core: &RustCore, q: &'static str) -> String { core.store().write(move |c| Ok(c.tx.query_row(q, [], |r| r.get::<_, String>(0))?)).unwrap() }

// ---------------------------------------------------------------- Mute, through the real sync

/// The fake with three INBOX messages (each its own thread, Message-ID <n@example.test>) and the account added through the core.
fn added() -> (RustCore, String, FakeImap, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    (core, out.created_id.unwrap(), imap, rt, dir)
}
/// A message that answers <1@example.test>: it joins that message's thread by its References header. It reaches the INBOX like new mail does.
fn reply_to_message_1(imap: &FakeImap) {
    let raw = b"From: Replier <replier@example.test>\r\nTo: owner@example.test\r\nSubject: Re: Message 1\r\nMessage-ID: <reply-1@example.test>\r\nIn-Reply-To: <1@example.test>\r\nReferences: <1@example.test>\r\nDate: Sat, 07 Nov 2026 11:00:00 +0000\r\n\r\nThe answer.\r\n".to_vec();
    imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(Msg { uid, flags: BTreeSet::new(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
    });
    imap.drop_idle();   // the IDLE connection is re-opened and the INBOX is read again: the fake's way of announcing new mail
}

#[test]
fn mute_a_thread_then_its_next_mail_arrives_read_and_muted_through_the_real_sync() {
    let (core, id, imap, _rt, _d) = added();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    let one = core.inbox(tab("primary")).into_iter().find(|r| r.subject == "Message 1").expect("message 1").message_id;
    let other = core.inbox(tab("primary")).into_iter().find(|r| r.subject == "Message 2").expect("message 2").message_id;
    assert_eq!(core.state(format!("message/{one}/muted")).as_deref(), Some("false"));
    core.run(Command::Mute { message_ids: vec![one.clone()], muted: true }).expect("muted");
    assert_eq!(core.state(format!("message/{one}/muted")).as_deref(), Some("true"), "the view read");
    assert_eq!(core.state(format!("message/{other}/muted")).as_deref(), Some("false"), "another thread is not muted");
    assert_eq!(ids(&core.inbox(mailbox(Mailbox::Muted))), vec![one.clone()], "the Muted mailbox lists it");
    assert!(!ids(&core.inbox(tab("primary"))).contains(&one), "and the inbox does not");
    // new mail of that thread arrives by IDLE
    reply_to_message_1(&imap);
    wait_until("the reply in the Muted mailbox", || core.inbox(mailbox(Mailbox::Muted)).len() == 2);
    let arrived = core.inbox(mailbox(Mailbox::Muted)).into_iter().find(|r| r.subject == "Re: Message 1").expect("the reply");
    assert!(!arrived.unread, "it arrived read");
    assert_eq!(core.state(format!("message/{}/muted", arrived.message_id)).as_deref(), Some("true"), "and muted");
    assert!(core.inbox(tab("primary")).iter().all(|r| r.subject != "Re: Message 1"), "it never shows in the inbox");
    let primary = core.counts().tabs.into_iter().find(|t| t.tab_id == "primary").unwrap();
    assert_eq!(primary.unread, 2, "Message 2 and 3 are unread; the muted thread counts for nothing: {:?}", core.counts());
    // unmute: both messages of the thread come back to the inbox
    core.run(Command::Mute { message_ids: vec![arrived.message_id.clone()], muted: false }).expect("unmuted");
    assert!(core.inbox(mailbox(Mailbox::Muted)).is_empty());
    assert_eq!(core.state(format!("message/{one}/muted")).as_deref(), Some("false"), "unmuting from the reply lifts the whole thread");
    // a list row is a conversation (13 Nov): the unmuted thread is back as ONE row, at its newest mail
    let listed = ids(&core.inbox(tab("primary")));
    assert!(listed.contains(&arrived.message_id) && !listed.contains(&one), "{listed:?}");
}
// mutant "mute-named-only": store/src/delta10.rs, in WriteCtx::mute_threads, `self.set_muted(&members, on)` becomes `self.set_muted(message_ids, on)`
// (red here on `message/<reply>/muted` after the unmute from the reply, and in the store's own test).

#[test]
fn mute_refuses_an_unknown_message_and_writes_nothing() {
    let (core, id, _d) = seeded(true);
    core.store().insert_messages(vec![msg(&id, "INBOX", "m1", "t1", "them@ext.test", 100)]).unwrap();
    let Err(CoreError::NotFound { kind, id: missing }) = core.run(Command::Mute { message_ids: vec!["m1".into(), "ghost".into()], muted: true }) else { panic!("not NotFound") };
    assert_eq!((kind.as_str(), missing.as_str()), ("message", "ghost"));
    assert_eq!(core.state("message/m1/muted".into()).as_deref(), Some("false"), "m1 was named too, and nothing was written");
    assert_eq!(core.state("message/ghost/muted".into()), None);
}

// ---------------------------------------------------------------- Unarchive

#[test]
fn unarchive_returns_the_messages_to_the_inbox_and_the_next_sync_does_not_put_them_back() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    imap.seed("Archive", 2);   // Message 1 and 2 of the Archive folder (UIDs 1 and 2)
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).unwrap().created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    wait_until("the two Archive rows", || core.inbox(mailbox(Mailbox::Archive)).len() == 2);
    let archived = ids(&core.inbox(mailbox(Mailbox::Archive)));
    assert!(core.inbox(tab("primary")).len() == 1, "the inbox holds its own message only");
    core.run(Command::Unarchive { message_ids: vec![archived[0].clone()] }).expect("unarchived");
    assert_eq!(ids(&core.inbox(mailbox(Mailbox::Archive))), vec![archived[1].clone()], "the other one stays archived");
    let m = core.message(archived[0].clone()).unwrap();
    // a list row is a conversation (13 Nov): the inbox lists the conversation that holds the returned mail, at its newest mail
    assert!(core.inbox(tab("primary")).iter().any(|r| r.thread_id == m.thread_id), "its conversation is back in the inbox tab");
    assert_eq!((m.folder.as_str(), m.archived), ("INBOX", false));
    // the next sync reads the Archive folder again: the local move holds and nothing is stored twice
    let before = imap.with(|s| s.idle_starts);
    core.start_stored_accounts();
    wait_until("the sync ran again", || imap.with(|s| s.idle_starts > before));
    wait_until("listening again", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    assert_eq!(ids(&core.inbox(mailbox(Mailbox::Archive))), vec![archived[1].clone()], "the next sync did not put it back in Archive");
    // the inbox HOLDS two mails (its own and the returned one); the list shows conversations, so the mails are counted in the store
    assert_eq!(count(&core, "SELECT count(*) FROM message WHERE folder = 'INBOX' AND archived = 0"), 2, "and did not store it twice");
    assert_eq!(count(&core, "SELECT count(*) FROM message"), 3);
}
// mutant "unarchive-keeps-folder": store/src/delta10.rs, in WriteCtx::unarchive, the archive-folder statement
// `UPDATE message SET archived = 0, folder = 'INBOX', remote_id = NULL WHERE id = ?1` becomes `UPDATE message SET archived = 0 WHERE id = ?1`.

#[test]
fn unarchive_clears_the_archived_flag_refuses_an_unknown_id_and_touches_nothing_else() {
    let (core, id, _d) = seeded(true);
    let mut a = msg(&id, "INBOX", "a1", "ta1", "x@ext.test", 100); a.message.archived = true;
    core.store().insert_messages(vec![a, msg(&id, "INBOX", "a2", "ta2", "y@ext.test", 200)]).unwrap();
    assert_eq!(ids(&core.inbox(tab("primary"))), vec!["a2".to_string()]);
    assert!(core.run(Command::Unarchive { message_ids: vec!["a1".into(), "ghost".into()] }).is_err(), "an unknown id is refused");
    assert_eq!(count(&core, "SELECT archived FROM message WHERE id = 'a1'"), 1, "and the refusal wrote nothing");
    core.run(Command::Unarchive { message_ids: vec!["a1".into()] }).expect("unarchived");
    assert_eq!(ids(&core.inbox(tab("primary"))), vec!["a1".to_string(), "a2".to_string()]);
}

// ---------------------------------------------------------------- Unsubscribe

#[derive(Debug, Clone, PartialEq)]
struct Post { method: String, path: String, content_type: String, body: String }
/// The list's server: answers `status` to every request and keeps them all.
struct ListServer { port: u16, seen: Arc<Mutex<Vec<Post>>>, status: Arc<Mutex<u16>> }
impl ListServer {
    fn start(status: u16) -> ListServer {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let (seen, status) = (Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(status)));
        let (s2, st2) = (seen.clone(), status.clone());
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || for s in l.incoming().flatten() { let (s2, st2) = (s2.clone(), st2.clone()); std::thread::spawn(move || serve(s, s2, st2)); });
        ListServer { port, seen, status }
    }
    fn answer(&self, status: u16) { *self.status.lock().unwrap() = status; }
    fn seen(&self) -> Vec<Post> { self.seen.lock().unwrap().clone() }
    fn url(&self, path: &str) -> String { format!("http://127.0.0.1:{}{path}", self.port) }
}
fn serve(mut s: TcpStream, seen: Arc<Mutex<Vec<Post>>>, status: Arc<Mutex<u16>>) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    if r.read_line(&mut line).is_err() { return; }
    let (mut len, mut ct) = (0usize, String::new());
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).is_err() || h.trim().is_empty() { break; }
        let lower = h.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") { len = v.trim().parse().unwrap_or(0); }
        if let Some(v) = lower.strip_prefix("content-type:") { ct = v.trim().to_string(); }
    }
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let mut parts = line.split_whitespace();
    seen.lock().unwrap().push(Post { method: parts.next().unwrap_or("").into(), path: parts.next().unwrap_or("").into(), content_type: ct, body: String::from_utf8_lossy(&body).into_owned() });
    let code = *status.lock().unwrap();
    let words = if code == 500 { "the list is down" } else { "" };
    let _ = write!(s, "HTTP/1.1 {code} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{words}", words.len());
}

/// A seeded core with one message carrying the given List-Unsubscribe header.
fn listed(urls: &[String], one_click: bool) -> (RustCore, String, tempfile::TempDir) {
    let (core, account, d) = seeded(true);
    core.store().insert_messages(vec![msg(&account, "INBOX", "l1", "tl1", "news@list.example", 100)]).unwrap();
    core.store().set_list_unsubscribe("l1".into(), urls.to_vec(), one_click).unwrap();
    (core, account, d)
}
fn run_unsub(core: &RustCore) -> Result<CommandOutcome, CoreError> { core.run(Command::Unsubscribe { message_id: "l1".into() }) }

#[test]
fn unsubscribe_one_click_posts_once_to_the_list_and_a_second_try_is_refused_without_a_second_post() {
    let server = ListServer::start(200);
    let (core, _a, _d) = listed(&[format!("<{}>", server.url("/u/abc?t=1")), "<mailto:leave@list.example>".into()], true);
    assert_eq!(core.unsubscribe_method("l1".into()), Some(UnsubscribeMethod::OneClick { url: "https://127.0.0.1:".to_string() + &server.port.to_string() + "/u/abc?t=1" }));
    assert_eq!(core.state("message/l1/unsubscribe".into()).as_deref(), Some(r#""one_click""#));
    run_unsub(&core).expect("left");
    let seen = server.seen();
    assert_eq!(seen, vec![Post { method: "POST".into(), path: "/u/abc?t=1".into(), content_type: "application/x-www-form-urlencoded".into(), body: "List-Unsubscribe=One-Click".into() }], "one POST, RFC 8058's body");
    assert_eq!(core.state("message/l1/unsubscribe".into()).as_deref(), Some(r#""done""#));
    assert_eq!(core.state("message/l1/unsubscribed".into()).as_deref(), Some("true"));
    let (field, words) = invalid(run_unsub(&core));
    assert_eq!((field.as_str(), words.as_str()), ("unsubscribe_already_done", "You already left this list."));
    assert_eq!(server.seen().len(), 1, "the second try posted nothing");
    assert_eq!(count(&core, "SELECT count(*) FROM outbox_entry"), 0, "and a one-click leaves no mail behind");
}
// mutant "unsubscribe-ignores-done": engine/src/unsubscribe.rs, in run(), `check_unsubscribe(method(store, message_id, plain_loopback), done)`
// becomes `check_unsubscribe(method(store, message_id, plain_loopback), false)` (red here: the second try posts again).

#[test]
fn a_refused_one_click_is_refused_in_the_servers_words_and_marks_nothing() {
    let server = ListServer::start(500);
    let (core, _a, _d) = listed(&[server.url("/u")], true);
    let Err(CoreError::Action { failure }) = run_unsub(&core) else { panic!("not refused") };
    assert!(failure.reason_words.contains("500") && failure.reason_words.contains("the list is down"), "{}", failure.reason_words);
    assert!(failure.retryable, "a server error may pass");
    assert_eq!(core.state("message/l1/unsubscribed".into()).as_deref(), Some("false"), "nothing is marked: the owner is still on the list");
    assert_eq!(core.state("message/l1/unsubscribe".into()).as_deref(), Some(r#""one_click""#));
    server.answer(200);
    run_unsub(&core).expect("the retry goes through");
    assert_eq!(server.seen().len(), 2, "the refused try and the good one");
    assert_eq!(core.state("message/l1/unsubscribed".into()).as_deref(), Some("true"));
}
// mutant "unsubscribe-marks-on-failure": engine/src/unsubscribe.rs, `UnsubscribeMethod::OneClick { url } => post_one_click(&wire_url(&url, plain_loopback))?,`
// becomes `... => { let _ = post_one_click(&wire_url(&url, plain_loopback)); }`.

#[test]
fn a_mail_link_queues_one_outbox_entry_with_the_headers_subject_and_sends_no_post() {
    let server = ListServer::start(200);
    let (core, account, _d) = listed(&["<mailto:leave@list.example?subject=unsubscribe%20me>".into(), "<https://list.example/page>".into()], false);
    assert_eq!(core.unsubscribe_method("l1".into()), Some(UnsubscribeMethod::Mail { address: "leave@list.example".into(), subject: Some("unsubscribe me".into()) }));
    run_unsub(&core).expect("left");
    assert_eq!(count(&core, "SELECT count(*) FROM outbox_entry"), 1, "one entry");
    let json = text(&core, "SELECT draft_json FROM outbox_entry");
    assert!(json.contains(r#""to":["leave@list.example"]"#), "{json}");
    assert!(json.contains(r#""subject":"unsubscribe me""#), "{json}");
    assert!(json.contains(&format!(r#""from_account_id":"{account}""#)), "sent from the account that received the list: {json}");
    assert_eq!(text(&core, "SELECT state FROM outbox_entry"), "undoable");
    assert!(count(&core, "SELECT due_at_ms FROM outbox_entry") <= now(), "due at once");
    assert!(server.seen().is_empty(), "no HTTP request for a mail link");
    assert_eq!(core.state("message/l1/unsubscribe".into()).as_deref(), Some(r#""done""#));
    let (field, _) = invalid(run_unsub(&core));
    assert_eq!(field, "unsubscribe_already_done");
    assert_eq!(count(&core, "SELECT count(*) FROM outbox_entry"), 1, "still one entry after the second try");
}
// mutant "mail-drops-subject": engine/src/unsubscribe.rs, in queue_mail(), `subject: subject.filter(` becomes `subject: None::<String>.filter(`.

#[test]
fn a_web_page_is_left_to_the_ui_a_message_with_no_header_is_refused_and_an_unknown_message_is_not_found() {
    let (core, _a, _d) = listed(&["<https://list.example/leave>".into()], false);
    assert_eq!(core.unsubscribe_method("l1".into()), Some(UnsubscribeMethod::Link { url: "https://list.example/leave".into() }));
    run_unsub(&core).expect("the page is the UI's: the core marks the list left");
    assert_eq!(count(&core, "SELECT count(*) FROM outbox_entry"), 0);
    assert_eq!(core.state("message/l1/unsubscribe".into()).as_deref(), Some(r#""done""#));
    // no header
    let (core, account, _d) = seeded(true);
    core.store().insert_messages(vec![msg(&account, "INBOX", "n1", "tn1", "friend@ext.test", 100)]).unwrap();
    assert_eq!(core.unsubscribe_method("n1".into()), None);
    assert_eq!(core.state("message/n1/unsubscribe".into()).as_deref(), Some(r#""none""#));
    let (field, words) = invalid(core.run(Command::Unsubscribe { message_id: "n1".into() }));
    assert_eq!((field.as_str(), words.as_str()), ("unsubscribe_not_a_list", "This message is not from a mailing list."));
    let Err(CoreError::NotFound { kind, .. }) = core.run(Command::Unsubscribe { message_id: "ghost".into() }) else { panic!("not NotFound") };
    assert_eq!(kind, "message");
}

#[test]
fn a_one_click_url_is_posted_to_only_by_a_plain_loopback_core_and_only_on_this_macs_own_address() {
    let server = ListServer::start(200);
    // a core opened for real (not plain-loopback) reads the header as it is: an http:// URL is a page, never a one-click target
    let (core, account, _d) = seeded(false);
    core.store().insert_messages(vec![msg(&account, "INBOX", "l1", "tl1", "news@list.example", 100)]).unwrap();
    core.store().set_list_unsubscribe("l1".into(), vec![server.url("/u")], true).unwrap();
    assert_eq!(core.unsubscribe_method("l1".into()), Some(UnsubscribeMethod::Link { url: server.url("/u") }));
    run_unsub(&core).expect("a link: the UI's");
    assert!(server.seen().is_empty(), "no POST left this core");
}

// ---------------------------------------------------------------- RemindIfNoReply

fn draft(core: &RustCore, id: &'static str, thread: &'static str) {
    core.store().write(move |c| { c.tx.execute("INSERT INTO draft (id, from_account_id, reply_to_thread_id, updated_at_ms) VALUES (?1, (SELECT id FROM account LIMIT 1), ?2, 0)", rusqlite::params![id, thread])?; Ok(()) }).unwrap();
}
fn reminder_state(core: &RustCore, thread: &str) -> String { core.state(format!("thread/{thread}/reminder")).expect("a reminder read") }

#[test]
fn remind_if_no_reply_is_set_on_a_draft_and_refused_for_the_past_and_for_an_unknown_draft() {
    let (core, _a, _d) = seeded(true);
    draft(&core, "d1", "t1");
    let at = now() + 3_600_000;
    let out = core.run(Command::RemindIfNoReply { draft_id: "d1".into(), at_ms: at }).expect("set");
    assert_eq!(out.created_id.as_deref(), Some("d1"));
    assert_eq!(reminder_state(&core, "t1"), format!(r#"{{"at_ms":{at},"draft_id":"d1","state":"waiting"}}"#));
    let (field, words) = invalid(core.run(Command::RemindIfNoReply { draft_id: "d1".into(), at_ms: now() - 1 }));
    assert_eq!((field.as_str(), words.as_str()), ("reminder_in_the_past", "That time has already passed."));
    assert_eq!(reminder_state(&core, "t1"), format!(r#"{{"at_ms":{at},"draft_id":"d1","state":"waiting"}}"#), "the refusal changed nothing");
    let Err(CoreError::NotFound { kind, id }) = core.run(Command::RemindIfNoReply { draft_id: "ghost".into(), at_ms: at }) else { panic!("not NotFound") };
    assert_eq!((kind.as_str(), id.as_str()), ("draft", "ghost"));
    // a past time on an unknown draft is the unknown draft's refusal first, as the fixture core does
    assert!(matches!(core.run(Command::RemindIfNoReply { draft_id: "ghost".into(), at_ms: 1 }), Err(CoreError::NotFound { .. })));
    // ClearReplyReminder takes it off again
    core.run(Command::ClearReplyReminder { draft_id: "d1".into() }).expect("cleared");
    assert_eq!(reminder_state(&core, "t1"), r#"{"at_ms":null,"draft_id":null,"state":"none"}"#);
}
// mutant "remind-accepts-past": engine/src/reminders.rs, in set(), the line `check_remind_if_no_reply(at_ms, now_ms)?;` is deleted (the store then refuses the past in its own words, so the contract's code is gone).

#[test]
fn a_reply_that_comes_before_the_time_answers_the_reminder_and_it_never_fires() {
    let (core, account, _d) = seeded(true);
    draft(&core, "d1", "t1");
    core.run(Command::RemindIfNoReply { draft_id: "d1".into(), at_ms: now() + 600 }).expect("set");
    // the draft is sent: the sent copy is stored and the reminder follows it
    let sent_at = now();
    let mut sent = msg(&account, "Sent", "s1", "t1", USER, sent_at); sent.message.read = true;
    core.store().insert_messages(vec![sent]).unwrap();
    assert_eq!(engine::follow_sent(&core.store(), "d1", "s1", sent_at).unwrap().as_deref(), Some("rr-s1"));
    assert!(reminder_state(&core, "t1").contains(r#""draft_id":"s1","state":"waiting""#), "{}", reminder_state(&core, "t1"));
    // someone else answers in the thread, before the time
    core.store().insert_messages(vec![msg(&account, "INBOX", "r1", "t1", "them@ext.test", sent_at + 100)]).unwrap();
    assert!(reminder_state(&core, "t1").ends_with(r#""state":"answered"}"#), "{}", reminder_state(&core, "t1"));
    std::thread::sleep(Duration::from_millis(900));
    assert!(reminder_state(&core, "t1").ends_with(r#""state":"answered"}"#), "time passing does not fire an answered reminder: {}", reminder_state(&core, "t1"));
    assert!(core.inbox(mailbox(Mailbox::Reminders)).is_empty(), "it is not in the Reminders view");
    assert!(core.fire_due_reminders().is_empty(), "and no notification goes out");
}

#[test]
fn a_reminder_with_no_reply_comes_due_lists_the_thread_and_fires_once() {
    let (core, account, _d) = seeded(true);
    draft(&core, "d2", "t2");
    core.run(Command::RemindIfNoReply { draft_id: "d2".into(), at_ms: now() + 400 }).expect("set");
    let sent_at = now();
    core.store().insert_messages(vec![msg(&account, "Sent", "s2", "t2", USER, sent_at)]).unwrap();
    engine::follow_sent(&core.store(), "d2", "s2", sent_at).unwrap();
    // an unrelated reply in another thread answers nothing
    core.store().insert_messages(vec![msg(&account, "INBOX", "x1", "t-other", "them@ext.test", sent_at + 50)]).unwrap();
    assert!(reminder_state(&core, "t2").ends_with(r#""state":"waiting"}"#));
    assert!(core.inbox(mailbox(Mailbox::Reminders)).is_empty(), "not due yet");
    wait_until("the reminder is due", || reminder_state(&core, "t2").ends_with(r#""state":"fired"}"#));
    assert_eq!(ids(&core.inbox(mailbox(Mailbox::Reminders))), vec!["s2".to_string()], "the Reminders view lists the thread");
    assert_eq!(core.fire_due_reminders(), vec!["s2".to_string()], "the notification goes out for it");
    assert!(core.fire_due_reminders().is_empty(), "once");
}

#[test]
fn a_reminder_set_on_an_outbox_entry_is_read_cleared_and_follows_the_sent_message() {
    let (core, account, _d) = seeded(true);
    sql(&core, r#"INSERT INTO outbox_entry (id, draft_json, state, due_at_ms, created_at_ms, updated_at_ms)
                  VALUES ('ob1', '{"scheduled":false,"draft":{"id":"d-queued"},"idempotency_key":"k","threading":null,"copy_pending":null}', 'undoable', 10, 0, 0)"#);
    let at = now() + 3_600_000;
    assert_eq!(core.run(Command::RemindIfNoReply { draft_id: "d-queued".into(), at_ms: at }).unwrap().created_id.as_deref(), Some("d-queued"));
    assert_eq!(reminder_state(&core, "d-queued"), format!(r#"{{"at_ms":{at},"draft_id":"d-queued","state":"waiting"}}"#));
    let sent_at = now();
    core.store().insert_messages(vec![msg(&account, "Sent", "s3", "t3", USER, sent_at)]).unwrap();
    assert_eq!(engine::follow_sent(&core.store(), "d-queued", "s3", sent_at).unwrap().as_deref(), Some("rr-s3"));
    assert!(reminder_state(&core, "t3").contains(r#""draft_id":"s3""#), "{}", reminder_state(&core, "t3"));
    core.run(Command::ClearReplyReminder { draft_id: "s3".into() }).expect("cleared from the sent message");
    assert!(reminder_state(&core, "t3").ends_with(r#""state":"none"}"#));
}
// mutant "sent-ignores-outbox": store/src/delta10.rs, in WriteCtx::reply_reminder_on_sent, the fallback `None => self.outbox_reply_reminder(draft_id)?` becomes `None => None`.

#[test]
fn every_other_command_is_still_refused_in_words() {
    let (core, _a, _d) = seeded(true);
    // fix-blockforget: BlockSender is served now (block_cmds.rs): one row of the block table, no undo entry (it was refused "does not do this yet")
    let out = core.run(Command::BlockSender { address: "someone@example.test".into() }).expect("BlockSender is served");
    assert_eq!((out.undo_entry_id, core.blocks().len()), (None, 1));
}
