//! rig-core (docs/lanes/realcore-firstrun.testplan.md R2, R5, the new mutant rows; docs/lanes/ai-settings-core.md round 2): the hidden
//! test copy's switches on the REAL core. R2 runs the product's own path (SaveProviderKey, the background check, the model list, the
//! Writer's model choice, a completion) against checks/realcore_aifake.py itself, started by the test (`--serve`) on 127.0.0.1. No real
//! provider, no real Keychain: every core here is a rig core (`open_production(.., true)`) or `with_parts` over a memory keyring.
//!
//! The tests of this file run one at a time (`serial`): one sets process environment variables, as a rig launch does.
//!
//! Mutants (single edits; each run and seen red, verbatim results in _captures/ai-settings-core/rig-mutants-run.log, runner
//! _captures/ai-settings-core/run-rig-mutants.py): R1 rig.rs `from_env` reads the variables on the owner's copy too; R2 ai_settings.rs
//! `route_key_providers_to_loopback` routes nothing; R3 rust_core.rs the `rig::apply` line removed; R4 rust_core.rs the rig keyring is a
//! memory keyring again; R5 rust_core.rs the no-account-stored line removed; R6 bodies.rs the bodies-never-stored line removed; R7
//! indexer.rs the index-never-writes filter removed; R8 ai_settings.rs choices-in-memory also writes the store; R9 rig.rs `dial_out_to`
//! opens nothing; R10 ai_settings.rs slow-save sleeps 0; R11 ai_settings.rs `agent_config/<id>/model` not served; R12 ai_settings.rs
//! `loopback_url` routes every host.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command as Process, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings::{loopback_url, AiJob};
use engine::bodies::BodySource;
use engine::rig::{self, RigMutant, RigSwitches};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@whoosh-check.invalid";
/// The password sync-imap's FakeImap accepts.
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(60);

