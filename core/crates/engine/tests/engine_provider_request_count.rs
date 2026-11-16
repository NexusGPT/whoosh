//! Lane fix-aicore, part B (hid-ai finding F2): the real core serves `provider_request/count`, the requests asked of a model provider OFF this
//! Mac since launch, from ONE counter in the providers' shared client (`ai_settings.rs` `ProviderHttps::send`, `OFF_DEVICE_REQUESTS`). Each kind of
//! off-device request moves it by one (a key check, a question to the hosted decision service Jev, a hosted Writer's call); an ask of a model on
//! this Mac does not. Every case runs as a rig process (`rig::apply`): nothing can reach a host past this Mac; the fakes sit on 127.0.0.1, and a key
//! provider's host is routed to them by `route_key_providers_to_loopback` (the hidden copy's own routing), so the request still names the hosted
//! provider's address. The counter is one per process: the cases take turns (`TURN`) and read the count before and after their own step.
//! Each case prints one `PASS <case>` line (`-- --nocapture`).
//!
//! Broken copies (single edits, each red in the named test):
//! - C1 core/crates/engine/src/ai_settings.rs `ProviderHttps::send`: the line
//!   `if !netguard::is_this_mac(&netguard::host_of_url(&request.url)) { OFF_DEVICE_REQUESTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst); }   // fix-aicore F2`
//!   removed -> a_key_check_sent_off_this_mac_moves_the_count_by_one, a_hosted_writer_call_moves_the_count_by_one and
//!   a_question_to_jev_moves_the_count_by_one are red (the count stays)
//! - C2 the same line with `!netguard::is_this_mac(&netguard::host_of_url(&request.url))` replaced by `true` -> an_on_device_ask_does_not_move_the_count is red
//! - C3 core/crates/engine/src/rust_core.rs `state`: the `provider_request/count` arm removed -> the_state_path_answers_the_count is red
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings::{self, AiJob};
use engine::{AiStep, RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const OWNER: &str = "owner@example.test";
const GOOD: &str = "FIXTURE-sk-ant-good-2c41e0aa";

/// The cases take turns: the counter is the process's.
static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> { TURN.lock().unwrap_or_else(|e| e.into_inner()) }

/// A fake provider on 127.0.0.1 for every path a provider is asked on: a model listing, an Anthropic message, an OpenAI-style chat. Records each path.
struct Fake { port: u16, paths: Arc<Mutex<Vec<String>>> }

fn fake_provider() -> Fake {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let paths = Arc::new(Mutex::new(Vec::new()));
    let seen = paths.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let seen = seen.clone();
            std::thread::spawn(move || answer(s, seen));
        }
    });
    Fake { port, paths }
}

fn answer(mut s: TcpStream, paths: Arc<Mutex<Vec<String>>>) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).is_err() { return; }
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).is_err() || line.trim().is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') { if k.trim().eq_ignore_ascii_case("content-length") { len = v.trim().parse().unwrap_or(0); } }
    }
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    paths.lock().unwrap().push(path.clone());
    let text = if path.ends_with("/models") {
        r#"{"data":[{"id":"claude-sonnet-5-5"},{"id":"llama3"}],"has_more":false}"#.to_string()
    } else if path.ends_with("/messages") {
        r#"{"content":[{"type":"text","text":"ok"}]}"#.to_string()
    } else {
        r#"{"choices":[{"message":{"content":"ok"}}]}"#.to_string()
    };
    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

fn rig_process(store_path: &std::path::Path) { engine::rig::apply(&engine::rig::RigSwitches { rig: true, ..Default::default() }, store_path); }

struct Rig { core: RustCore, account: String, label: String, _dir: tempfile::TempDir }

