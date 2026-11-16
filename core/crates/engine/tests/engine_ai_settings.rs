//! ai-settings-core (docs/lanes/ai-settings-core.md): the real core stores and uses the AI model the owner chose. Every case runs
//! `RustCore` over a real migrated store, a fake secret store (`MemoryKeychain`, rig namespace: never the owner's Keychain) and a fake
//! provider: an HTTP server on 127.0.0.1 in this test that answers the Anthropic shapes (a good key, a refused key in the provider's
//! words, a 5xx, a slow answer). Nothing here reaches a real provider.
//!
//! Mutants (single edits; each run and seen red, the verbatim results in _captures/ai-settings-core/mutants-run.log, the runner
//! _captures/ai-settings-core/run-mutants.py):
//! - M1 ai_settings.rs `save_provider_key`: the command waits for its check thread
//! - M2 ai_settings.rs `keep`: the row's Keychain reference carries the key text
//! - M3 ai_settings.rs `apply`: a refusal loses the provider's words
//! - M4 ai_settings.rs `apply`: an accepted key is written to core.log
//! - M5 ai_settings.rs `save_provider_key`: a refusal's words are the key
//! - M6 ai_settings.rs `save_provider_key`: a refused key is kept as if accepted
//! - M7 ai_settings.rs `remove_provider_key`: the Keychain item is not deleted
//! - M8 store ai_settings.rs `ai_provider_forget`: the provider row is not deleted
//! - M9 ai_settings.rs `model_for`: a key provider is used without a saved key
//! - M10 ai_settings.rs `model_for`: the drafts read another agent than the Writer
//! - M11 ai_settings.rs `state`: `agent_config/<id>/allowed` is served under another name
//! - M12 rust_core.rs `state`: the AI settings line is removed
//! - M13 ai_settings.rs `apply`: an older check's answer is applied after a newer save
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings::{self, AiJob, AI_NO_MODEL_CODE};
use engine::{Keyring, RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace, ReadSecret, SecretKind};
use store::Store;

const GOOD: &str = "FIXTURE-sk-ant-good-7f3c91d2";
const SLOW: &str = "FIXTURE-sk-ant-slow-2b8e40aa";
const REFUSED: &str = "FIXTURE-sk-ant-refused-9d1a77c0";
const FAULT: &str = "FIXTURE-sk-ant-fault-4c6e05b3";
/// The provider's own words for a refused key (Anthropic's 401 body) and for an overloaded server.
const REFUSED_WORDS: &str = "invalid x-api-key";
const FAULT_WORDS: &str = "Overloaded, try again in a moment";
const MODELS: [&str; 2] = ["claude-sonnet-5-5", "claude-haiku-5-5"];
/// The fake's slow answer: ten times the owner's 50 ms bound for a command.
const SLOW_MS: u64 = 500;
const COMMAND_BOUND_MS: u128 = 50;

// ---------------------------------------------------------------- the fake provider

struct FakeProvider { port: u16, completions: Arc<Mutex<Vec<String>>> }

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