/// One test at a time, and each one fenced (see `Fence`).
fn serial() -> (Fenced, std::sync::MutexGuard<'static, ()>) {
    static LOCK: Mutex<()> = Mutex::new(());
    fence();
    (Fenced, LOCK.lock().unwrap_or_else(|e| e.into_inner()))
}

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

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn rig_core(dir: &Path) -> RustCore { RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens") }

fn state(core: &RustCore, path: &str) -> Option<String> { core.state(path.to_string()) }

fn settle(core: &RustCore, provider: &str) -> String {
    let path = format!("ai_provider/{provider}/state");
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = state(core, &path).unwrap_or_default();
        if s != "\"checking\"" || Instant::now() > until { return s; }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn save_key(core: &RustCore, provider: &str, key: &str) {
    core.run(Command::SaveProviderKey { provider_id: provider.into(), key: SecretText { value: key.into() } }).expect("the command answers");
}

/// checks/realcore_aifake.py `--serve`, the check's own fake: its port, and the child (stopped when the test drops it).
struct AiFake { child: Child, port: u16 }
impl AiFake {
    fn start() -> AiFake {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../checks/realcore_aifake.py");
        let mut child = Process::new("python3").arg("-I").arg(&script).arg("--serve").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
            .spawn().expect("python3 runs the AI fake");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        let port = line.trim().strip_prefix("port ").and_then(|p| p.parse().ok()).unwrap_or_else(|| panic!("the fake said {line:?}"));
        AiFake { child, port }
    }
}
impl Drop for AiFake {
    fn drop(&mut self) { drop(self.child.stdin.take()); let _ = self.child.wait(); }
}

/// Sets process variables for one rig launch and clears them after (the file runs one test at a time).
struct Env(Vec<&'static str>);
impl Env {
    fn set(pairs: &[(&'static str, String)]) -> Env {
        for (k, v) in pairs { std::env::set_var(k, v); }
        Env(pairs.iter().map(|(k, _)| *k).collect())
    }
}
impl Drop for Env {
    fn drop(&mut self) { for k in &self.0 { std::env::remove_var(k); } }
}

// ---------------------------------------------------------------- the switches

#[test]
fn the_rig_variables_are_read_on_a_rig_copy_only() {
    let _one = serial();
    let dir = Path::new("/run/folder/core");
    let all = |k: &str| -> Option<String> { match k { "WHOOSH_CORE_CHECK_PROVIDERS" => Some("40123".into()), "WHOOSH_CORE_MUTANT" => Some("dial-out".into()), "WHOOSH_CORE_MUTANT_DIAL_PORT" => Some("40999".into()), _ => None } };
    assert_eq!(rig::from_env(false, dir, &all), RigSwitches::default(), "the owner's copy reads nothing, whatever the variables say");
    assert_eq!(rig::from_env(true, dir, &all), RigSwitches { rig: true, providers_port: Some(40123), dial_port: Some(40999), mutant: Some(RigMutant::DialOut), keys_file: Some(dir.join("rig-keys.json")) });
    for m in RigMutant::ALL {
        let named = |k: &str| (k == "WHOOSH_CORE_MUTANT").then(|| m.name().to_string());
        assert_eq!(rig::from_env(true, dir, &named).mutant, Some(m), "{}", m.name());
    }
    let odd = |k: &str| -> Option<String> { match k { "WHOOSH_CORE_CHECK_PROVIDERS" => Some("not a port".into()), "WHOOSH_CORE_MUTANT" => Some("sync-never-stores".into()), _ => None } };
    let s = rig::from_env(true, dir, &odd);
    assert_eq!((s.providers_port, s.mutant), (None, None), "a bad port is none; an older mutant name stays rust_core.rs's");
}

#[test]
fn only_the_key_providers_hosts_are_routed_to_loopback() {
    let _one = serial();
    assert_eq!(loopback_url("https://api.anthropic.com/v1/models?after_id=x", 7).as_deref(), Some("http://127.0.0.1:7/v1/models?after_id=x"));
    assert_eq!(loopback_url("https://api.openai.com/v1/responses", 7).as_deref(), Some("http://127.0.0.1:7/v1/responses"));
    assert!(loopback_url("https://generativelanguage.googleapis.com/v1beta/models", 7).is_some() && loopback_url("https://openrouter.ai/api/v1/models", 7).is_some());
    assert_eq!(loopback_url("https://example.org/v1/models", 7), None, "any other host keeps its address");
    assert_eq!(loopback_url("http://localhost:1234/v1/models", 7), None, "a local server keeps its address");
}

// ---------------------------------------------------------------- R2 + R5: the product's own path against the check's fake

#[test]
fn check_providers_runs_the_key_the_model_list_the_writer_and_a_completion_and_a_relaunch_keeps_them() {
    let _one = serial();
    let fake = AiFake::start();
    let dir = tempfile::tempdir().unwrap();
    let _env = Env::set(&[("WHOOSH_CORE_CHECK_PROVIDERS", fake.port.to_string())]);
    let core = rig_core(dir.path());
    // checked before any key is pasted: a mutant that skips the rig wiring stops here, before a request could reach a real provider
    assert!(rig::is_rig_process(), "the rig core marked the process");
    save_key(&core, "anthropic", "sk-rig-bad");
    assert_eq!(settle(&core, "anthropic"), "\"refused\"");
    assert_eq!(state(&core, "ai_provider/anthropic/refusal").as_deref(), Some("\"invalid x-api-key\""), "the provider's own words");
    assert_eq!(state(&core, "provider_credential/anthropic/stored").as_deref(), Some("false"));
    assert!(!dir.path().join("rig-keys.json").exists(), "a refused key reaches no keyring");
    save_key(&core, "anthropic", "sk-rig-good");
    assert_eq!(settle(&core, "anthropic"), "\"connected\"");
    assert_eq!(state(&core, "ai_provider/anthropic/models").as_deref(), Some("[\"model-a\",\"model-b\"]"));
    save_key(&core, "openai", "sk-rig-good");
    assert_eq!(settle(&core, "openai"), "\"connected\"", "OpenAI's shape too");
    let mut writer = core.agent_configs().into_iter().find(|a| a.id == "writer").unwrap();
    writer.model = "model-b".into();
    core.run(Command::SaveAgentConfig { config: writer }).unwrap();
    assert_eq!(state(&core, "agent_config/writer/model").as_deref(), Some("\"model-b\""));
    assert_eq!(state(&core, "agent_config/writer/provider").as_deref(), Some("\"anthropic\""));
    let m = core.model_for(AiJob::Draft).expect("the Writer has a model");
    assert_eq!(m.complete(None, "Reply to this".into(), 50).unwrap(), "A reply from model-b.", "the draft's completion used the chosen model");
    drop(core);
    // the relaunch: the same copy on the same folder (R5)
    let again = rig_core(dir.path());
    assert_eq!(state(&again, "agent_config/writer/model").as_deref(), Some("\"model-b\""), "the choice is in the store");
    assert_eq!(state(&again, "provider_credential/anthropic/stored").as_deref(), Some("true"));
    let m = again.model_for(AiJob::Draft).expect("still a model after the relaunch");
    assert_eq!(m.complete(None, "Again".into(), 50).unwrap(), "A reply from model-b.", "the key pasted before the relaunch is read from the rig keyring file");
    let file = std::fs::read_to_string(dir.path().join("rig-keys.json")).unwrap();
    assert!(!file.contains("sk-rig-good"), "the file keeps hex, never the key text");
}

// ---------------------------------------------------------------- the mutants

#[test]
fn no_account_stored_answers_ok_and_stores_nothing() {
    let _one = serial();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 2);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let add = |armed: bool| {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
        let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector, TestTransport::PlainLoopback);
        if armed { rig::arm(core.store().path(), Some(RigMutant::NoAccountStored)); }
        let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } });
        let n = core.store().account_summaries().unwrap().len();
        rig::arm(core.store().path(), None);
        (out.map(|o| o.created_id), n)
    };
    let (out, n) = add(false);
    assert!(out.unwrap().is_some() && n == 1, "the fake signs the account in and the core stores it");
    let (out, n) = add(true);
    assert_eq!((out.unwrap(), n), (None, 0), "armed: Ok, and no account row");
}

