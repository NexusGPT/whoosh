//! Lane fix-aicore, part A: the reader's thread summary in the real engine (engine/src/thread_summary.rs, store/src/thread_summary.rs, migration
//! 0030). The real `RustCore` over a real migrated store; the writing model is a TWIN behind the providers' HTTP seam
//! (`ai_settings::set_scripted_transport`: every job's model talks to it instead of HTTPS, through the real `model_for`, the real adapter and
//! the reply engine's `ProviderModel`). Every case runs as a rig process: nothing can reach a host past this Mac. Each case prints one
//! `PASS <case>` line (`-- --nocapture`); the time case prints its times.
//!
//! Broken copies (single edits, each red in the named test):
//! - S1 core/crates/engine/src/thread_summary.rs `make`: `if let Err(e) = store.thread_summary_put(thread_id, text.trim(), &newest) {` becomes
//!   `if let Err(e) = Ok::<bool, store::StoreError>(true) {` (nothing is kept) -> a_thread_gets_its_summary_from_the_writing_model_and_keeps_it_across_a_restart
//! - S2 core/crates/store/src/thread_summary.rs `is_stale`: `(self.text.is_none() || self.made_from != self.newest)` becomes `self.text.is_none()`
//!   -> a_new_message_in_the_thread_refreshes_the_summary
//! - S3 core/crates/engine/src/thread_summary.rs `state`: `.unwrap_or_else(|| "null".into()))` becomes `.unwrap_or_else(|| "\"\"".into()))`
//!   -> no_writing_model_means_no_call_and_the_path_answers_null
//! - S4 core/crates/engine/src/thread_summary.rs `make`: `if !on_device {` becomes `if false {` -> a_hosted_provider_without_consent_is_never_asked
//! - S5 core/crates/engine/src/thread_summary.rs `state`: `if row.is_stale() { queue(store, keychain, gate(), thread_id); }` becomes
//!   `if row.is_stale() { make(store, keychain, gate().as_ref(), thread_id); }` (the read waits on the model)
//!   -> opening_a_thread_reads_the_stored_summary_under_50_ms_on_2000_mails
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ai_providers::transport::{Body, Request, Response, Transport};
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const OWNER: &str = "owner@example.test";
const BODY_WORDS: &str = "BODYWORDS a quote for 12,000 brochures at 0.84 EUR, answer by Tuesday";
const GOOD: &str = "FIXTURE-sk-ant-good-90d1c3b2";
const SETTLE: Duration = Duration::from_secs(10);

/// The writing model's twin: answers each call with the next summary (the last one again when the list is used up), in both answer shapes
/// (OpenAI-style `choices`, Anthropic `content`), after `delay`. Keeps the body of every request it was sent.
struct Twin { answers: Mutex<Vec<String>>, seen: Mutex<Vec<String>>, delay: Duration }

impl Twin {
    fn new(answers: &[&str], delay: Duration) -> Arc<Twin> {
        Arc::new(Twin { answers: Mutex::new(answers.iter().map(|a| a.to_string()).collect()), seen: Mutex::new(vec![]), delay })
    }
    fn calls(&self) -> usize { self.seen.lock().unwrap().len() }
}

