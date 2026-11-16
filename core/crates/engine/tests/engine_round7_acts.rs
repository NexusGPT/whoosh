//! engine-act-r6 round 7 (restart): ⌘U cannot send twice and cannot send unasked. The core is real over a real migrated store; the list's server
//! is a listener on 127.0.0.1 (nothing leaves this Mac). The single-edit mutant each test was seen red on is named under it.
//! rig-net-rust round 2: no wait here is without a limit. Every command and read of the core runs on its own thread and is awaited at most
//! `ANSWER_WITHIN` (`bounded`), ending red with a sentence naming what did not answer; the fake server's reads and its hold are bounded too.
//! A command that never answers leaves its thread behind, never the test binary (the threads are not joined).
//! mutant "fake-never-answers" (one per test): the test's `list_server(Answer::After(..))` made `list_server(Answer::Never)` (red in
//! about `ANSWER_WITHIN`, with the sentence; the run ends and leaves no process).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn core_over(dir: &tempfile::TempDir) -> (RustCore, String) {
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let config = AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Ssl, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())),
        Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())), TestTransport::PlainLoopback);
    (core, account)
}

fn mail(account: &str, id: &str, thread: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "news".into(), from_address: "news@list.example".into(), to: vec![USER.into()],
            subject: format!("Issue {id}"), preview: "Body".into(), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(at.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

/// The longest any command or read of these tests may take before the test ends red. A normal run answers in under a second (the slowest
/// press waits for the other's 400 ms POST); the core's own POST limit is 10 s (OAUTH_HTTP_TIMEOUT_MS), so a fake that never answers
/// is caught here first, and a core that never answers at all is caught too.
const ANSWER_WITHIN: Duration = Duration::from_secs(5);
/// How long the fake waits for a request's bytes, and how long a `Never` fake holds a connection before it lets go.
const FAKE_READ_WITHIN: Duration = Duration::from_secs(5);
const FAKE_HOLDS: Duration = Duration::from_secs(30);

/// `f` on its own thread, awaited at most `ANSWER_WITHIN`: red with a sentence naming `what` when it does not answer. The thread is not
/// joined, so a command that never answers cannot hold the test binary.
fn bounded<T: Send + 'static>(what: &str, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { let _ = tx.send(f()); });
    match rx.recv_timeout(ANSWER_WITHIN) {
        Ok(v) => v,
        Err(RecvTimeoutError::Timeout) => panic!("{what} did not answer within {ANSWER_WITHIN:?}: the core is still waiting (for the list's server or a lock)"),
        Err(RecvTimeoutError::Disconnected) => panic!("{what} stopped on its thread without an answer (its panic is printed above)"),
    }
}

/// How the fake list's server answers.
#[derive(Clone, Copy)]
#[allow(dead_code)] // `Never` is the fake of the mutant "fake-never-answers" (header)
enum Answer { After(Duration), Never }

/// A list's server on 127.0.0.1 that answers 200 (or never, holding the connection `FAKE_HOLDS`) and records every request.
fn list_server(answer: Answer) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || for s in l.incoming().flatten() { let s2 = s2.clone(); std::thread::spawn(move || serve(s, s2, answer)); });
    (port, seen)
}
fn serve(mut s: TcpStream, seen: Arc<Mutex<Vec<String>>>, answer: Answer) {
    if s.set_read_timeout(Some(FAKE_READ_WITHIN)).is_err() || s.set_write_timeout(Some(FAKE_READ_WITHIN)).is_err() { return; }
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    if r.read_line(&mut line).is_err() { return; }
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).is_err() || h.trim().is_empty() { break; }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") { len = v.trim().parse().unwrap_or(0); }
    }
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    seen.lock().unwrap().push(line.trim().to_string());
    let delay = match answer { Answer::After(d) => d, Answer::Never => { std::thread::sleep(FAKE_HOLDS); return; } };
    std::thread::sleep(delay);
    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
}
fn outbox(core: &RustCore) -> i64 { core.store().write(|c| Ok(c.tx.query_row("SELECT count(*) FROM outbox_entry", [], |r| r.get::<_, i64>(0))?)).unwrap() }