struct FakeSource { calls: AtomicUsize }
#[async_trait]
impl BodySource for FakeSource {
    async fn fetch_bodies(&self, _folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(uids.iter().map(|u| (*u, format!("From: s <s@example.test>\r\nTo: {USER}\r\nSubject: m {u}\r\nMessage-ID: <m{u}@example.test>\r\nMIME-Version: 1.0\r\n\
Content-Type: text/plain; charset=utf-8\r\n\r\nUn cadeau pour toi, message {u}.\r\n").into_bytes())).collect())
    }
}

fn row(account: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:INBOX:1:{uid}"), thread_id: format!("{account}:<{uid}@thread>"), account_id: account.into(), sender_name: "s".into(), from_address: "s@example.test".into(),
            to: vec![USER.into()], subject: format!("m {uid}"), preview: format!("m {uid}"), received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(400), tab_id: None,
    }
}

/// A rig core with one account and three INBOX messages stored without their body, the fake body source registered, `mutant` armed.
fn with_mail(dir: &Path, mutant: Option<RigMutant>) -> (RustCore, String, Arc<FakeSource>) {
    let core = rig_core(dir);
    rig::arm(core.store().path(), mutant);
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages((1..=3).map(|u| row(&account, u)).collect()).unwrap();
    let source = Arc::new(FakeSource { calls: AtomicUsize::new(0) });
    core.set_body_source(&account, source.clone());
    (core, account, source)
}

fn kept(core: &RustCore, account: &str) -> usize {
    (1..=3).filter(|u| core.store().body_parts(&format!("{account}:INBOX:1:{u}")).unwrap().unwrap().body_text.is_some()).count()
}

#[test]
fn bodies_never_stored_fetches_and_keeps_nothing() {
    let _one = serial();
    for (mutant, want) in [(None, 3), (Some(RigMutant::BodiesNeverStored), 0)] {
        let dir = tempfile::tempdir().unwrap();
        let (core, account, source) = with_mail(dir.path(), mutant);
        core.prefetch_bodies(&account, "INBOX");
        wait_until("the server was asked", || source.calls.load(Ordering::SeqCst) >= 1);
        assert!(core.settle_bodies(WAIT));
        assert_eq!(kept(&core, &account), want, "{mutant:?}");
        rig::arm(core.store().path(), None);
    }
}

#[test]
fn index_never_writes_leaves_the_word_index_empty() {
    let _one = serial();
    for (mutant, found) in [(None, true), (Some(RigMutant::IndexNeverWrites), false)] {
        let dir = tempfile::tempdir().unwrap();
        let (core, account, _source) = with_mail(dir.path(), mutant);
        let bg = core.background();
        bg.crawl_kick(&account);
        wait_until("every body is stored", || kept(&core, &account) == 3);
        assert!(bg.settle_crawl(WAIT) && bg.settle_index(WAIT));
        // the word index's reader reloads after its commit: the found case waits for it; the empty case reads after the same settle
        if found { wait_until("the word is found", || !bg.search_words("cadeau", 5).is_empty()); }
        else { let until = Instant::now() + Duration::from_secs(3); while Instant::now() < until && bg.search_words("cadeau", 5).is_empty() { std::thread::sleep(Duration::from_millis(50)); } }
        assert_eq!(!bg.search_words("cadeau", 5).is_empty(), found, "{mutant:?}: a word only in the bodies");
        rig::arm(core.store().path(), None);
    }
}