impl Transport for Twin {
    fn send(&self, r: &Request) -> Result<Response, String> {
        let body = match &r.body { Body::Json(t) => t.clone(), _ => String::new() };
        self.seen.lock().unwrap().push(body);
        std::thread::sleep(self.delay);
        let text = { let mut a = self.answers.lock().unwrap(); if a.len() > 1 { a.remove(0) } else { a.first().cloned().unwrap_or_default() } };
        let quoted = serde_json::Value::String(text).to_string();
        Ok(Response { status: 200, body: format!(r#"{{"choices":[{{"message":{{"content":{quoted}}}}}],"content":[{{"type":"text","text":{quoted}}}]}}"#) })
    }
}

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn mail(account: &str, id: &str, thread: &str, at: i64, body: &str) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "Ann".into(), from_address: "ann@client.example".into(), to: vec![OWNER.into()],
            subject: "Brochures".into(), preview: body.chars().take(80).collect(), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(id.into()), body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

struct Rig { core: RustCore, keys: Arc<MemoryKeychain>, account: String, path: PathBuf, dir: tempfile::TempDir }

impl Rig {
    /// A rig core with one account and one message m1 in thread t1.
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("whoosh.sqlite");
        engine::rig::apply(&engine::rig::RigSwitches { rig: true, ..Default::default() }, &path);
        let store = Store::open(&path).unwrap();
        let config = AccountConfig {
            imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
            smtp_security: ConnectionSecurity::Starttls, username: OWNER.into(), smtp_username: OWNER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
        };
        let account = store.add_account(OWNER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config, 1).unwrap();
        store.insert_messages(vec![mail(&account, "m1", "t1", 1_000, BODY_WORDS)]).unwrap();
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let core = RustCore::with_parts(store, keys.clone(), detector(), TestTransport::PlainLoopback);
        Rig { core, keys, account, path, dir }
    }
    /// The same store reopened by a new core (a restart).
    fn restart(self) -> Rig {
        let Rig { core, keys, account, path, dir } = self;
        drop(core);
        let core = RustCore::with_parts(Store::open(&path).unwrap(), keys.clone(), detector(), TestTransport::PlainLoopback);
        Rig { core, keys, account, path, dir }
    }
    fn summary(&self, thread: &str) -> Option<String> { self.core.state(format!("thread/{thread}/summary")) }
    fn settle(&self) { assert!(engine::thread_summary::settle(&self.path, SETTLE), "the summary worker finished within {SETTLE:?}"); }
    /// The Writer on Ollama, a model on this Mac (no key, no consent). Written as a row, as engine_reply_work.rs does: SaveAgentConfig takes a
    /// key provider or "local" only (ai_settings.rs `known_provider`), and `model_for` has no text model for "local" (finding F-A3 of the notes).
    fn writer_on_this_mac(&self) {
        self.core.store().write(|w| Ok(w.tx.execute("INSERT OR REPLACE INTO agent_config (id, name, provider_id, model, updated_at_ms) VALUES ('writer', 'Writer', 'ollama', 'llama3', 1)", [])?)).unwrap();
        assert!(self.core.model_for(ai_settings::AiJob::Draft).expect("the Writer on Ollama").on_device);
    }
    fn twin(&self, twin: &Arc<Twin>) { ai_settings::set_scripted_transport(&self.path, Some(twin.clone())); }
}

fn quoted(s: &str) -> Option<String> { Some(serde_json::Value::String(s.into()).to_string()) }

#[test]
fn a_thread_gets_its_summary_from_the_writing_model_and_keeps_it_across_a_restart() {
    let rig = Rig::new();
    let twin = Twin::new(&["A quote for 12,000 brochures; answer by Tuesday."], Duration::ZERO);
    rig.twin(&twin);
    rig.writer_on_this_mac();
    // the first open: nothing kept yet, so null at once, and the summary is made in the background
    assert_eq!(rig.summary("t1").as_deref(), Some("null"));
    rig.settle();
    assert_eq!(rig.summary("t1"), quoted("A quote for 12,000 brochures; answer by Tuesday."), "the writing model's words are kept");
    assert_eq!(twin.calls(), 1, "one call");
    assert!(twin.seen.lock().unwrap()[0].contains("BODYWORDS a quote for 12,000 brochures"), "the model read the thread's stored body");
    // read again: kept, not made again
    rig.summary("t1");
    rig.settle();
    assert_eq!(twin.calls(), 1, "a kept summary is read, never made again");
    // a restart: the same words come from the store, with no call
    let rig = rig.restart();
    assert_eq!(rig.summary("t1"), quoted("A quote for 12,000 brochures; answer by Tuesday."), "the summary survives a restart");
    rig.settle();
    assert_eq!(twin.calls(), 1, "and nothing is asked after the restart");
    // a mail's text never reaches a log line
    let log = std::fs::read_to_string(rig.dir.path().join("core.log")).unwrap_or_default();
    assert!(!log.contains("BODYWORDS") && !log.contains("12,000 brochures"), "the core log holds no mail text");
    ai_settings::set_scripted_transport(&rig.path, None);
    println!("PASS a_thread_gets_its_summary_from_the_writing_model_and_keeps_it_across_a_restart");
}

#[test]
fn a_new_message_in_the_thread_refreshes_the_summary() {
    let rig = Rig::new();
    let twin = Twin::new(&["First summary.", "Second summary, with the new price."], Duration::ZERO);
    rig.twin(&twin);
    rig.writer_on_this_mac();
    rig.summary("t1");
    rig.settle();
    assert_eq!(rig.summary("t1"), quoted("First summary."));
    // a newer message joins t1: the next read answers the kept words at once and makes the summary again in the background
    rig.core.store().insert_messages(vec![mail(&rig.account, "m2", "t1", 2_000, "NEWPRICE the price is now 0.80 EUR")]).unwrap();
    assert_eq!(rig.summary("t1"), quoted("First summary."), "the read never waits: the kept words meanwhile");
    rig.settle();
    assert_eq!(rig.summary("t1"), quoted("Second summary, with the new price."), "made again from the thread with its new message");
    assert_eq!(twin.calls(), 2);
    assert!(twin.seen.lock().unwrap()[1].contains("NEWPRICE"), "the second call read the new message");
    let row = rig.core.store().thread_summary("t1").unwrap().expect("t1 is stored");
    assert_eq!(row.made_from.as_deref(), Some("m2"), "kept with the newest message it read");
    ai_settings::set_scripted_transport(&rig.path, None);
    println!("PASS a_new_message_in_the_thread_refreshes_the_summary");
}

#[test]
fn no_writing_model_means_no_call_and_the_path_answers_null() {
    let rig = Rig::new();
    let twin = Twin::new(&["must not be asked"], Duration::ZERO);
    rig.twin(&twin);
    // the built-in Writer is Anthropic's model and no key is kept: no writing model
    assert!(rig.core.model_for(ai_settings::AiJob::Draft).is_err(), "no writing model is set");
    assert_eq!(rig.summary("t1").as_deref(), Some("null"));
    rig.settle();
    assert_eq!(rig.summary("t1").as_deref(), Some("null"), "still null: nothing was made");
    rig.settle();
    assert_eq!(twin.calls(), 0, "no model call");
    assert_eq!(rig.summary("no-such-thread").as_deref(), Some("null"), "an unknown thread answers null too");
    ai_settings::set_scripted_transport(&rig.path, None);
    println!("PASS no_writing_model_means_no_call_and_the_path_answers_null");
}

/// A fake Anthropic on 127.0.0.1 for the key check (`/models`) only: the jobs' calls go to the twin.
fn fake_models() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() { std::thread::spawn(move || models(s)); }
    });
    port
}

