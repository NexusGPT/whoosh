//! gap-engine-state, item ee20d7cc: "a real-core check saves a key against a local fake provider AND GETS A DRAFT". The real `RustCore` (a real migrated store, a memory
//! secret store in the rig namespace, the draft engine the core installs at start, NO test model) saves a key through `Command::SaveProviderKey` against a fake provider
//! on 127.0.0.1, then `Command::DraftReplyOnDemand` writes a draft through that key: the draft's text is what the fake provider answered, the provider received the
//! key and the message's words, and no `ai_no_model` refusal is raised. Before the key the same command refuses in words (`ai_no_model`). Nothing leaves this Mac.
//! Each case prints one `PASS <case>` line (`-- --nocapture`).
//!
//! Broken copies (single edits, each red in the named test):
//! - D1 core/crates/engine/src/ai_settings.rs `model_for`: `provider_state_is_usable(provider_state(store, provider.slug))` becomes `true` (the item's own mutant): a key still
//!   being checked is used, `a_key_that_is_still_being_checked_gets_no_draft` is red
//! - D2 core/crates/engine/src/ai_settings.rs `model_for`: `let key = if provider.needs_key {` becomes `let key = if false {` (the request carries no key, the provider refuses it, no draft is written):
//!   `a_saved_key_gets_a_draft_written_by_the_provider` is red
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const OWNER: &str = "owner@example.test";
const GOOD: &str = "FIXTURE-sk-ant-good-5a1d93e7";
const SLOW: &str = "FIXTURE-sk-ant-slow-61c0b2f4";
const ANSWER: &str = "A drafted reply.";
const SLOW_MS: u64 = 600;

/// The fake provider: models for a good or slow key, completions (recorded with the key and the request body) for them, a refusal for any other.
struct Fake { port: u16, completions: Arc<Mutex<Vec<(String, String)>>> }

fn fake_provider() -> Fake {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let completions = Arc::new(Mutex::new(Vec::new()));
    let seen = completions.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let seen = seen.clone();
            std::thread::spawn(move || answer(s, seen));
        }
    });
    Fake { port, completions }
}