fn new_agent(name: &str) -> AgentConfig {
    AgentConfig { id: String::new(), name: name.into(), purpose: String::new(), provider_id: "anthropic".into(), model: "model-a".into(), instructions: String::new(), allowed: vec![AgentPermission::ReadMail], built_in: false }
}

#[test]
fn choices_in_memory_keeps_the_agent_out_of_the_store() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = rig_core(dir.path());
    rig::arm(core.store().path(), Some(RigMutant::ChoicesInMemory));
    let id = core.run(Command::SaveAgentConfig { config: new_agent("Desk") }).unwrap().created_id.unwrap();
    assert!(core.agent_configs().iter().any(|a| a.id == id), "the session still shows it");
    assert_eq!(state(&core, "agent_config/Desk/model").as_deref(), Some("\"model-a\""));
    assert!(core.store().agent_config_rows().unwrap().iter().all(|a| a.id != id), "the store never got it: a relaunch loses it");
    rig::arm(core.store().path(), None);
    let id2 = core.run(Command::SaveAgentConfig { config: new_agent("Desk two") }).unwrap().created_id.unwrap();
    assert!(core.store().agent_config_rows().unwrap().iter().any(|a| a.id == id2), "disarmed, the store keeps it");
}

#[test]
fn slow_save_makes_save_agent_config_take_80_ms() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = rig_core(dir.path());
    let timed = |name: &str| { let t0 = Instant::now(); core.run(Command::SaveAgentConfig { config: new_agent(name) }).unwrap(); t0.elapsed() };
    let quick = timed("Quick");
    rig::arm(core.store().path(), Some(RigMutant::SlowSave));
    let slow = timed("Slow");
    rig::arm(core.store().path(), None);
    assert!(slow >= rig::SLOW_SAVE, "armed: {slow:?}");
    assert!(quick < rig::SLOW_SAVE, "disarmed: {quick:?} (a timing bound: under a heavy load this alone may fail)");
}

/// dial-out through a rig launch: WHOOSH_CORE_MUTANT=dial-out with WHOOSH_CORE_MUTANT_DIAL_PORT dials 127.0.0.1 on that port (the check's own
/// listener, outside the walk's fakes); a peer that is not this Mac (0.0.0.0 here) is refused before any socket opens.
#[test]
fn dial_out_dials_this_mac_on_the_checks_port_only() {
    let _one = serial();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let before = rig::dial_attempts();
    let dir = tempfile::tempdir().unwrap();
    let _env = Env::set(&[("WHOOSH_CORE_MUTANT", "dial-out".into()), ("WHOOSH_CORE_MUTANT_DIAL_PORT", listener.local_addr().unwrap().port().to_string())]);
    let core = rig_core(dir.path());
    wait_until("the rig core dialled the check's port", || listener.accept().is_ok());
    assert!(rig::dial_attempts() > before);
    rig::arm(core.store().path(), None);
    let attempts = rig::dial_attempts();
    // 0.0.0.0: not loopback, so refused; and if a mutant removed the refusal, a connection to it never leaves this Mac
    let refused = rig::dial_out_to("0.0.0.0:9".parse().unwrap());
    assert!(refused.is_err(), "{refused:?}");
    std::thread::sleep(Duration::from_millis(300));
    assert!(rig::dial_attempts() <= attempts + 1, "no new dial thread for a peer past this Mac (the running one may count once more)");
}

