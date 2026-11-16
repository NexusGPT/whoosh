//! fix-jevrig (docs/lanes/fix-jevrig.md): on the hidden test copy of the REAL engine (`open_production(_, rig = true)`), Jev's key Test reaches the local stand-in.
//! With `WHOOSH_CORE_CHECK_PROVIDERS=<port>` set, Jev's host goes to http://127.0.0.1:<port> at Jev's own path (`/v1/systemone`), the Test answers "connected",
//! and a bad key reads "refused" in Jev's words. With no port set, the rig still refuses Jev's host ("not this Mac"): the loopback listener and the name-lookup
//! stub see nothing. The listener is a Jev twin written here (Jev's answer shapes, as engine_provider_key_test.rs); nothing in this file opens a socket past
//! 127.0.0.1: every name looked up goes to a stub that answers port 9 on this Mac and records the name.
//!
//! MUTANTS (each `find` occurs once in core/crates/engine/src/ai_settings.rs; the named test goes red):
//! - R1 `let jev = host_of(decision::adapters::jev::JEV_URL).eq_ignore_ascii_case(&host);` => `let jev = false;`:
//!   with_the_rig_port_the_jev_test_reaches_the_loopback_listener_and_reads_connected
//! - R2 `netguard::outbound_allowed_url(&url).map_err(|e| words_of(&e))?;` => `let _ = netguard::outbound_allowed_url(&url);`:
//!   with_no_rig_port_the_jev_test_is_refused_and_nothing_is_dialled (the name api.typesafe.ai reaches the lookup stub)
//! - R3 `format!("http://127.0.0.1:{port}{path}")` => `format!("http://127.0.0.1:{port}/")`:
//!   with_the_rig_port_the_jev_test_reaches_the_loopback_listener_and_reads_connected (the listener sees "/", not Jev's path)
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;

const GOOD: &str = "FIXTURE-jevrig-good-5d21c9";
const BAD: &str = "FIXTURE-jevrig-bad-70be13";
const JEV_PATH: &str = "/v1/systemone";

// ---------------------------------------------------------------- the lookup stub: every name is this Mac's port 9, and is recorded

struct Fence { names: Mutex<Vec<String>> }
impl netguard::Resolve for Fence {
    fn lookup(&self, host: &str, _port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).push(host.to_string());
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
fn names() -> Vec<String> { fence().names.lock().unwrap_or_else(|e| e.into_inner()).clone() }

/// The tests of this file run one at a time: each sets process variables, as a rig launch does.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    let g = ONE.lock().unwrap_or_else(|e| e.into_inner());
    fence();
    g
}

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

// ---------------------------------------------------------------- the Jev twin on 127.0.0.1

#[derive(Debug, Clone)]
struct Seen { path: String, authorization: String }