fn answer(mut s: TcpStream, completions: Arc<Mutex<Vec<String>>>) {
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
    let key = header(&headers, "x-api-key").map(String::from).or_else(|| header(&headers, "authorization").and_then(|v| v.strip_prefix("Bearer ")).map(String::from)).unwrap_or_default();
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    let models = format!("{{\"data\":[{{\"id\":\"{}\"}},{{\"id\":\"{}\"}}],\"has_more\":false}}", MODELS[0], MODELS[1]);
    let (status, text) = match (key.as_str(), path.as_str()) {
        (SLOW, "/models") => { std::thread::sleep(Duration::from_millis(SLOW_MS)); (200, models) }
        (GOOD, "/models") => (200, models),
        (GOOD | SLOW, "/messages") => {
            completions.lock().unwrap().push(key.clone());
            (200, r#"{"content":[{"type":"text","text":"A drafted reply."}]}"#.to_string())
        }
        (FAULT, _) => (503, format!(r#"{{"error":{{"message":"{FAULT_WORDS}"}}}}"#)),
        _ => (401, format!(r#"{{"type":"error","error":{{"type":"authentication_error","message":"{REFUSED_WORDS}"}}}}"#)),
    };
    let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

fn fake_provider() -> FakeProvider {
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
    FakeProvider { port, completions }
}

// ---------------------------------------------------------------- the core

struct Rig { core: RustCore, keys: Arc<MemoryKeychain>, dir: tempfile::TempDir, fake: FakeProvider, _fenced: Fenced }
/// The fence (the lead's ruling, round 5): from the first test on, every name a test of this file resolves through netguard's lookup (the
/// provider client's resolver, the guarded dialers) is answered with this Mac's counting listener and recorded, so no test and no mutant run
/// can look up, or reach, a name past this Mac. Each test ends with the check (`Fenced` drop); `no_path_of_this_file_looks_up_a_name_past_this_mac`
/// drives the outgoing paths on purpose.
/// The address tests point at themselves for "not this Mac": a connection to it is answered on this Mac and never leaves it.
const UNSPECIFIED: &str = "0.0.0.0";
struct Fence { names: Mutex<Vec<String>>, listener: std::net::TcpListener, port: u16 }
impl netguard::Resolve for Fence {
    fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).push(host.to_string());
        // this Mac's own names keep their port; 0.0.0.0 (the tests' own "not this Mac" that never leaves it) stays itself; any other name
        // is sent to the counting listener, never past this Mac
        if netguard::is_this_mac(host) { return Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], port))]); }
        if host == UNSPECIFIED { return Ok(vec![std::net::SocketAddr::from(([0, 0, 0, 0], port))]); }
        Ok(vec![std::net::SocketAddr::from(([127, 0, 0, 1], self.port))])
    }
}
fn fence() -> &'static Arc<Fence> {
    static F: std::sync::OnceLock<Arc<Fence>> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let f = Arc::new(Fence { names: Mutex::new(Vec::new()), listener, port });
        netguard::set_resolver(f.clone());
        f
    })
}
/// The names past this Mac any test looked up so far, and whether anything reached the counting listener.
fn fence_breaches() -> (Vec<String>, bool) {
    let f = fence();
    let names = f.names.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|h| !netguard::is_this_mac(h) && h.as_str() != UNSPECIFIED).cloned().collect();
    (names, f.listener.accept().is_ok())
}
struct Fenced;
impl Drop for Fenced {
    fn drop(&mut self) {
        if std::thread::panicking() { return; }
        let (names, reached) = fence_breaches();
        assert!(names.is_empty() && !reached, "a test looked up a name past this Mac {names:?} or reached the fence's listener ({reached})");
    }
}