/// No path of a rig copy reaches past this Mac through the providers' transport: with no CHECK_PROVIDERS port, a pasted key is refused in
/// words before any request (the guard in ai_settings.rs `ProviderHttps::route`); nothing is kept. The provider is pointed at `0.0.0.0` so that
/// a mutant removing the guard still sends nothing past this Mac (with the table's real host it would ask the real provider).
#[test]
fn a_rig_core_refuses_a_provider_host_past_this_mac() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = rig_core(dir.path());
    assert!(rig::is_rig_process());
    engine::ai_settings::set_provider_address(core.store().path(), "anthropic", Some("https://0.0.0.0:9/v1".into()));
    save_key(&core, "anthropic", "sk-rig-good");
    assert_eq!(settle(&core, "anthropic"), "\"refused\"");
    let words = state(&core, "ai_provider/anthropic/refusal").unwrap_or_default();
    assert!(words.contains("0.0.0.0 is not this Mac"), "{words}");
    assert_eq!(state(&core, "provider_credential/anthropic/stored").as_deref(), Some("false"));
}

// ---------------------------------------------------------------- round 3: one guard on every outgoing path of a rig copy
//
// The hosts are `0.0.0.0` on purpose: past the guard it is "not this Mac" (not loopback), and if a mutant removes a guard the attempt
// still never leaves the Mac (the system answers a connection to it locally). A real name would cost a DNS query past this Mac.

const PAST: &str = "0.0.0.0";

/// `rig/refused-outbound` as (count, last host).
fn refused(core: &RustCore) -> (u64, Option<String>) {
    let v: serde_json::Value = serde_json::from_str(&state(core, "rig/refused-outbound").expect("served on a rig")).unwrap();
    (v["count"].as_u64().unwrap(), v["last_host"].as_str().map(String::from))
}

fn config_at(imap: &str, imap_port: u16, smtp: &str) -> AccountConfig {
    AccountConfig { imap_host: imap.into(), smtp_host: smtp.into(), ..config(imap_port, 9) }
}

/// A rig core over `dir` whose store already holds one password account at `cfg` (its password in the rig keyring file), opened after.
fn rig_with_account(dir: &Path, cfg: AccountConfig) -> (RustCore, String) {
    let keys = secrets::RigFileKeychain::open(Namespace::rig(), dir.join("rig-keys.json")).unwrap();
    let r = secrets::StoreSecret::execute(&keys, secrets::SecretKind::AccountCredential, USER, &secrets::Secret::new(PW)).unwrap();
    drop(keys);
    let store = Store::open(&dir.join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, &format!("{}\u{1f}{}", r.service, r.account), false, cfg, 1).unwrap();
    drop(store);
    (rig_core(dir), id)
}

/// The sign-in check of a rig copy with its IMAP host past this Mac: refused in words, counted. Since round 5 the guard is engine-act-r6's
/// (crate netguard, in the dialers); this file keeps the case as a check of the path, the mutants are that lane's.
#[test]
fn on_a_rig_the_sign_in_check_past_this_mac_is_refused_and_counted() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = rig_core(dir.path());
    for (imap, smtp) in [(PAST, "127.0.0.1")] {
        let (before, _) = refused(&core);
        let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: None, config: config_at(imap, 1, smtp), password: SecretText { value: PW.into() } });
        let words = format!("{out:?}");
        assert!(words.contains("0.0.0.0 is not this Mac"), "imap {imap}, smtp {smtp}: {words}");
        assert_eq!(refused(&core), (before + 1, Some(PAST.into())), "imap {imap}, smtp {smtp}");
    }
    assert!(core.store().account_summaries().unwrap().is_empty());
}

/// A stored IMAP account past this Mac: the rig copy's sync never dials it; its sync state says why. Mutant G3 (the sync's guard line removed).
#[test]
fn on_a_rig_a_stored_imap_account_past_this_mac_is_refused() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let before = netguard::refused_count();
    let (core, id) = rig_with_account(dir.path(), config_at(PAST, 993, "127.0.0.1"));
    wait_until("the sync says why", || core.sync_states().iter().any(|s| s.account_id == id && s.last_error.as_ref().is_some_and(|f| f.reason_words.contains("0.0.0.0 is not this Mac"))));
    let (after, last) = refused(&core);
    assert!(after > before && last.as_deref() == Some(PAST));
}

/// A send of a rig copy to an SMTP host past this Mac is refused before any connection. Mutant G4 (the send's guard line removed).
#[test]
fn on_a_rig_a_send_past_this_mac_is_refused() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    // IMAP on this Mac's own closed port 1 (the sync fails locally); SMTP past this Mac
    let (core, account) = rig_with_account(dir.path(), config_at("127.0.0.1", 1, PAST));
    core.store().insert_messages(vec![row(&account, 1)]).unwrap();
    let (before, _) = refused(&core);
    let draft = core.run(Command::SaveDraft { from_account_id: Some(account.clone()), to: vec!["x@example.test".into()], cc: vec![], subject: "Hi".into(), body: "words".into(), reply_to_thread_id: None })
        .expect("saved").created_id.unwrap();
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    let _ = core.run(Command::SendNow { outbox_id: entry });
    wait_until("the send was refused by the guard", || refused(&core).0 > before);
    assert_eq!(refused(&core).1.as_deref(), Some(PAST));
}

