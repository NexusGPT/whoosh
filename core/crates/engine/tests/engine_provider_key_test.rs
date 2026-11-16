//! fix-teachcore, part A (docs/lanes/hid-ai.md F6; docs/DECISION-MODEL.md:63 "a Test button that sends one fixed sample"): the REAL engine serves
//! `Command::TestProviderKey { provider_id: "jev" }`. Every case runs `RustCore` over a real migrated store, a fake secret store (`MemoryKeychain`, rig
//! namespace: never the owner's Keychain) and a Jev twin: an HTTP server on 127.0.0.1 in this test that answers `POST /v1/systemone` in Jev's shapes (a
//! good key: an `answers` object; a refused key: 401 with Jev's words, which echo the key). Jev's address is pointed at the twin with
//! `ai_settings::set_provider_address(<store>, "jev", ..)`, and every name this file's clients resolve goes to a stub that answers this Mac only and
//! records the name (`fence`), so no case and no broken copy can reach api.typesafe.ai. Each test prints `PASS <case>`; the command's time is printed.
//!
//! BROKEN COPIES (each `find` occurs once in core/crates/engine/src/ai_settings.rs; the named test goes red):
//! - T1 `log(store, &format!("ai key {JEV_ID}: kept, {}", ai_provider_state_key(provider_state_after_save(false))));` =>
//!   `let _ = ask_jev_sample(&b, keychain.clone(), r.clone());`: saving_jevs_key_sends_nothing
//! - T2 `let answer = ask_jev_sample(&b2, keys, key);` => `let answer: Result<(), String> = Ok(());`: the_test_sends_one_fixed_sample_to_the_twin_and_reads_connected
//! - T3 `return if accepted { provider_state_after_check(true) } else { provider_state_after_save(false) };` => `return provider_state_after_save(false);`:
//!   the_test_sends_one_fixed_sample_to_the_twin_and_reads_connected
//! - T4 `b2.finish(JEV_ID, generation, answer.err());` => `b2.finish(JEV_ID, generation, answer.err().map(|_| "refused".to_string()));`:
//!   a_refused_key_reads_refused_with_jevs_own_words_and_the_key_is_kept_out_of_them
//! - T5 `return Err(CoreError::Invalid { field: "key".into(), reason_words: JEV_NO_KEY_WORDS.into() });` =>
//!   `return Err(CoreError::Invalid { field: "key".into(), reason_words: String::new() });`: with_no_key_the_test_is_refused_in_words_and_sends_nothing
//! - T6 `log(store, &format!("ai key {JEV_ID}: test sent"));` =>
//!   `log(store, &format!("ai key {JEV_ID}: test sent {:?}", ReadSecret::execute(&**keychain, &key).map(|s| String::from_utf8_lossy(s.expose()).to_string())));`:
//!   the_test_sends_one_fixed_sample_to_the_twin_and_reads_connected (the key reaches core.log)
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const GOOD: &str = "FIXTURE-tsa-jev-good-31c7a0";
const BAD: &str = "FIXTURE-tsa-jev-bad-8e44f2";
/// Jev's own words for a refused key, as the twin writes them (they echo the key; the client puts `<key>` in its place: `jev_words`).
fn refused_body(key: &str) -> String { format!(r#"{{"error":{{"message":"Invalid API key: {key}"}}}}"#) }
const REFUSED_WORDS: &str = "Invalid API key: <key>";
const COMMAND_BOUND_MS: u128 = 50;

// ---------------------------------------------------------------- the fence: every name looked up is this Mac's

struct Fence { names: Mutex<Vec<String>> }
impl netguard::Resolve for Fence {
    fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).push(host.to_string());
        // port 9 on this Mac: nothing answers there, so a request for a name past this Mac fails here and never leaves
        let _ = port;
        Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], 9))])
    }
}
fn fence() -> &'static Arc<Fence> {
    static F: OnceLock<Arc<Fence>> = OnceLock::new();
    F.get_or_init(|| {
        let f = Arc::new(Fence { names: Mutex::new(Vec::new()) });
        netguard::set_resolver(f.clone());
        f
    })
}
fn names_past_this_mac() -> Vec<String> {
    fence().names.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|h| !netguard::is_this_mac(h)).cloned().collect()
}

// ---------------------------------------------------------------- the Jev twin

#[derive(Debug, Clone)]
struct Seen { path: String, authorization: String, body: String }

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