impl Rig {
    fn new() -> Rig {
        fence();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
        let core = RustCore::with_parts(store, keys.clone(), detector, TestTransport::Real);
        let fake = fake_provider();
        for slug in ["anthropic", "openai"] {
            ai_settings::set_provider_address(&core.store().path().to_path_buf(), slug, Some(format!("http://127.0.0.1:{}", fake.port)));
        }
        Rig { core, keys, dir, fake, _fenced: Fenced }
    }
    fn keyring(&self) -> Arc<dyn Keyring> { self.keys.clone() }
    fn state(&self, path: &str) -> Option<String> { self.core.state(path.to_string()) }
    fn save(&self, provider: &str, key: &str) -> Result<CommandOutcome, CoreError> {
        self.core.run(Command::SaveProviderKey { provider_id: provider.into(), key: SecretText { value: key.into() } })
    }
    /// Waits for the provider's check to land (the state leaves "checking"), at most 5 s.
    fn settle(&self, provider: &str) -> String {
        let path = format!("ai_provider/{provider}/state");
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let s = self.state(&path).unwrap_or_default();
            if s != "\"checking\"" || Instant::now() > until { return s; }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    /// What the secret store holds for a provider, as text (None = no item).
    fn kept(&self, provider: &str) -> Option<String> {
        let r = Namespace::rig().reference(SecretKind::ProviderCredential, provider).unwrap();
        ReadSecret::execute(&*self.keys, &r).ok().map(|s| String::from_utf8(s.expose().to_vec()).unwrap())
    }
    fn files_named(&self, prefix: &str) -> Vec<PathBuf> {
        std::fs::read_dir(self.dir.path()).unwrap().flatten().map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix)).collect()
    }
    fn log(&self) -> String { std::fs::read_to_string(self.dir.path().join("core.log")).unwrap_or_default() }
    /// Every AI state value the core serves for these providers, joined.
    fn every_state(&self) -> String {
        let mut out = String::new();
        for id in ["anthropic", "openai", "gemini", "openrouter", "local", "jev"] {
            for leaf in ["state", "refusal", "connected", "models", "key"] { out += &format!("{:?}\n", self.state(&format!("ai_provider/{id}/{leaf}"))); }
            out += &format!("{:?}\n", self.state(&format!("provider_credential/{id}/stored")));
        }
        out += &format!("{:?}\n{:?}\n", self.state("speech_engine/chosen/state"), self.state("agent_config/writer/allowed"));
        out
    }
}

fn contains(hay: &[u8], needle: &str) -> bool { hay.windows(needle.len()).any(|w| w == needle.as_bytes()) }

fn refusal_code(e: &CoreError) -> Option<String> {
    match e { CoreError::Action { failure } => failure.code.clone(), _ => None }
}

// ---------------------------------------------------------------- the cases

#[test]
fn save_key_answers_at_once_while_the_provider_takes_500_ms() {
    let rig = Rig::new();
    let started = Instant::now();
    rig.save("anthropic", SLOW).expect("the command answers");
    let took = started.elapsed().as_millis();
    assert!(took < COMMAND_BOUND_MS, "SaveProviderKey took {took} ms while the provider answers in {SLOW_MS} ms");
    assert_eq!(rig.state("ai_provider/anthropic/state").as_deref(), Some("\"checking\""));
    assert_eq!(rig.state("provider_credential/anthropic/stored").as_deref(), Some("false"), "nothing is kept before the provider answers");
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    assert_eq!(rig.state("ai_provider/anthropic/models").as_deref(), Some("[\"claude-sonnet-5-5\",\"claude-haiku-5-5\"]"));
    let p = rig.core.ai_providers().into_iter().find(|p| p.id == "anthropic").unwrap();
    assert!(p.connected && p.models == MODELS);
}

#[test]
fn an_accepted_key_is_in_the_secret_store_and_nowhere_in_the_database_the_state_or_core_log() {
    let rig = Rig::new();
    rig.save("anthropic", GOOD).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    assert_eq!(rig.kept("anthropic").as_deref(), Some(GOOD), "the key is in the secret store");
    let p = rig.core.ai_providers().into_iter().find(|p| p.id == "anthropic").unwrap();
    let r = p.keychain.clone().expect("the row names its Keychain item");
    assert_eq!(String::from_utf8(ReadSecret::execute(&*rig.keys, &r).unwrap().expose().to_vec()).unwrap(), GOOD, "the row's reference opens the key");
    assert_eq!(rig.state("provider_credential/anthropic/stored").as_deref(), Some("true"));
    assert_eq!(rig.state("ai_provider/anthropic/key"), None, "a key is never read back");
    let db = rig.files_named("whoosh.sqlite");
    assert!(!db.is_empty());
    for f in &db { assert!(!contains(&std::fs::read(f).unwrap(), GOOD), "{} holds the key", f.display()); }
    assert!(!rig.every_state().contains(GOOD), "a state value holds the key");
    assert!(!format!("{:?}", rig.core.ai_providers()).contains(GOOD), "the provider rows hold the key");
    let log = rig.log();
    assert!(log.contains("ai key anthropic: accepted"), "core.log: {log:?}");
    assert!(!log.contains(GOOD), "core.log holds the key");
}