#[test]
fn two_unsubscribes_at_once_on_one_thread_post_once() {
    let (port, seen) = list_server(Answer::After(Duration::from_millis(400)));
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_over(&dir);
    core.store().insert_messages(vec![mail(&account, "n1", "t", 100), mail(&account, "n2", "t", 200)]).unwrap();
    for (id, tok) in [("n1", "a"), ("n2", "b")] { core.store().set_list_unsubscribe(id.into(), vec![format!("<http://127.0.0.1:{port}/u/{tok}>")], true).unwrap(); }
    // a double press of the confirm, or two windows: both reach the core while the first POST is still on the wire
    // each press on its own thread (not joined); both answers are awaited until one deadline, ANSWER_WITHIN from the first press
    let core = Arc::new(core);
    let (tx, rx) = mpsc::channel();
    for (i, id) in ["n1", "n2"].into_iter().enumerate() {
        let (core, tx) = (core.clone(), tx.clone());
        std::thread::spawn(move || { if i == 1 { std::thread::sleep(Duration::from_millis(100)); } let _ = tx.send((i, core.run(Command::Unsubscribe { message_id: id.into() }))); });
    }
    drop(tx);
    let deadline = Instant::now() + ANSWER_WITHIN;
    let mut results: Vec<Option<Result<CommandOutcome, CoreError>>> = vec![None, None];
    while results.iter().any(|r| r.is_none()) {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok((i, r)) => results[i] = Some(r),
            Err(_) => {
                let late: Vec<String> = results.iter().enumerate().filter(|(_, r)| r.is_none()).map(|(i, _)| format!("press {} (Unsubscribe of {})", i + 1, ["n1", "n2"][i])).collect();
                panic!("{} did not answer within {ANSWER_WITHIN:?}: the core is still waiting (for the list's server or the one-at-a-time lock)", late.join(" and "));
            }
        }
    }
    let results: Vec<Result<CommandOutcome, CoreError>> = results.into_iter().map(|r| r.unwrap()).collect();
    let posts = seen.lock().unwrap().clone();
    assert_eq!(posts.len(), 1, "one POST for the thread: {posts:?}");
    assert!(results[0].is_ok(), "the first leaves the list: {:?}", results[0]);
    match &results[1] { Err(CoreError::Invalid { reason_words, .. }) => assert_eq!(reason_words, "You already left this list."), other => panic!("the second says it is done: {other:?}") }
}
// mutant "no-guard-across-the-post": unsubscribe.rs `run`, the `let _once = UNSUBSCRIBE_ONE_AT_A_TIME.lock()...` line deleted (red: two POSTs)

#[test]
fn nothing_is_sent_or_marked_until_the_confirmed_unsubscribe_and_the_reads_return_what_the_screen_confirms() {
    let (port, seen) = list_server(Answer::After(Duration::from_millis(0)));
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_over(&dir);
    let core = Arc::new(core);
    core.store().insert_messages(vec![mail(&account, "oc", "t1", 100), mail(&account, "ml", "t2", 200), mail(&account, "lk", "t3", 300)]).unwrap();
    core.store().set_list_unsubscribe("oc".into(), vec![format!("<http://127.0.0.1:{port}/u/x>")], true).unwrap();
    core.store().set_list_unsubscribe("ml".into(), vec!["<mailto:leave@list.example?subject=bye>".into()], false).unwrap();
    core.store().set_list_unsubscribe("lk".into(), vec!["<https://list.example/leave>".into()], false).unwrap();
    // what the screen asks the owner about
    let c = core.clone();
    let methods = bounded("the unsubscribe_method reads of oc, ml and lk", move || ["oc", "ml", "lk"].map(|id| c.unsubscribe_method(id.into())));
    assert!(matches!(methods[0], Some(UnsubscribeMethod::OneClick { .. })));
    assert_eq!(methods[1], Some(UnsubscribeMethod::Mail { address: "leave@list.example".into(), subject: Some("bye".into()) }));
    assert_eq!(methods[2], Some(UnsubscribeMethod::Link { url: "https://list.example/leave".into() }));
    // every read, many times, as a list or a reader redraw does
    let c = core.clone();
    bounded("the 3 rounds of state and message reads", move || for _ in 0..3 { for id in ["oc", "ml", "lk"] { for p in ["unsubscribe", "unsubscribed", "unsubscribe_method", "unsubscribed_at", "muted", "archived"] { let _ = c.state(format!("message/{id}/{p}")); } let _ = c.message(id.into()); } });
    let posts = seen.lock().unwrap().clone();
    assert!(posts.is_empty(), "reads sent a request: {posts:?}");
    let c = core.clone();
    assert_eq!(bounded("the outbox count after the reads", move || outbox(&c)), 0, "reads queued a mail");
    let c = core.clone();
    let marked = bounded("the unsubscribed reads", move || ["oc", "ml", "lk"].map(|id| c.state(format!("message/{id}/unsubscribed"))));
    for (id, m) in ["oc", "ml", "lk"].iter().zip(marked) { assert_eq!(m.as_deref(), Some("false"), "{id} marked without a confirm"); }
    // the confirm: the one-click posts once, the mail link queues one mail, the web link only marks
    for id in ["ml", "lk"] {
        let c = core.clone();
        bounded(&format!("Unsubscribe of {id}"), move || c.run(Command::Unsubscribe { message_id: id.into() })).unwrap();
    }
    let c = core.clone();
    assert_eq!((seen.lock().unwrap().len(), bounded("the outbox count after ml and lk", move || outbox(&c))), (0, 1));
    let c = core.clone();
    bounded("Unsubscribe of oc (the one-click POST)", move || c.run(Command::Unsubscribe { message_id: "oc".into() })).unwrap();
    let c = core.clone();
    assert_eq!((seen.lock().unwrap().len(), bounded("the outbox count after oc", move || outbox(&c))), (1, 1));
}
// mutant "a-read-posts": unsubscribe.rs `method`, a `post_one_click` call on the one-click URL before `unsubscribe_method_from` (red: a request seen from a read)