fn models(mut s: TcpStream) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).is_err() { return; }
    loop { let mut line = String::new(); if r.read_line(&mut line).is_err() || line.trim().is_empty() { break; } }
    let text = r#"{"data":[{"id":"claude-sonnet-5-5"}],"has_more":false}"#;
    let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

#[test]
fn a_hosted_provider_without_consent_is_never_asked() {
    let rig = Rig::new();
    let twin = Twin::new(&["Hosted summary."], Duration::ZERO);
    rig.twin(&twin);
    // the built-in Writer (Anthropic) with its key connected, and no "Allow" for Anthropic
    ai_settings::set_provider_address(&rig.path, "anthropic", Some(format!("http://127.0.0.1:{}", fake_models())));
    rig.core.run(Command::SaveProviderKey { provider_id: "anthropic".into(), key: SecretText { value: GOOD.into() } }).expect("the key is accepted for checking");
    let until = Instant::now() + SETTLE;
    while rig.core.state("ai_provider/anthropic/state".into()).as_deref() == Some("\"checking\"") && Instant::now() < until { std::thread::sleep(Duration::from_millis(10)); }
    let model = rig.core.model_for(ai_settings::AiJob::Draft).expect("the hosted Writer is usable for replies");
    assert!(!model.on_device);
    assert_eq!(rig.summary("t1").as_deref(), Some("null"));
    rig.settle();
    assert_eq!(rig.summary("t1").as_deref(), Some("null"), "no summary without the owner's Allow");
    rig.settle();
    assert_eq!(twin.calls(), 0, "the hosted provider was never asked");
    // the owner allows Anthropic: the next open makes the summary
    rig.core.run(Command::GiveAgentConsent { provider_id: "anthropic".into() }).expect("consent is kept");
    rig.summary("t1");
    rig.settle();
    assert_eq!(rig.summary("t1"), quoted("Hosted summary."), "with the Allow the hosted model writes it");
    assert_eq!(twin.calls(), 1);
    ai_settings::set_scripted_transport(&rig.path, None);
    println!("PASS a_hosted_provider_without_consent_is_never_asked");
}