fn answer(mut s: TcpStream, seen: Arc<Mutex<Vec<Seen>>>) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    if r.read_line(&mut first).is_err() { return; }
    let (mut len, mut authorization) = (0usize, String::new());
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).is_err() || line.trim().is_empty() { break; }
        if let Some((k, v)) = line.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().unwrap_or(0),
                "authorization" => authorization = v.trim().to_string(),
                _ => {}
            }
        }
    }
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let path = first.split_whitespace().nth(1).unwrap_or("").to_string();
    seen.lock().unwrap().push(Seen { path: path.clone(), authorization: authorization.clone() });
    let key = authorization.strip_prefix("Bearer ").unwrap_or("");
    let (status, text) = match (key, path.as_str()) {
        (GOOD, JEV_PATH) => (200, r#"{"model":"jev-latest","answers":{"which_tab":{"choice":"Updates","confidence":0.91}}}"#.to_string()),
        (_, JEV_PATH) => (401, format!(r#"{{"error":{{"message":"Invalid API key: {key}"}}}}"#)),
        _ => (404, r#"{"detail":"no such route"}"#.to_string()),
    };
    let _ = write!(s, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
}

fn jev_twin() -> (u16, Arc<Mutex<Vec<Seen>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || for s in l.incoming().flatten() { let s2 = s2.clone(); std::thread::spawn(move || answer(s, s2)); });
    (port, seen)
}

// ---------------------------------------------------------------- the rig core

fn rig_core(dir: &Path) -> RustCore { RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens") }

fn state(core: &RustCore, path: &str) -> Option<String> { core.state(path.to_string()) }

fn save_and_test(core: &RustCore, key: &str) {
    core.run(Command::SaveProviderKey { provider_id: "jev".into(), key: SecretText { value: key.into() } }).expect("Jev's key is kept");
    core.run(Command::TestProviderKey { provider_id: "jev".into() }).expect("the Test is served");
}

fn settle(core: &RustCore) -> String {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        let s = state(core, "ai_provider/jev/state").unwrap_or_default();
        if s != "\"checking\"" || Instant::now() > until { return s; }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// With the rig port set: the Test's one request reaches the loopback listener at Jev's path with the kept key, the state reads "connected"; a bad key reads
/// "refused" in Jev's words (with the key taken out of them).
#[test]
fn with_the_rig_port_the_jev_test_reaches_the_loopback_listener_and_reads_connected() {
    let _one = serial();
    let (port, seen) = jev_twin();
    let dir = tempfile::tempdir().unwrap();
    let before = names().len();
    let env = Env::set(&[("WHOOSH_CORE_CHECK_PROVIDERS", port.to_string())]);
    let core = rig_core(dir.path());
    drop(env);
    assert!(engine::rig::is_rig_process(), "the rig core marked the process");
    save_and_test(&core, GOOD);
    assert_eq!(settle(&core), "\"connected\"");
    let got = seen.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "one fixed sample, not {}: {got:?}", got.len());
    assert_eq!(got[0].path, JEV_PATH, "Jev's own path");
    assert_eq!(got[0].authorization, format!("Bearer {GOOD}"));
    assert_eq!(state(&core, "ai_provider/jev/refusal"), None);
    // a bad key: refused, in Jev's words
    save_and_test(&core, BAD);
    assert_eq!(settle(&core), "\"refused\"");
    assert_eq!(state(&core, "ai_provider/jev/refusal").as_deref(), Some("\"Invalid API key: <key>\""));
    assert_eq!(seen.lock().unwrap().len(), 2);
    assert_eq!(names().len(), before, "a literal 127.0.0.1 needs no name lookup: {:?}", names());
    println!("PASS with_the_rig_port_the_jev_test_reaches_the_loopback_listener_and_reads_connected");
}

/// With no rig port: the rig still refuses Jev's host in words ("not this Mac"); the loopback listener hears nothing and no name past this Mac is looked up.
#[test]
fn with_no_rig_port_the_jev_test_is_refused_and_nothing_is_dialled() {
    let _one = serial();
    let (_port, seen) = jev_twin();
    let dir = tempfile::tempdir().unwrap();
    let before = names().len();
    std::env::remove_var("WHOOSH_CORE_CHECK_PROVIDERS");
    let core = rig_core(dir.path());
    assert!(engine::rig::is_rig_process(), "the rig core marked the process");
    save_and_test(&core, GOOD);
    assert_eq!(settle(&core), "\"refused\"");
    let words = state(&core, "ai_provider/jev/refusal").unwrap_or_default();
    assert!(words.contains("is not this Mac"), "the guard's words: {words}");
    std::thread::sleep(Duration::from_millis(200));
    assert!(seen.lock().unwrap().is_empty(), "the listener heard {:?}", seen.lock().unwrap());
    let new_names = names().split_off(before);
    assert!(new_names.is_empty(), "a name was looked up: {new_names:?}");
    assert!(!words.contains(GOOD));
    println!("PASS with_no_rig_port_the_jev_test_is_refused_and_nothing_is_dialled");
}