#[test]
fn a_refused_key_stores_nothing_and_shows_the_providers_words() {
    let rig = Rig::new();
    rig.save("anthropic", REFUSED).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"refused\"");
    assert_eq!(rig.state("ai_provider/anthropic/refusal"), Some(format!("\"{REFUSED_WORDS}\"")));
    assert_eq!(rig.state("provider_credential/anthropic/stored").as_deref(), Some("false"));
    assert_eq!(rig.state("ai_provider/anthropic/connected").as_deref(), Some("false"));
    assert_eq!(rig.kept("anthropic"), None, "the secret store holds nothing");
    assert!(rig.core.store().ai_provider_rows().unwrap().is_empty(), "the store holds no provider row");
    let log = rig.log();
    assert!(log.contains(&format!("ai key anthropic: refused: {REFUSED_WORDS}")), "core.log: {log:?}");
    assert!(!log.contains(REFUSED) && !rig.every_state().contains(REFUSED), "the refused key reached core.log or a state value");
    for f in rig.files_named("whoosh.sqlite") { assert!(!contains(&std::fs::read(&f).unwrap(), REFUSED), "{} holds the refused key", f.display()); }
}

#[test]
fn a_server_fault_stores_nothing_and_shows_its_words() {
    let rig = Rig::new();
    rig.save("anthropic", FAULT).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"refused\"");
    assert_eq!(rig.state("ai_provider/anthropic/refusal"), Some(format!("\"{FAULT_WORDS}\"")));
    assert_eq!(rig.kept("anthropic"), None);
    assert_eq!(rig.state("provider_credential/anthropic/stored").as_deref(), Some("false"));
}

#[test]
fn a_newer_save_wins_over_an_older_slow_check() {
    let rig = Rig::new();
    rig.save("anthropic", SLOW).unwrap();
    rig.save("anthropic", REFUSED).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"refused\"");
    std::thread::sleep(Duration::from_millis(SLOW_MS + 300));
    assert_eq!(rig.state("ai_provider/anthropic/state").as_deref(), Some("\"refused\""), "the older, slower check landed after the newer one");
    assert_eq!(rig.kept("anthropic"), None, "the older key was kept");
}

#[test]
fn remove_deletes_the_key_and_the_row() {
    let rig = Rig::new();
    rig.save("anthropic", GOOD).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    assert!(rig.kept("anthropic").is_some());
    ai_settings::remove_provider_key(&rig.core.store(), &rig.keyring(), "anthropic").unwrap();
    assert_eq!(rig.kept("anthropic"), None, "the Keychain item is gone");
    assert!(rig.core.store().ai_provider_row("anthropic").unwrap().is_none(), "the provider row is gone");
    assert_eq!(rig.state("ai_provider/anthropic/state").as_deref(), Some("\"not connected\""));
    assert_eq!(rig.state("provider_credential/anthropic/stored").as_deref(), Some("false"));
    ai_settings::remove_provider_key(&rig.core.store(), &rig.keyring(), "anthropic").expect("removing twice is not an error");
    assert!(matches!(ai_settings::remove_provider_key(&rig.core.store(), &rig.keyring(), "nowhere"), Err(CoreError::NotFound { .. })));
}

