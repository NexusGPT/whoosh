//! Lane labels-r5, round 8: Jev goes out over the REAL provider transport (`ProviderHttps`, ai_settings.rs), and a hidden test copy refuses it. Every case
//! runs as a rig process (`rig::apply`), so no case can reach a host past this Mac, and the fake provider sits on 127.0.0.1.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out; runner _captures/labels-r5/run-mutants.py):
//! - W1 decision_model.rs `default_jev_transport`: not ProviderHttps (a loopback port 9 stands in) -> the_default_jev_wiring_is_the_real_https_and_a_rig_copy_refuses_it
//! - W2 decision_model.rs `JevLoopback::send`: the request body is replaced by an empty one -> through_the_real_transport_a_jev_request_carries_exactly_the_ticked_fields
//! - W3 decision_model.rs `JevLoopback::send`: the key header is dropped -> through_the_real_transport_a_jev_request_carries_exactly_the_ticked_fields
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{AiStep, RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// A stored message: `from` writes to the owner (an inbound one) unless it is the owner's own address (a reply, which lives in Sent).
fn mail(account: &str, key: &str, thread: &str, from: &str, subject: &str, at: i64) -> NewMessage {
    let outgoing = from == USER;
    let id = format!("{account}:{key}");
    NewMessage {
        message: Message {
            id, thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(),
            to: vec![if outgoing { "friend@other.example".into() } else { USER.into() }], subject: subject.into(), preview: format!("Body of {subject}"),
            received_at_ms: at, read: outgoing, archived: false, folder: if outgoing { "Sent".into() } else { "INBOX".into() }, attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: outgoing,
        },
        remote_id: Some(key.rsplit(':').next().unwrap_or("1").to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn id_of(account: &str, key: &str) -> String { format!("{account}:{key}") }



struct Rig { core: RustCore, keys: Arc<MemoryKeychain>, account: String, label: String, _dir: tempfile::TempDir }

impl Rig {
    /// A core with one account and a described label "Reply soon" (its AI card on).
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
        let label = store.label_create_full(spec("Reply soon", "mail that waits for my answer", None), 1).unwrap().id;
        let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let core = RustCore::with_parts(store, keys.clone(), detector(), TestTransport::Real);
        Rig { core, keys, account, label, _dir: dir }
    }
    /// The owner saves the decision model (Settings, Decisions).
    fn choose(&self, provider: DecisionProviderKind, keychain: Option<KeychainRef>) {
        // a hosted provider is chosen only after the owner's yes (delta 23): the helper gives it, the cases about consent do it by hand
        if provider == DecisionProviderKind::Jev { self.core.run(Command::GiveDecisionConsent { provider, given: true }).expect("consent is served"); }
        let model = DecisionModel { provider, model: None, keychain, decisions_enabled: vec!["labels".into()], question_words: Default::default(), last_run_ms: None };
        self.core.run(Command::SaveDecisionModel { model }).expect("SaveDecisionModel is served");
    }
    /// One inbound message arrives and the label flow runs on it. fix-aisort round 18: each mail has its own sender (`ann<n>@client.example`),
    /// so a request names the mail it is about; the four mails were identical, and a request was told apart only by its place in the list.
    fn arrive(&self, n: u32) -> engine::Filed {
        self.core.store().insert_messages(vec![mail(&self.account, &format!("INBOX:{n}"), &format!("t{n}"), &format!("ann{n}@client.example"), "Can you answer?", 1_000 + n as i64)]).unwrap();
        self.core.file_stored_messages(&[id_of(&self.account, &format!("INBOX:{n}"))]).unwrap().pop().expect("the message was filed or left")
    }
}

fn spec(name: &str, description: &str, words: Option<&str>) -> LabelSpec {
    let conditions = words.map(|w| vec![RuleCondition::SubjectContains { words: w.into() }]).unwrap_or_default();
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules: RuleSet { conditions, match_mode: MatchMode::Any }, pinned: false }
}

fn rig_process(dir: &std::path::Path) { engine::rig::apply(&engine::rig::RigSwitches { rig: true, ..Default::default() }, &dir.join("whoosh.sqlite")); }

/// A fake Jev on 127.0.0.1: records (path, authorization header, body) of every request and answers each question with "Reply soon" at 0.95.
struct FakeJevServer { port: u16, seen: Arc<std::sync::Mutex<Vec<(String, String, String)>>> }
impl FakeJevServer {
    fn start() -> FakeJevServer {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen: Arc<std::sync::Mutex<Vec<(String, String, String)>>> = Default::default();
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                let (head, mut body) = loop {
                    let n = s.read(&mut chunk).unwrap_or(0);
                    if n == 0 { break (String::from_utf8_lossy(&buf).to_string(), Vec::new()); }
                    buf.extend_from_slice(&chunk[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") { break (String::from_utf8_lossy(&buf[..i]).to_string(), buf[i + 4..].to_vec()); }
                };
                let length: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
                while body.len() < length { let n = s.read(&mut chunk).unwrap_or(0); if n == 0 { break; } body.extend_from_slice(&chunk[..n]); }
                let path = head.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("").to_string();
                let auth = head.lines().find(|l| l.to_ascii_lowercase().starts_with("authorization:")).unwrap_or("").to_string();
                let text = String::from_utf8_lossy(&body).to_string();
                log.lock().unwrap().push((path, auth, text.clone()));
                let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                let mut answers = serde_json::Map::new();
                for (id, q) in v["questions"].as_object().cloned().unwrap_or_default() {
                    let pick = q["criteria"].as_object().and_then(|o| o.keys().find(|k| k.as_str() == "Reply soon").cloned()).unwrap_or_default();
                    // fix-aisort round 3: a per-label yes/no question gets the yes-probability of the label it names
            let noul = ::decision::decide::statement_label(q["instructions"].as_str().unwrap_or("")).map_or(0.1, |(label, _)| ::decision::test_answers::yes_for(label, Some("Reply soon"), 0.95));
            answers.insert(id, serde_json::json!({"choice": pick, "confidence": 0.95, "noul": noul}));
                }
                let out = serde_json::json!({"answers": answers, "model": "jev-fake"}).to_string();
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", out.len(), out);
            }
        });
        FakeJevServer { port, seen }
    }
    fn calls(&self) -> usize { self.seen.lock().unwrap().len() }
    /// fix-aisort round 18: the state text of each request so far about mail `n` (its sender's line), in the order sent.
    fn about(&self, n: u32) -> Vec<String> {
        self.states().into_iter().filter(|s| s.lines().any(|l| l == format!("from: ann{n}@client.example"))).collect()
    }
    fn states(&self) -> Vec<String> {
        self.seen.lock().unwrap().iter().map(|(_, _, body)| serde_json::from_str::<serde_json::Value>(body).unwrap()["state"].as_str().unwrap_or("").to_string()).collect()
    }
}

/// fix-aisort round 18: the label flow's worker has run every job queued so far, and the background retry it starts (a mail that waited for
/// the consent is asked once the consent is given: lane fix-aisort rounds 16 and 17), so a count read next does not race it.
fn settle(rig: &Rig) {
    for _ in 0..5 {
        rig.core.flush_filing();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The owner's path: the key through `SaveProviderKey` for "jev", then the consent, then the model (Settings, Decisions).
fn owner_picks_jev(rig: &Rig, key: &str) {
    rig.core.run(Command::SaveProviderKey { provider_id: "jev".into(), key: SecretText { value: key.into() } }).expect("Jev's key is kept");
    let keychain = rig.core.ai_providers().into_iter().find(|p| p.id == "jev").and_then(|p| p.keychain).expect("the key's reference is on Jev's row");
    rig.choose(DecisionProviderKind::Jev, Some(keychain));
}

#[test]
fn through_the_real_transport_a_jev_request_carries_exactly_the_ticked_fields() {
    let dir = tempfile::tempdir().unwrap();
    rig_process(dir.path());
    let rig = Rig::new();
    let server = FakeJevServer::start();
    rig.core.set_decision_loopback(server.port);
    owner_picks_jev(&rig, "FIXTURE-jev-key-9z8y7x");
    // consent withdrawn after choosing: nothing is sent
    rig.core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: false }).unwrap();
    assert!(matches!(rig.arrive(1).ai, AiStep::Refused(_)));
    settle(&rig);
    assert_eq!(server.calls(), 0, "no consent, no request (mail 1 waits for the AI)");
    // key, consent and model: the message goes out, subject and body, no attachment text, the key as a bearer, to Jev's path
    rig.core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: true }).unwrap();
    let f = rig.arrive(2);
    assert_eq!((f.place.as_str(), &f.ai), (rig.label.as_str(), &AiStep::Asked), "{f:?}");
    settle(&rig);
    assert_eq!(server.about(2).len(), 1, "mail 2 is asked once: {:?}", server.states());
    // fix-aisort round 18: mail 1 waited for the consent; once it is given the background retry may ask it (lanes fix-aisort 16 and 17): at
    // most once, and only now, with the fields ticked now
    assert!(server.about(1).len() <= 1, "mail 1 is asked at most once: {:?}", server.states());
    for (path, auth, _) in server.seen.lock().unwrap().iter() {
        assert_eq!(path, "/v1/systemone");
        assert!(auth.ends_with("Bearer FIXTURE-jev-key-9z8y7x"), "{auth}");
    }
    for state in server.states() {
        assert!(state.contains("subject: Can you answer?") && state.contains("Body of Can you answer?"), "subject and body are ticked: {state}");
        assert!(!state.to_lowercase().contains("attachment"), "attachments are not ticked: {state}");
    }
    // the owner unticks the body on every described label: every request from now on carries the subject and no body text
    for id in [rig.label.as_str(), "needs-reply", "updates", "promotions", "social"] {
        rig.core.store().label_set_ai_reads(id.to_string(), AiReads { subject: true, body: false, rest_of_thread: false, attachments: false }, 9).unwrap();
    }
    let sent_before_untick = server.calls();
    rig.arrive(3);
    settle(&rig);
    assert_eq!(server.about(3).len(), 1, "mail 3 is asked once: {:?}", server.states());
    for state in &server.states()[sent_before_untick..] {
        assert!(state.contains("subject: Can you answer?") && !state.contains("Body of Can you answer?"), "{state}");
    }
    // the consent is withdrawn: no request
    rig.core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: false }).unwrap();
    settle(&rig);
    let before = server.calls();
    rig.arrive(4);
    settle(&rig);
    assert_eq!(server.calls(), before, "a withdrawn consent sends nothing, over the real transport too");
}

#[test]
fn the_default_jev_wiring_is_the_real_https_and_a_rig_copy_refuses_it() {
    let dir = tempfile::tempdir().unwrap();
    rig_process(dir.path());
    let rig = Rig::new();
    // a listener on this Mac that no request may reach: the rig refuses Jev's host before any connection is made
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    owner_picks_jev(&rig, "FIXTURE-jev-key-9z8y7x");
    match rig.arrive(1).ai {
        AiStep::Refused(why) => assert!(why.contains("is not this Mac") && !why.contains("not built into this version"), "{why}"),
        other => panic!("{other:?}"),
    }
    assert!(listener.accept().is_err(), "nothing connected");
}