fn answer(mut s: TcpStream, seen: Arc<Mutex<Vec<Seen>>>) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).is_err() { return; }
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).is_err() || line.trim().is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') { headers.push((k.trim().to_string(), v.trim().to_string())); }
    }
    let len: usize = header(&headers, "content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let authorization = header(&headers, "authorization").unwrap_or("").to_string();
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    seen.lock().unwrap().push(Seen { path: path.clone(), authorization: authorization.clone(), body: String::from_utf8_lossy(&body).to_string() });
    let key = authorization.strip_prefix("Bearer ").unwrap_or("");
    let (status, text) = match (key, path.as_str()) {
        (GOOD, "/v1/systemone") => (200, r#"{"model":"jev-latest","answers":{"which_tab":{"choice":"Updates","confidence":0.91}}}"#.to_string()),
        (_, "/v1/systemone") => (401, refused_body(key)),
        _ => (404, r#"{"detail":"no such route"}"#.to_string()),
    };
    let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

/// The twin on 127.0.0.1: its port and every request it was sent.
fn jev_twin() -> (u16, Arc<Mutex<Vec<Seen>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || for s in l.incoming().flatten() { let s2 = s2.clone(); std::thread::spawn(move || answer(s, s2)); });
    (port, seen)
}

// ---------------------------------------------------------------- the core

struct Rig { core: RustCore, dir: tempfile::TempDir, seen: Arc<Mutex<Vec<Seen>>> }

impl Rig {
    fn new() -> Rig {
        fence();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
        let core = RustCore::with_parts(store, keys, detector, TestTransport::Real);
        let (port, seen) = jev_twin();
        ai_settings::set_provider_address(&core.store().path().to_path_buf(), "jev", Some(format!("http://127.0.0.1:{port}")));
        Rig { core, dir, seen }
    }
    fn state(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }
    fn save(&self, key: &str) -> CommandOutcome {
        self.core.run(Command::SaveProviderKey { provider_id: "jev".into(), key: SecretText { value: key.into() } }).expect("Jev's key is kept")
    }
    /// Runs the Test and prints how long the command took to answer.
    fn test_key(&self, provider: &str) -> Result<CommandOutcome, CoreError> {
        let t = Instant::now();
        let out = self.core.run(Command::TestProviderKey { provider_id: provider.into() });
        let ms = t.elapsed().as_millis();
        println!("TIME TestProviderKey {provider} {ms} ms");
        if out.is_ok() { assert!(ms < COMMAND_BOUND_MS, "TestProviderKey took {ms} ms, not under {COMMAND_BOUND_MS} ms: it must answer at once"); }
        out
    }
    /// Waits for the Test to land (the state leaves "checking"), at most 5 s.
    fn settle(&self) -> String {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let s = self.state("ai_provider/jev/state").unwrap_or_default();
            if s != "\"checking\"" || Instant::now() > until { return s; }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn requests(&self) -> Vec<Seen> { self.seen.lock().unwrap().clone() }
    fn log(&self) -> String { std::fs::read_to_string(self.dir.path().join("core.log")).unwrap_or_default() }
    /// Every Jev state value the core serves, joined.
    fn every_state(&self) -> String {
        ["state", "refusal", "connected", "models", "key"].iter().map(|leaf| format!("{:?}\n", self.state(&format!("ai_provider/jev/{leaf}")))).collect::<String>()
            + &format!("{:?}\n", self.state("provider_credential/jev/stored"))
    }
}

fn words(e: &CoreError) -> String {
    match e {
        CoreError::Invalid { reason_words, .. } | CoreError::Unexpected { reason_words } => reason_words.clone(),
        CoreError::Action { failure } => failure.reason_words.clone(),
        other => format!("{other:?}"),
    }
}

// ---------------------------------------------------------------- the cases

/// Saving Jev's key makes no network call (as before F6): the twin hears nothing and the state stays "not checked".
#[test]
fn saving_jevs_key_sends_nothing() {
    let rig = Rig::new();
    rig.save(GOOD);
    std::thread::sleep(Duration::from_millis(300));
    assert!(rig.requests().is_empty(), "saving the key sent {:?}", rig.requests());
    assert_eq!(rig.state("ai_provider/jev/state").as_deref(), Some("\"not checked\""));
    assert!(names_past_this_mac().is_empty(), "a name past this Mac was looked up: {:?}", names_past_this_mac());
    println!("PASS saving_jevs_key_sends_nothing");
}

/// The Test answers at once, then ONE request reaches the twin: POST /v1/systemone, the kept key in the Bearer header, the fixed sample (the Jev client's
/// `{state, model, questions}` with the sample's subject and the "Which tab?" question); the state reads "connected" and no refusal; the key is in no log
/// line and no state value.
#[test]
fn the_test_sends_one_fixed_sample_to_the_twin_and_reads_connected() {
    let rig = Rig::new();
    rig.save(GOOD);
    let out = rig.test_key("jev").expect("the Test is served");
    assert_eq!(out, CommandOutcome { undo_entry_id: None, created_id: None });
    assert_eq!(rig.settle(), "\"connected\"");
    let seen = rig.requests();
    assert_eq!(seen.len(), 1, "one fixed sample, not {}: {seen:?}", seen.len());
    assert_eq!(seen[0].path, "/v1/systemone");
    assert_eq!(seen[0].authorization, format!("Bearer {GOOD}"));
    let body: serde_json::Value = serde_json::from_str(&seen[0].body).expect("the sample is JSON");
    assert_eq!(body["model"], "jev-latest");
    assert!(body["state"].as_str().is_some_and(|s| s.contains("subject: Whoosh key test") && s.contains("from: sample@example.com")), "{body}");
    assert_eq!(body["questions"]["which_tab"]["instructions"], "Which inbox tab should this email sit in?");
    assert_eq!(rig.state("ai_provider/jev/refusal"), None);
    let log = rig.log();
    assert!(log.contains("ai key jev: test sent") && log.contains("ai key jev: test answered, connected"), "core.log: {log}");
    assert!(!log.contains(GOOD) && !rig.every_state().contains(GOOD), "the key reached core.log or a state value");
    // the same sample again: one more request, never more
    rig.test_key("jev").unwrap();
    assert_eq!(rig.settle(), "\"connected\"");
    assert_eq!(rig.requests().len(), 2);
    // a new save makes the Test's answer old: "not checked" again, no request
    rig.save(GOOD);
    assert_eq!(rig.state("ai_provider/jev/state").as_deref(), Some("\"not checked\""));
    assert_eq!(rig.requests().len(), 2);
    assert!(names_past_this_mac().is_empty(), "a name past this Mac was looked up: {:?}", names_past_this_mac());
    println!("PASS the_test_sends_one_fixed_sample_to_the_twin_and_reads_connected");
}

/// A key Jev refuses: "refused", and `ai_provider/jev/refusal` holds Jev's own words with the key taken out of them; core.log says the same and never
/// holds the key.
#[test]
fn a_refused_key_reads_refused_with_jevs_own_words_and_the_key_is_kept_out_of_them() {
    let rig = Rig::new();
    rig.save(BAD);
    rig.test_key("jev").expect("the Test is served");
    assert_eq!(rig.settle(), "\"refused\"");
    assert_eq!(rig.requests().len(), 1);
    assert_eq!(rig.state("ai_provider/jev/refusal"), Some(format!("\"{REFUSED_WORDS}\"")));
    let log = rig.log();
    assert!(log.contains(&format!("ai key jev: test refused: {REFUSED_WORDS}")), "core.log: {log}");
    assert!(!log.contains(BAD) && !rig.every_state().contains(BAD), "the key reached core.log or a state value");
    println!("PASS a_refused_key_reads_refused_with_jevs_own_words_and_the_key_is_kept_out_of_them");
}

/// No Jev key kept: the Test is refused in words, nothing is sent, the state stays "not connected". Another key provider is refused in words (its key is
/// checked when saved); an unknown provider is NotFound.
#[test]
fn with_no_key_the_test_is_refused_in_words_and_sends_nothing() {
    let rig = Rig::new();
    let e = rig.test_key("jev").unwrap_err();
    assert_eq!(words(&e), "Paste the Jev key first.");
    std::thread::sleep(Duration::from_millis(200));
    assert!(rig.requests().is_empty(), "a request without a key: {:?}", rig.requests());
    assert_eq!(rig.state("ai_provider/jev/state").as_deref(), Some("\"not connected\""));
    let other = rig.test_key("anthropic").unwrap_err();
    assert!(words(&other).contains("checked when its key is saved"), "{other:?}");
    assert!(matches!(rig.test_key("no-such-provider"), Err(CoreError::NotFound { .. })));
    assert!(rig.requests().is_empty());
    println!("PASS with_no_key_the_test_is_refused_in_words_and_sends_nothing");
}