#[test]
fn the_model_to_use_is_refused_in_words_before_a_save_and_is_the_provider_after() {
    let rig = Rig::new();
    let store = rig.core.store();
    let before = ai_settings::model_for(&store, &rig.keyring(), AiJob::Draft).err().expect("no model before a key is saved");
    assert_eq!(refusal_code(&before).as_deref(), Some(AI_NO_MODEL_CODE));
    assert!(before.to_string().contains("Anthropic is not connected"), "{before}");
    rig.save("anthropic", GOOD).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    for job in [AiJob::Draft, AiJob::VoiceAnalysis] {
        let m = ai_settings::model_for(&store, &rig.keyring(), job).unwrap_or_else(|e| panic!("{job:?}: {e}"));
        assert_eq!((m.provider_name, m.model.as_str(), m.on_device), ("Anthropic", "claude-sonnet-5-5", false));
        assert!(m.key.is_some() && !format!("{:?}", m.key).contains(GOOD), "the choice holds a reference, never the key");
    }
    // the key is read from the secret store at the call: the fake provider sees it on the wire
    let m = ai_settings::model_for(&store, &rig.keyring(), AiJob::Draft).unwrap();
    assert_eq!(m.complete(None, "Reply to this".into(), 100).unwrap(), "A drafted reply.");
    assert_eq!(*rig.fake.completions.lock().unwrap(), vec![GOOD.to_string()]);
    // the owner moves the Writer to OpenAI, which holds no key: refused again, in words
    let mut writer = rig.core.agent_configs().into_iter().find(|a| a.id == "writer").unwrap();
    writer.provider_id = "openai".into();
    writer.model = "gpt-5".into();
    rig.core.run(Command::SaveAgentConfig { config: writer }).unwrap();
    let e = ai_settings::model_for(&store, &rig.keyring(), AiJob::Draft).err().unwrap();
    assert!(e.to_string().contains("OpenAI is not connected"), "{e}");
    let labels = ai_settings::model_for(&store, &rig.keyring(), AiJob::Labels).err().unwrap();
    assert_eq!(refusal_code(&labels).as_deref(), Some(AI_NO_MODEL_CODE));
    assert!(labels.to_string().contains("No model is set for sorting mail: choose one in Settings, Decisions."), "{labels}"); // labels-r5
}

#[test]
fn agents_are_saved_as_the_fixture_core_saves_them() {
    let rig = Rig::new();
    let names: Vec<String> = rig.core.agent_configs().into_iter().map(|a| a.id).collect();
    assert_eq!(names, vec!["inbox", "writer"], "the built-in agents");
    let draft = AgentConfig { id: String::new(), name: "  Supplier desk ".into(), purpose: String::new(), provider_id: "anthropic".into(), model: "claude-haiku-5-5".into(), instructions: String::new(), allowed: vec![AgentPermission::ReadMail, AgentPermission::Search, AgentPermission::Label], built_in: true };
    let id = rig.core.run(Command::SaveAgentConfig { config: draft.clone() }).unwrap().created_id.expect("a new agent's id");
    assert_eq!(rig.state("agent_config/Supplier desk/allowed").as_deref(), Some("[\"readMail\",\"search\",\"label\"]"));
    let saved = rig.core.agent_configs().into_iter().find(|a| a.id == id).unwrap();
    assert_eq!((saved.name.as_str(), saved.built_in), ("Supplier desk", false), "the name is trimmed; an agent the owner made is never built in");
    let again = rig.core.run(Command::SaveAgentConfig { config: draft.clone() }).unwrap_err();
    assert!(again.to_string().contains("exists already"), "{again}");
    let nameless = rig.core.run(Command::SaveAgentConfig { config: AgentConfig { name: " ".into(), ..draft.clone() } }).unwrap_err();
    assert!(matches!(nameless, CoreError::Invalid { ref field, .. } if field == "name"));
    let nowhere = rig.core.run(Command::SaveAgentConfig { config: AgentConfig { name: "Other".into(), provider_id: "nowhere".into(), ..draft.clone() } }).unwrap_err();
    assert!(matches!(nowhere, CoreError::Invalid { ref field, .. } if field == "provider"));
    let ghost = rig.core.run(Command::SaveAgentConfig { config: AgentConfig { id: "agent-99".into(), ..draft } }).unwrap_err();
    assert!(matches!(ghost, CoreError::NotFound { .. }));
    // a built-in saved by the owner stays built in and replaces the default
    let mut writer = rig.core.agent_configs().into_iter().find(|a| a.id == "writer").unwrap();
    writer.allowed = vec![AgentPermission::ReadMail, AgentPermission::Draft];
    writer.built_in = false;
    rig.core.run(Command::SaveAgentConfig { config: writer }).unwrap();
    let writer = rig.core.agent_configs().into_iter().find(|a| a.id == "writer").unwrap();
    assert!(writer.built_in);
    assert_eq!(rig.state("agent_config/writer/allowed").as_deref(), Some("[\"readMail\",\"draft\"]"));
    assert_eq!(rig.core.agent_configs().len(), 3);
}