impl Rig {
    /// A rig core with one account and a described label "Reply soon".
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("whoosh.sqlite");
        rig_process(&path);
        let store = Store::open(&path).unwrap();
        let config = AccountConfig {
            imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
            smtp_security: ConnectionSecurity::Starttls, username: OWNER.into(), smtp_username: OWNER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
        };
        let account = store.add_account(OWNER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
        let spec = LabelSpec { name: "Reply soon".into(), parent_id: None, colour: None, description: "mail that waits for my answer".into(), scope: LabelScope::All, rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false };
        let label = store.label_create_full(spec, 1).unwrap().id;
        let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
        let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector, TestTransport::Real);
        Rig { core, account, label, _dir: dir }
    }
    fn state(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }
    /// The key of `provider` pasted, and its check run to its end ("connected" or "refused").
    fn save_key(&self, provider: &str, key: &str) -> String {
        self.core.run(Command::SaveProviderKey { provider_id: provider.into(), key: SecretText { value: key.into() } }).expect("a key is accepted for checking");
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let s = self.state(&format!("ai_provider/{provider}/state")).unwrap_or_default();
            if s != "\"checking\"" || Instant::now() > until { return s; }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    /// One inbound message arrives and the label flow runs on it.
    fn arrive(&self, n: u32) -> engine::Filed {
        let id = format!("{}:INBOX:{n}", self.account);
        let message = Message {
            id: id.clone(), thread_id: format!("t{n}"), account_id: self.account.clone(), sender_name: "Ann".into(), from_address: "ann@client.example".into(),
            to: vec![OWNER.into()], subject: "Can you answer?".into(), preview: "Body of Can you answer?".into(), received_at_ms: 1_000 + n as i64, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None,
            cc: vec![], bcc: vec![], is_outgoing: false,
        };
        self.core.store().insert_messages(vec![NewMessage { message, remote_id: Some(n.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None }]).unwrap();
        self.core.file_stored_messages(&[id]).unwrap().pop().expect("the message was filed or left")
    }
}

fn count() -> u64 { ai_settings::provider_request_count() }

#[test]
fn a_key_check_sent_off_this_mac_moves_the_count_by_one() {
    let _t = turn();
    let rig = Rig::new();
    let fake = fake_provider();
    ai_settings::route_key_providers_to_loopback(rig.core.store().path(), fake.port);
    let before = count();
    assert_eq!(rig.save_key("anthropic", GOOD), "\"connected\"", "the check reached the fake through the rig's routing");
    assert_eq!(count() - before, 1, "one key check asked of api.anthropic.com is one request off this Mac");
    assert_eq!(fake.paths.lock().unwrap().len(), 1, "and it was the one request made: {:?}", fake.paths.lock().unwrap());
    println!("PASS a_key_check_sent_off_this_mac_moves_the_count_by_one");
}

#[test]
fn a_hosted_writer_call_moves_the_count_by_one() {
    let _t = turn();
    let rig = Rig::new();
    let fake = fake_provider();
    ai_settings::route_key_providers_to_loopback(rig.core.store().path(), fake.port);
    assert_eq!(rig.save_key("anthropic", GOOD), "\"connected\"");
    // the built-in Writer is Anthropic's model: its key is connected, so a job's model is the hosted one
    let model = rig.core.model_for(AiJob::Draft).expect("the Writer's hosted model is usable");
    assert!(!model.on_device);
    let before = count();
    assert_eq!(model.complete(None, "Reply to this".into(), 50).expect("the fake answers"), "ok");
    assert_eq!(count() - before, 1, "one call of a hosted model is one request off this Mac");
    println!("PASS a_hosted_writer_call_moves_the_count_by_one");
}

#[test]
fn a_question_to_jev_moves_the_count_by_one() {
    let _t = turn();
    let rig = Rig::new();
    // the owner's path: Jev's key, the consent, the model (Settings, Decisions); Jev's own address, refused by the rig before any connection
    rig.core.run(Command::SaveProviderKey { provider_id: "jev".into(), key: SecretText { value: "FIXTURE-jev-key-77aa".into() } }).expect("Jev's key is kept");
    let keychain = rig.core.ai_providers().into_iter().find(|p| p.id == "jev").and_then(|p| p.keychain).expect("the key's reference is on Jev's row");
    rig.core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: true }).expect("consent is served");
    let model = DecisionModel { provider: DecisionProviderKind::Jev, model: None, keychain: Some(keychain), decisions_enabled: vec!["labels".into()], question_words: Default::default(), last_run_ms: None };
    rig.core.run(Command::SaveDecisionModel { model }).expect("SaveDecisionModel is served");
    let before = count();
    match rig.arrive(1).ai {
        AiStep::Refused(why) => assert!(why.contains("is not this Mac"), "the rig refused Jev's host: {why}"),
        other => panic!("Jev was asked and the rig refused it, not {other:?}"),
    }
    assert_eq!(count() - before, 1, "one question to Jev is one request off this Mac (refused by the rig, still asked)");
    let _ = &rig.label;
    println!("PASS a_question_to_jev_moves_the_count_by_one");
}

#[test]
fn an_on_device_ask_does_not_move_the_count() {
    let _t = turn();
    let rig = Rig::new();
    let fake = fake_provider();
    // the owner's Writer on Ollama, a model on this Mac (its address is this Mac's own)
    ai_settings::set_provider_address(rig.core.store().path(), "ollama", Some(format!("http://127.0.0.1:{}", fake.port)));
    // written as a row, as engine_reply_work.rs does: SaveAgentConfig takes a key provider or "local" only (ai_settings.rs `known_provider`)
    rig.core.store().write(|w| Ok(w.tx.execute("INSERT OR REPLACE INTO agent_config (id, name, provider_id, model, updated_at_ms) VALUES ('writer', 'Writer', 'ollama', 'llama3', 1)", [])?)).unwrap();
    let model = rig.core.model_for(AiJob::Draft).expect("Ollama needs no key");
    assert!(model.on_device);
    let before = count();
    assert_eq!(model.complete(None, "Reply to this".into(), 50).expect("the fake answers"), "ok");
    assert_eq!(fake.paths.lock().unwrap().clone(), vec!["/chat/completions".to_string()], "the model on this Mac was asked once");
    assert_eq!(count(), before, "an ask of a model on this Mac is not a request off this Mac");
    println!("PASS an_on_device_ask_does_not_move_the_count");
}

#[test]
fn the_state_path_answers_the_count() {
    let _t = turn();
    let rig = Rig::new();
    let fake = fake_provider();
    ai_settings::route_key_providers_to_loopback(rig.core.store().path(), fake.port);
    let read = || rig.state("provider_request/count").expect("provider_request/count is served").parse::<u64>().expect("a JSON number");
    let before = read();
    assert_eq!(before, count(), "the path answers the counter");
    rig.save_key("anthropic", GOOD);
    assert_eq!(read(), before + 1, "the path moves with the key check");
    println!("PASS the_state_path_answers_the_count");
}
