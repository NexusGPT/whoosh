//! engine-act-r6 round 7.
//! (1) The filing record of mail the label flow left in Primary says what the label AI can do NOW (`message/<id>/filing` `ai`): "no_provider"
//!     (no model chosen), "refused" with the model's own words in `ai_refusal` (chosen but cannot answer: Laya handed over without its weights),
//!     "configured" (it can answer). It said "configured" whenever a port existed, so a rig with Laya and no weights claimed a working model.
//! (2) ⌘U: a list left is left for the whole thread, with its date; reads never send; a second ⌘U on the thread posts nothing.
//! The core is real over a real migrated store and a memory keyring; the list's server is a loopback listener. The single-edit mutant each
//! test was seen red on is named under it.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn core_over(dir: &tempfile::TempDir, transport: TestTransport) -> (RustCore, String) {
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let config = AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Ssl, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())),
        Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())), transport);
    (core, account)
}

fn mail(account: &str, id: &str, thread: &str, from: &str, subject: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(), to: vec![USER.into()],
            subject: subject.into(), preview: format!("Body of {subject}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(at.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

// ---------------------------------------------------------------- (1) the filing's word for the label AI

/// The shell's on-device model as the core is handed it: answers "Reply soon", or refuses in words while `fail` holds them.
struct Laya { fail: Mutex<Option<String>> }
impl DecisionPort for Laya {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        if let Some(words) = self.fail.lock().unwrap().clone() {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Laya".into(), reason_words: words, retryable: false, code: None } });
        }
        // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
        let answers = questions.iter().map(|q| ::decision::test_answers::labels_answer(q, Some("Reply soon"), 0.95).unwrap_or_else(|| match q {
            DecisionQuestion::Choice { id, .. } => DecisionAnswer::Choice { id: id.clone(), choice: "Reply soon".into(), probabilities: [("Reply soon".to_string(), 0.95)].into() },
            DecisionQuestion::Score { id, .. } => DecisionAnswer::Score { id: id.clone(), value: 0.0 },
            DecisionQuestion::Noul { id, .. } => DecisionAnswer::Noul { id: id.clone(), yes: false, probability: 0.1 },
        })).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some("laya-scripted".into()) })
    }
}

fn filing(core: &RustCore, id: &str) -> serde_json::Value {
    serde_json::from_str(&core.state(format!("message/{id}/filing")).expect("the filing is served")).expect("JSON")
}

#[test]
fn the_filing_says_no_provider_refused_with_its_words_or_configured_as_the_ai_stands_now() {
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_over(&dir, TestTransport::Real);
    let spec = LabelSpec { name: "Reply soon".into(), parent_id: None, colour: None, description: "mail that waits for my answer".into(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false };
    core.store().label_create_full(spec, 1).unwrap();
    let arrive = |n: i64| -> String {
        let id = format!("{account}:INBOX:{n}");
        core.store().insert_messages(vec![mail(&account, &id, &format!("t{n}"), "ann@client.example", "Can you answer?", 1_000 + n)]).unwrap();
        core.file_stored_messages(&[id.clone()]).unwrap();
        id
    };
    // no model at all: the flow leaves the mail in Primary and says so
    let first = arrive(1);
    let f = filing(&core, &first);
    assert_eq!((f["ai"].as_str(), f["card"].as_str(), f["place"].as_str(), f["ai_refusal"].is_null()), (Some("no_provider"), Some("hand"), Some("primary"), true), "{f}");
    // the shell hands Laya over without its weights: chosen, asked, refused; the filing says so in the model's words, never "configured"
    let words = "The on-device decision model is not on this Mac yet: download it in Settings, Decisions.";
    let laya = Arc::new(Laya { fail: Mutex::new(Some(words.into())) });
    core.set_laya_port(Some(laya.clone()));
    let second = arrive(2);
    for id in [&first, &second] {
        let f = filing(&core, id);
        assert_eq!(f["ai"].as_str(), Some("refused"), "{f}");
        assert!(f["ai_refusal"].as_str().is_some_and(|w| w.contains(words)), "the refusal's words ride with it: {f}");
        assert_eq!(f["place"].as_str(), Some("primary"));
    }
    // the weights arrive: a judgement works, the mail it reads is answered, and the one still in Primary says the AI is configured
    *laya.fail.lock().unwrap() = None;
    let third = arrive(3);
    assert_eq!(filing(&core, &third)["ai"].as_str(), Some("answered"));
    let f = filing(&core, &first);
    assert_eq!((f["ai"].as_str(), f["ai_refusal"].is_null()), (Some("configured"), true), "{f}");
}
// mutant "configured-means-a-port": rust_core.rs `filing_ai`, `if self.deps.filing.ai_can_answer()` -> `if self.deps.filing.port().is_some()` (red: "configured" while Laya refuses)
// mutant "refusal-words-dropped": label_flow.rs `filing_json`, `(WhyCard::Hand, FilingAi::Refused(w)) => s(w)` -> `(WhyCard::Hand, FilingAi::Refused(_)) => "null".to_string()` (red)

// ---------------------------------------------------------------- (2) ⌘U: the list left for the whole thread

#[derive(Debug, Clone, PartialEq)]
struct Post { path: String, body: String }

/// A list's server on 127.0.0.1 that answers 200 and records every request.
fn list_server() -> (u16, Arc<Mutex<Vec<Post>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || for s in l.incoming().flatten() { let s2 = s2.clone(); std::thread::spawn(move || serve(s, s2)); });
    (port, seen)
}
fn serve(mut s: TcpStream, seen: Arc<Mutex<Vec<Post>>>) {
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
    seen.lock().unwrap().push(Post { path: line.split_whitespace().nth(1).unwrap_or("").into(), body: String::from_utf8_lossy(&body).into_owned() });
    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
}