fn answer(mut s: TcpStream, completions: Arc<Mutex<Vec<(String, String)>>>) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).is_err() { return; }
    let mut headers: Vec<(String, String)> = Vec::new();
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).is_err() || line.trim().is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') { headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string())); }
    }
    let get = |n: &str| headers.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    let len: usize = get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let key = get("x-api-key").or_else(|| get("authorization").and_then(|v| v.strip_prefix("Bearer ").map(String::from))).unwrap_or_default();
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    let (status, text) = match (key.as_str(), path.as_str()) {
        (GOOD | SLOW, "/models") => {
            if key == SLOW { std::thread::sleep(Duration::from_millis(SLOW_MS)); }
            (200, r#"{"data":[{"id":"claude-sonnet-5-5"},{"id":"claude-haiku-5-5"}],"has_more":false}"#.to_string())
        }
        (GOOD | SLOW, "/messages") => {
            completions.lock().unwrap().push((key.clone(), String::from_utf8_lossy(&body).to_string()));
            (200, format!(r#"{{"content":[{{"type":"text","text":"{ANSWER}"}}]}}"#))
        }
        _ => (401, r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#.to_string()),
    };
    let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }

#[derive(Default)]
struct Events(Mutex<Vec<CoreEvent>>);
impl EventSink for Events { fn on_event(&self, e: CoreEvent) { self.0.lock().unwrap().push(e); } }
impl Events {
    fn ready(&self) -> Vec<String> { self.0.lock().unwrap().iter().filter_map(|e| if let CoreEvent::ReplyDraftReady { draft_id, .. } = e { Some(draft_id.clone()) } else { None }).collect() }
}

struct Rig { core: RustCore, fake: Fake, events: Arc<Events>, _dir: tempfile::TempDir }

impl Rig {
    /// A core with one account and one message from claire in thread t1; the provider's address is the fake's; no key, no test model.
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let config = AccountConfig {
            imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
            smtp_security: ConnectionSecurity::Starttls, username: OWNER.into(), smtp_username: OWNER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
        };
        let account = store.add_account(OWNER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
        let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
        let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector, TestTransport::PlainLoopback);
        let fake = fake_provider();
        ai_settings::set_provider_address(core.store().path(), "anthropic", Some(format!("http://127.0.0.1:{}", fake.port)));
        let body = "Hello, what is the contract price? Claire";
        core.store().insert_messages(vec![NewMessage {
            message: Message {
                id: "m1".into(), thread_id: "t1".into(), account_id: account, sender_name: String::new(), from_address: "claire@client.test".into(), to: vec![OWNER.into()],
                subject: "Contract".into(), preview: body.into(), received_at_ms: now() - 60_000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
                login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
            },
            remote_id: Some("m1".into()), body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
        }]).unwrap();
        let events = Arc::new(Events::default());
        core.subscribe(StreamKind::Thread, events.clone());
        Rig { core, fake, events, _dir: dir }
    }
    fn state(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }
    fn save(&self, key: &str) { self.core.run(Command::SaveProviderKey { provider_id: "anthropic".into(), key: SecretText { value: key.into() } }).expect("a key is accepted for checking"); }
    fn settle(&self) -> String {
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let s = self.state("ai_provider/anthropic/state").unwrap_or_default();
            if s != "\"checking\"" || Instant::now() > until { return s; }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn draft(&self) -> Result<CommandOutcome, CoreError> { self.core.run(Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }) }
}

fn refusal_code(e: &CoreError) -> Option<String> { match e { CoreError::Action { failure } => failure.code.clone(), _ => None } }

#[test]
fn a_saved_key_gets_a_draft_written_by_the_provider() {
    let rig = Rig::new();
    // before a key: the same command refuses in words
    let e = rig.draft().expect_err("no model before a key is saved");
    assert_eq!(refusal_code(&e).as_deref(), Some(AI_NO_MODEL_CODE), "{e}");
    assert!(e.to_string().contains("Anthropic is not connected"), "{e}");
    assert!(rig.fake.completions.lock().unwrap().is_empty(), "no completion was asked before a key was saved");
    // the key is saved, the provider accepts it
    rig.save(GOOD);
    assert_eq!(rig.settle(), "\"connected\"", "the provider accepted the key");
    // the draft is asked, written in the background, announced
    let out = rig.draft().expect("with a saved key no `ai_no_model` is raised");
    let draft_id = out.created_id.expect("the command answers with the draft's id");
    let until = Instant::now() + Duration::from_secs(20);
    while !rig.events.ready().contains(&draft_id) && Instant::now() < until { std::thread::sleep(Duration::from_millis(25)); }
    assert!(rig.events.ready().contains(&draft_id), "ReplyDraftReady never came for {draft_id}");
    let live: serde_json::Value = serde_json::from_str(&rig.state("reply/draft/t1").expect("the thread has a live draft")).unwrap();
    assert_eq!(live["id"], draft_id.as_str(), "the live draft is the one asked for");
    assert_eq!(live["text"], ANSWER, "the draft text is what the fake provider returned");
    // the provider saw the saved key and the message's own words
    let calls = rig.fake.completions.lock().unwrap().clone();
    assert!(!calls.is_empty(), "the provider received no completion request");
    assert!(calls.iter().all(|(k, _)| k == GOOD), "every completion carried the saved key: {:?}", calls.iter().map(|(k, _)| k.len()).collect::<Vec<_>>());
    assert!(calls.iter().any(|(_, body)| body.contains("what is the contract price")), "the draft's request holds the message's words");
    println!("PASS key-saved-then-draft-written-by-the-provider ({} completion request(s))", calls.len());
}

#[test]
fn a_key_that_is_still_being_checked_gets_no_draft() {
    let rig = Rig::new();
    rig.save(GOOD);
    assert_eq!(rig.settle(), "\"connected\"");
    rig.save(SLOW);   // a new paste: the row of the first key is still there, the state reads checking
    assert_eq!(rig.state("ai_provider/anthropic/state").as_deref(), Some("\"checking\""), "the slow provider is still checking");
    let e = rig.draft().expect_err("a key being checked is not usable");
    assert_eq!(refusal_code(&e).as_deref(), Some(AI_NO_MODEL_CODE), "{e}");
    assert!(rig.fake.completions.lock().unwrap().is_empty(), "no completion was asked while the new key was being checked");
    assert_eq!(rig.settle(), "\"connected\"");
    assert!(rig.draft().is_ok(), "usable once the check landed");
    println!("PASS draft-refused-while-the-key-is-checked-then-allowed");
}