#[test]
fn the_speech_engine_is_chosen_as_the_fixture_core_chooses_it() {
    let rig = Rig::new();
    assert_eq!(rig.core.speech_engine(), SpeechEngine { kind: SpeechEngineKind::Local, ready: false });
    assert_eq!(rig.state("speech_engine/chosen/state"), None, "nothing before the first choice");
    rig.core.run(Command::ChooseSpeechEngine { kind: SpeechEngineKind::Local }).unwrap();
    assert_eq!(rig.core.speech_engine(), SpeechEngine { kind: SpeechEngineKind::Local, ready: true });
    assert_eq!(rig.state("speech_engine/chosen/state").as_deref(), Some("\"ready\""));
    let cloud = SpeechEngineKind::Cloud { provider_id: "openai".into() };
    let e = rig.core.run(Command::ChooseSpeechEngine { kind: cloud.clone() }).unwrap_err();
    assert!(e.to_string().contains("Add your OpenAI key"), "{e}");
    assert_eq!(rig.state("speech_engine/chosen/state").as_deref(), Some("\"needs key\""));
    rig.save("openai", GOOD).unwrap();
    assert_eq!(rig.settle("openai"), "\"connected\"");
    rig.core.run(Command::ChooseSpeechEngine { kind: cloud.clone() }).unwrap();
    assert_eq!(rig.core.speech_engine(), SpeechEngine { kind: cloud, ready: true });
    // forgetting the provider takes its speech engine back to none ready
    ai_settings::remove_provider_key(&rig.core.store(), &rig.keyring(), "openai").unwrap();
    assert_eq!(rig.core.speech_engine(), SpeechEngine { kind: SpeechEngineKind::Local, ready: false });
}