/// The OAuth authority of a rig copy. Mutant G5 (the authority's guard line removed).
#[test]
fn on_a_rig_the_sign_in_authority_past_this_mac_is_refused() {
    let _one = serial();
    let core = rig_core(tempfile::tempdir().unwrap().path());
    let (before, _) = refused(&core);
    let e = engine::sign_in::authority::AuthorityHttp::new().get_status("https://0.0.0.0:9/common/v2.0/.well-known/openid-configuration").unwrap_err();
    assert!(e.to_string().contains("0.0.0.0 is not this Mac"), "{e}");
    assert_eq!(refused(&core), (before + 1, Some(PAST.into())));
}

/// One-click unsubscribe of a rig copy. Mutant G6 (the unsubscribe's guard line removed).
#[test]
fn on_a_rig_a_one_click_unsubscribe_past_this_mac_is_refused() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = rig_core(dir.path());
    let account = core.store().add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let mut m = row(&account, 1);
    m.message.list_unsubscribe = vec!["https://0.0.0.0:9/unsubscribe".into()];
    m.message.list_unsubscribe_one_click = true;
    m.has_list_header = true;
    core.store().insert_messages(vec![m]).unwrap();
    let (before, _) = refused(&core);
    let out = core.run(Command::Unsubscribe { message_id: format!("{account}:INBOX:1:1") });
    assert!(format!("{out:?}").contains("0.0.0.0 is not this Mac"), "{out:?}");
    assert_eq!(refused(&core), (before + 1, Some(PAST.into())));
}

/// Provider detection of a rig copy looks nothing up past this Mac (an address outside the check's domain): netguard's guard (engine-act-r6's,
/// in the detector's HTTP, DNS and probe) refuses each step and counts it. No mutant of mine: the guard is that lane's.
#[test]
fn on_a_rig_detection_past_this_mac_is_refused() {
    let _one = serial();
    let core = rig_core(tempfile::tempdir().unwrap().path());
    // checked before the lookup: without the rig wiring the real detector would query past this Mac
    assert!(rig::is_rig_process(), "the rig core marked the process");
    let (before, _) = refused(&core);
    let r = core.detect_provider("owner@example.org".into());
    // netguard refuses each step's lookup (ISPDB, the domain's files, DNS, Autodiscover, the probes): the detection finds nothing, every
    // step's refusal is counted, and the fence (this test's drop) proves no name past this Mac was looked up
    assert!(matches!(r.outcome, DetectionOutcome::Refused { .. }), "{r:?}");
    assert!(refused(&core).0 > before, "counted on netguard's counter");
}

/// THE fence test (the lead's ruling, round 5): every outgoing path this file drives, on a rig copy, pointed at the providers' and a mail
/// domain's REAL names (no loopback redirect, no address override): each one is refused, no name past this Mac is ever looked up, and the
/// fence's listener sees nothing. A mutant that removes a guard fails here, and still reaches this Mac only (the fence answers every name).
#[test]
fn no_path_of_this_file_looks_up_a_name_past_this_mac() {
    let _one = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = rig_core(dir.path());
    assert!(rig::is_rig_process(), "the rig core closed the process");
    let (before, _) = refused(&core);
    for provider in ["anthropic", "openai", "gemini", "openrouter"] {
        save_key(&core, provider, "sk-rig-good");
        assert_eq!(settle(&core, provider), "\"refused\"", "{provider}");
        assert!(state(&core, &format!("ai_provider/{provider}/refusal")).unwrap_or_default().contains("is not this Mac"), "{provider}");
    }
    let r = core.detect_provider("owner@example.org".into());
    assert!(matches!(r.outcome, DetectionOutcome::Refused { .. }), "{r:?}");
    assert!(refused(&core).0 >= before + 5, "every attempt counted on netguard's counter");
    let (names, reached) = fence_breaches();
    assert!(names.is_empty() && !reached, "looked up {names:?}, reached the listener: {reached}");
}