#[test]
fn opening_a_thread_reads_the_stored_summary_under_50_ms_on_2000_mails() {
    let rig = Rig::new();
    // 2,000 mails in 1,000 threads (two each), the newest one in each thread the second
    let mails: Vec<NewMessage> = (0..2_000).map(|i| mail(&rig.account, &format!("b{i}"), &format!("bt{}", i / 2), 10_000 + i as i64, "Words of a bulk mail.")).collect();
    rig.core.store().insert_messages(mails).unwrap();
    // a slow writing model: a read that waited on it would take 300 ms
    let twin = Twin::new(&["Slow summary."], Duration::from_millis(300));
    rig.twin(&twin);
    rig.writer_on_this_mac();
    let newest = rig.core.store().thread_summary("bt500").unwrap().and_then(|r| r.newest).expect("bt500 has messages");
    assert_eq!(newest, "b1001");
    assert!(rig.core.store().thread_summary_put("bt500", "Kept summary.", &newest).unwrap());
    let mut open = Vec::new();
    for _ in 0..20 {
        let t0 = Instant::now();
        let thread = rig.core.thread("bt500".into()).expect("bt500 opens");
        let summary = rig.summary("bt500");
        open.push(t0.elapsed());
        assert_eq!(thread.message_ids.len(), 2);
        assert_eq!(summary, quoted("Kept summary."));
    }
    // a thread with no kept summary: the read hands it to the slow model and answers at once, twice while the model works
    let mut stale = Vec::new();
    for t in ["bt7", "bt8"] {
        let t0 = Instant::now();
        let s = rig.summary(t);
        stale.push(t0.elapsed());
        assert_eq!(s.as_deref(), Some("null"));
    }
    let worst = open.iter().chain(stale.iter()).max().copied().unwrap();
    println!("TIME open thread + summary on 2,000 mails: worst of 20 {:?}, median {:?}; first read of an unsummarised thread {:?} and {:?} (the model takes 300 ms)",
        open.iter().max().unwrap(), { let mut o = open.clone(); o.sort(); o[o.len() / 2] }, stale[0], stale[1]);
    assert!(worst < Duration::from_millis(50), "took {worst:?}, not under 50 ms");
    assert!(twin.calls() <= 2, "only the two unsummarised threads are made");
    rig.settle();
    assert_eq!(rig.summary("bt7"), quoted("Slow summary."), "made in the background meanwhile");
    ai_settings::set_scripted_transport(&rig.path, None);
    println!("PASS opening_a_thread_reads_the_stored_summary_under_50_ms_on_2000_mails");
}