/// Jev (the decision model): its key is kept with no request made (no Jev key check is written down in this repo), its row says
/// the contract's "not checked", the key is in the fake keyring and nowhere in the database or core.log; remove deletes both; an empty key is refused.
/// Mutants J1 (Jev's row keeps the key text), J2 (an empty Jev key is kept), J3 (remove leaves Jev's Keychain item).
#[test]
fn jevs_key_is_kept_unchecked_and_removed() {
    const JEV_KEY: &str = "FIXTURE-tsa-jev-key-5e21";
    let rig = Rig::new();
    let empty = rig.save("jev", "  \n").unwrap_err();
    assert!(matches!(empty, CoreError::Invalid { ref field, .. } if field == "key"), "{empty:?}");
    assert_eq!(rig.kept("jev"), None, "an empty key is not kept");
    assert_eq!(rig.state("ai_provider/jev/state").as_deref(), Some("\"not connected\""));
    let started = Instant::now();
    let out = rig.save("jev", &format!("  {JEV_KEY} ")).expect("Jev's key is kept");
    assert!(started.elapsed().as_millis() < COMMAND_BOUND_MS, "no request is made: it answers at once");
    assert_eq!(rig.kept("jev").as_deref(), Some(JEV_KEY), "trimmed, in the keyring");
    let reference = out.created_id.expect("the stored reference");
    let (service, account) = reference.split_once('\u{1f}').expect("service U+001F account");
    assert_eq!(account, "jev");
    assert!(service.ends_with("provider-credential"), "{service}");
    assert_eq!(rig.state("ai_provider/jev/state").as_deref(), Some("\"not checked\""));
    assert_eq!(rig.state("provider_credential/jev/stored").as_deref(), Some("true"));
    assert_eq!(rig.state("ai_provider/jev/models").as_deref(), Some("[]"), "Jev lists no models");
    let row = rig.core.ai_providers().into_iter().find(|p| p.id == "jev").expect("Jev's row");
    assert!(row.connected && row.keychain.as_ref().is_some_and(|k| k.account == "jev"));
    assert!(rig.fake.completions.lock().unwrap().is_empty(), "nothing was asked of any provider");
    for f in rig.files_named("whoosh.sqlite") { assert!(!contains(&std::fs::read(&f).unwrap(), JEV_KEY), "{} holds Jev's key", f.display()); }
    assert!(!rig.every_state().contains(JEV_KEY) && !rig.log().contains(JEV_KEY), "Jev's key reached a state value or core.log");
    assert!(rig.log().contains("ai key jev: kept, not checked"));
    ai_settings::remove_provider_key(&rig.core.store(), &rig.keyring(), "jev").unwrap();
    assert_eq!(rig.kept("jev"), None, "the Keychain item is gone");
    assert!(rig.core.store().ai_provider_row("jev").unwrap().is_none(), "the row is gone");
    assert_eq!(rig.state("provider_credential/jev/stored").as_deref(), Some("false"));
}

/// A key a job may use is the contract's `provider_state_is_usable`: while a newly pasted key is checked ("checking"), the Writer's model is
/// refused in words, and usable again once the check lands "connected". Mutant U1 (`model_for` no longer asks `provider_state_is_usable`).
#[test]
fn a_job_uses_a_key_only_in_a_usable_state() {
    let rig = Rig::new();
    let store = rig.core.store();
    rig.save("anthropic", GOOD).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    assert!(ai_settings::model_for(&store, &rig.keyring(), AiJob::Draft).is_ok());
    rig.save("anthropic", SLOW).unwrap();
    assert_eq!(rig.state("ai_provider/anthropic/state").as_deref(), Some("\"checking\""));
    let e = ai_settings::model_for(&store, &rig.keyring(), AiJob::Draft).err().expect("not usable while checking");
    assert_eq!(refusal_code(&e).as_deref(), Some(AI_NO_MODEL_CODE));
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    assert!(ai_settings::model_for(&store, &rig.keyring(), AiJob::Draft).is_ok(), "usable again");
}