#[test]
fn a_list_left_is_left_for_its_thread_with_its_date_reads_send_nothing_and_a_second_unsubscribe_posts_nothing() {
    let (port, seen) = list_server();
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = core_over(&dir, TestTransport::PlainLoopback);
    // two issues of one newsletter in one thread, each with its own one-click URL (lists put a token per issue in it)
    core.store().insert_messages(vec![mail(&account, "n1", "t-news", "news@list.example", "Weekly 1", 100), mail(&account, "n2", "t-news", "news@list.example", "Weekly 2", 200)]).unwrap();
    for (id, token) in [("n1", "a1"), ("n2", "b2")] { core.store().set_list_unsubscribe(id.into(), vec![format!("<http://127.0.0.1:{port}/u/{token}>")], true).unwrap(); }
    // what the screen reads to ask the owner first: the method and the state; nothing is sent by reading
    assert!(matches!(core.unsubscribe_method("n1".into()), Some(UnsubscribeMethod::OneClick { .. })));
    for p in ["unsubscribe", "unsubscribed", "unsubscribe_method", "unsubscribed_at"] { let _ = core.state(format!("message/n2/{p}")); }
    assert_eq!(core.state("message/n2/unsubscribed".into()).as_deref(), Some("false"));
    assert_eq!(core.state("message/n2/unsubscribed_at".into()), None, "no date while the list was not left");
    assert!(seen.lock().unwrap().is_empty(), "a read sends nothing");
    // the owner confirmed: one POST (RFC 8058)
    core.run(Command::Unsubscribe { message_id: "n1".into() }).expect("left");
    assert_eq!(seen.lock().unwrap().clone(), vec![Post { path: "/u/a1".into(), body: "List-Unsubscribe=One-Click".into() }]);
    let at = core.state("message/n1/unsubscribed_at".into()).expect("the date is kept");
    assert!(at.parse::<i64>().is_ok_and(|ms| ms > 0), "{at}");
    // the other issue of the thread: left, the same date, and a second ⌘U says so and posts nothing
    assert_eq!(core.state("message/n2/unsubscribed".into()).as_deref(), Some("true"));
    assert_eq!(core.state("message/n2/unsubscribe".into()).as_deref(), Some("\"done\""));
    assert_eq!(core.state("message/n2/unsubscribed_at".into()), Some(at));
    match core.run(Command::Unsubscribe { message_id: "n2".into() }) {
        Err(CoreError::Invalid { reason_words, .. }) => assert_eq!(reason_words, "You already left this list."),
        other => panic!("a second unsubscribe on the thread must say it is done: {other:?}"),
    }
    assert_eq!(seen.lock().unwrap().len(), 1, "no second POST");
}
// mutant "left-per-message-only": unsubscribe.rs `run`, `store.list_left_at(message_id)` -> `store.unsubscribed_at(message_id)` (red: the second issue posts again)
// mutant "date-not-served": unsubscribe.rs `state`, the `"unsubscribed_at" => ..` arm deleted (red: no date)