/// The rig guard never reaches the owner's copy: this binary opens no rig core, so netguard lets any host through, the counter path
/// answers nothing, `apply` of the owner's switches (`rig` false at `open_production`) does not make the process a rig, and an IMAP host
/// past this Mac is tried (its refusal is the network's, not the guard's). Mutant G1 (`apply` marks the process a rig whatever `rig` says).
#[test]
fn the_rig_guard_never_reaches_the_owners_copy() {
    let rig = Rig::new();
    engine::rig::apply(&engine::rig::RigSwitches::default(), rig.core.store().path());
    assert!(!engine::rig::is_rig_process(), "the owner's switches never mark a rig nor close the process");
    assert!(netguard::outbound_allowed("api.anthropic.com").is_ok() && netguard::outbound_allowed_url("https://api.typesafe.ai/v1/systemone").is_ok());
    assert_eq!(rig.state("rig/refused-outbound"), None);
    let config = AccountConfig {
        imap_host: "0.0.0.0".into(), imap_port: 9, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 0,
        smtp_security: ConnectionSecurity::Starttls, username: "o@x.test".into(), smtp_username: String::new(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    };
    let report = rig.core.validate_account(config, SecretText { value: "pw".into() });
    assert!(!report.ok && !report.summary_words.contains("is not this Mac"), "tried, refused by the network only: {}", report.summary_words);
}

#[test]
fn an_unknown_provider_or_an_empty_key_is_refused_at_once() {
    let rig = Rig::new();
    assert!(matches!(rig.save("ollama", GOOD), Err(CoreError::NotFound { .. })), "a local server takes no key");
    assert!(matches!(rig.save("anthropic", "   "), Err(CoreError::Invalid { .. })));
    assert_eq!(rig.state("ai_provider/anthropic/state").as_deref(), Some("\"not connected\""));
}

/// The fixture core's AI state paths (Sources/CoreBridge/FixtureCore.swift `state`), read from its source: `prefix/<p1>/<p2>` with
/// `*` where the fixture takes any id.
fn fixture_ai_paths() -> BTreeSet<String> {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Sources/CoreBridge/FixtureCore.swift")).expect("the fixture core's source");
    let mut out = BTreeSet::new();
    for line in src.lines() {
        let l = line.trim();
        for prefix in ["ai_provider", "provider_credential", "speech_engine", "agent_config"] {
            let Some(rest) = l.strip_prefix(&format!("case (\"{prefix}\", 3) where ")) else { continue };
            let cond = rest.split(':').next().unwrap_or("");
            let fixed = |i: usize| cond.split(&format!("p[{i}] == \"")).nth(1).and_then(|s| s.split('"').next()).map(String::from).unwrap_or_else(|| "*".into());
            out.insert(format!("{prefix}/{}/{}", fixed(1), fixed(2)));
        }
    }
    out
}

#[test]
fn the_ai_state_paths_and_their_json_are_the_fixture_cores() {
    // the key set: the fixture's AI paths are exactly these, each with the JSON kind it answers
    // since contracts delta 24b the fixture serves /state, /refusal and /models too (the two-step SaveProviderKey): one key set for both cores
    let expected: [(&str, &str); 8] = [
        ("agent_config/*/allowed", "strings"),
        ("ai_provider/*/connected", "bool"),
        ("ai_provider/*/key", "absent"),
        ("ai_provider/*/models", "strings"),
        ("ai_provider/*/refusal", "absent"),   // read after an ACCEPTED key: the words exist only while a key is refused (a string then)
        ("ai_provider/*/state", "string"),
        ("provider_credential/*/stored", "bool"),
        ("speech_engine/chosen/state", "string"),
    ];
    let fixture = fixture_ai_paths();
    assert_eq!(fixture, expected.iter().map(|(p, _)| p.to_string()).collect::<BTreeSet<_>>(), "the fixture core's AI state paths changed: bring the real core along");
    let rig = Rig::new();
    rig.save("anthropic", GOOD).unwrap();
    assert_eq!(rig.settle("anthropic"), "\"connected\"");
    rig.core.run(Command::ChooseSpeechEngine { kind: SpeechEngineKind::Local }).unwrap();
    for (pattern, kind) in expected {
        let path = pattern.replace("agent_config/*", "agent_config/writer").replace('*', "anthropic");
        let got = rig.state(&path);
        let parsed: Option<serde_json::Value> = got.as_deref().map(|g| serde_json::from_str(g).unwrap_or_else(|e| panic!("{path}: {g:?} is not JSON: {e}")));
        let ok = match (kind, &parsed) {
            ("absent", None) => true,
            ("bool", Some(v)) => v.is_boolean(),
            ("string", Some(v)) => v.is_string(),
            ("strings", Some(v)) => v.as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|x| x.is_string())),
            _ => false,
        };
        assert!(ok, "{path}: the fixture answers {kind}, the real core {got:?}");
    }
    // the records are the contract's: the fixture's provider ids are served
    let ids: BTreeSet<String> = rig.core.ai_providers().into_iter().map(|p| p.id).collect();
    for id in ["anthropic", "openai", "local"] { assert!(ids.contains(id), "{id} missing from {ids:?}"); }
}
