//! fix-junkgate: one on-device model call at a time across the label flow and the junk filter, and the junk filter's on-device opinion
//! kept when the label flow's model is a hosted one. Through the real sync over the in-process IMAP fake (127.0.0.1 only) and model twins
//! (no real model, no provider). Every test ends with `nothing_left_this_mac` (copied from engine_junk_opinion.rs).
//!
//! The fixture mail is `auth3` (engine_junk_opinion.rs): 0.755 from the header rules, inside the suspect band, so the junk filter asks its
//! one yes/no; the same INBOX arrival is also asked about by the label flow (the shipped labels ask the model a Choice).
//!
//! BROKEN COPIES (one edit each; each must turn the named test red):
//!   G1 decision_model.rs `LayaPort::decide`: `let turn = self.gate.as_ref().map(|g| g.turn());` -> `let turn: Option<Turn> = None;`
//!        (the label flow's call takes no turn) -> a_suspect_arrival_the_label_flow_also_asks_about_never_has_two_calls_in_flight
//!   G2 junk_filter.rs `on_device_opinion`: `Some(_) => filing.label_model().on_device(),` -> `Some(_) => None,`
//!        (a hosted app port takes the junk opinion away) -> with_a_hosted_label_port_the_junk_opinion_is_asked_of_the_on_device_model
//!   G3 decision_model.rs `on_device_for_junk`: `DecisionProviderKind::Rules => None,` -> `DecisionProviderKind::Rules | DecisionProviderKind::Jev => None,`
//!        (the owner's saved Jev takes the junk opinion away) -> with_the_owners_saved_jev_the_junk_opinion_is_still_asked_on_this_mac
#![allow(dead_code)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use ai_providers::transport::{Request, Response, Transport};
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace, SecretKind};
use store::{JunkState, Store};

static NAMES_ASKED: AtomicUsize = AtomicUsize::new(0);

/// The guard's resolver stub: an address literal or `localhost` resolves as itself; any other name is counted and refused.
struct CountingResolver;
impl netguard::Resolve for CountingResolver {
    fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
        if netguard::is_this_mac(host) {
            use std::net::ToSocketAddrs;
            return Ok((host, port).to_socket_addrs()?.collect());
        }
        NAMES_ASKED.fetch_add(1, Ordering::SeqCst);
        Err(std::io::Error::new(std::io::ErrorKind::Other, "the test refuses every name outside this Mac"))
    }
}

fn close_to_this_mac() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| { netguard::set_resolver(std::sync::Arc::new(CountingResolver)); netguard::close_to_this_mac(); });
}

fn nothing_left_this_mac() {
    assert!(netguard::is_closed(), "the process was closed to this Mac");
    assert_eq!(NAMES_ASKED.load(Ordering::SeqCst), 0, "no name outside this Mac was looked up");
    assert_eq!((netguard::refused_count(), netguard::last_refused()), (0, None), "the guard refused nothing: nothing tried to leave");
}

const USER: &str = "owner@example.test";
const PW: &str = "app-password";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(20) {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within 20 s: {what}");
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// engine_junk_opinion.rs `auth3`: SPF, DKIM and DMARC fail in the receiving server's own header; no display-name or Reply-To rule.
fn auth3(n: u32) -> Vec<u8> {
    format!("Authentication-Results: mx.owner.example; spf=fail smtp.mailfrom=secure-login.example;\r\n dkim=fail header.d=secure-login.example; dmarc=fail header.from=secure-login.example\r\n\
             Authentication-Results: forged.example; spf=pass; dkim=pass; dmarc=pass\r\n\
             From: <alerts{n}@secure-login.example>\r\nTo: {USER}\r\n\
             Subject: Your account is locked {n}\r\nMessage-ID: <phish-{n}@secure-login.example>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\n\
             Verify your account now or it closes today.\r\n").into_bytes()
}

/// Puts one raw message in a mailbox of the fake and wakes an IDLE on it.
fn deliver(imap: &FakeImap, mbox: &str, raw: Vec<u8>) -> u32 {
    let uid = imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(imap_fakes::Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
        uid
    });
    imap.change_flags(mbox, &[], "\\Seen");
    uid
}

struct Synced { core: RustCore, account: String, imap: FakeImap, _smtp: FakeSmtp, _rt: tokio::runtime::Runtime, store_path: std::path::PathBuf, _dir: tempfile::TempDir }

/// An account added and synced to its history over the fake, listening by IDLE.
fn synced() -> Synced {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| { s.qresync = true; s.cap_condstore = true; });
    imap.seed("INBOX", 1);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store_path = dir.path().join("whoosh.sqlite");
    let core = RustCore::with_parts(Store::open(&store_path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(engine::SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    std::thread::sleep(Duration::from_millis(300));
    Synced { core, account, imap, _smtp: smtp, _rt: rt, store_path, _dir: dir }
}

fn peek(s: &Synced) -> Store { Store::open(&s.store_path).unwrap() }

/// The (id, folder, state) of the stored message with this subject once it is scored.
fn scored(st: &Store, subject: &str) -> (String, String, Option<JunkState>) {
    let mut out = None;
    wait_until(&format!("{subject:?} stored and scored"), || {
        let ids: Vec<String> = st.write(|ctx| {
            let mut q = ctx.tx.prepare("SELECT id FROM message")?;
            let rows = q.query_map([], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        }).unwrap();
        let Some(t) = st.junk_texts(&ids).unwrap().into_iter().find(|m| m.subject == subject) else { return false };
        let Some(m) = st.message(&t.message_id).unwrap() else { return false };
        if let Some((Some(_), state)) = st.junk_of(&t.message_id).unwrap() { out = Some((t.message_id.clone(), m.folder, state)); return true; }
        false
    });
    out.unwrap()
}

// ---------------------------------------------------------------- the model twin

/// A model twin: it records how many calls are in flight at once (`peak`), counts the junk filter's yes/no (`Noul` with id "junk") apart
/// from every other question (the label flow's), takes `delay` per call (a model that thinks), answers the junk question with `chance`
/// and refuses every other question in words.
struct Twin { kind: DecisionProviderKind, chance: f64, delay: Duration, now: AtomicUsize, peak: AtomicUsize, junk: AtomicUsize, other: AtomicUsize }
impl Twin {
    fn new(kind: DecisionProviderKind, chance: f64, delay_ms: u64) -> Arc<Twin> {
        Arc::new(Twin { kind, chance, delay: Duration::from_millis(delay_ms), now: AtomicUsize::new(0), peak: AtomicUsize::new(0), junk: AtomicUsize::new(0), other: AtomicUsize::new(0) })
    }
    fn junk(&self) -> usize { self.junk.load(Ordering::SeqCst) }
    fn other(&self) -> usize { self.other.load(Ordering::SeqCst) }
    fn peak(&self) -> usize { self.peak.load(Ordering::SeqCst) }
}
impl DecisionPort for Twin {
    fn provider(&self) -> DecisionProviderKind { self.kind }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let in_flight = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(in_flight, Ordering::SeqCst);
        let junk = matches!(questions.first(), Some(DecisionQuestion::Noul { id, .. }) if id == "junk");
        if junk { self.junk.fetch_add(1, Ordering::SeqCst); } else { self.other.fetch_add(1, Ordering::SeqCst); }
        std::thread::sleep(self.delay);
        self.now.fetch_sub(1, Ordering::SeqCst);
        if !junk { return Err(CoreError::Unexpected { reason_words: "this twin answers only the junk filter's yes/no".into() }); }
        Ok(DecisionResult { answers: vec![DecisionAnswer::Noul { id: "junk".into(), yes: self.chance > 0.5, probability: self.chance }], answered_by: self.kind, checkpoint: None })
    }
}

/// Where a Jev request would go: counted and refused (nothing is sent anywhere).
struct NoJev(AtomicUsize);
impl Transport for NoJev {
    fn send(&self, _request: &Request) -> Result<Response, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err("the test sends nothing to a hosted provider".into())
    }
}

// ---------------------------------------------------------------- one call at a time

#[test]
fn a_suspect_arrival_the_label_flow_also_asks_about_never_has_two_calls_in_flight() {
    close_to_this_mac();
    let s = synced();
    let st = peek(&s);
    // each call takes 1.5 s: two askers that did not wait for each other would overlap
    let laya = Twin::new(DecisionProviderKind::Local, 0.5, 1500);
    s.core.set_laya_port(Some(laya.clone()));
    deliver(&s.imap, "INBOX", auth3(61));
    let (_, folder, state) = scored(&st, "Your account is locked 61");
    assert_eq!((folder.as_str(), state), ("INBOX", Some(JunkState::Suspect)), "stored and suspected");
    wait_until("the junk filter and the label flow both asked", || laya.junk() == 1 && laya.other() >= 1);
    s.core.flush_filing();
    wait_until("no call left in flight", || laya.now.load(Ordering::SeqCst) == 0);
    assert_eq!(laya.junk(), 1, "one junk question for one suspect message");
    assert_eq!(laya.peak(), 1, "the on-device model answered one question at a time ({} junk, {} label flow)", laya.junk(), laya.other());
    nothing_left_this_mac();
}

// ---------------------------------------------------------------- the junk opinion stays on this Mac

#[test]
fn with_a_hosted_label_port_the_junk_opinion_is_asked_of_the_on_device_model() {
    close_to_this_mac();
    let s = synced();
    let st = peek(&s);
    let hosted = Twin::new(DecisionProviderKind::Jev, 0.99, 0);
    let laya = Twin::new(DecisionProviderKind::Local, 0.95, 0);
    s.core.set_decision_port(Some(hosted.clone()));
    s.core.set_laya_port(Some(laya.clone()));
    deliver(&s.imap, "INBOX", auth3(62));
    let (id, _, _) = scored(&st, "Your account is locked 62");
    wait_until("the on-device model is asked the junk question", || laya.junk() == 1);
    wait_until("its opinion of 0.95 moves the suspect message", || st.junk_of(&id).unwrap().map(|(_, state)| state) == Some(Some(JunkState::Moved)));
    s.core.flush_filing();
    std::thread::sleep(Duration::from_millis(700));
    assert_eq!(hosted.junk(), 0, "the junk question never goes to the hosted model");
    assert_eq!(laya.junk(), 1, "one junk question, on this Mac");
    nothing_left_this_mac();
}

#[test]
fn with_the_owners_saved_jev_the_junk_opinion_is_still_asked_on_this_mac() {
    close_to_this_mac();
    let s = synced();
    let st = peek(&s);
    // the owner chose Jev for labels (consent given, a key reference saved but no key stored: the label flow cannot reach Jev, and every
    // request Jev would make goes to a transport that counts and refuses it)
    let jev = Arc::new(NoJev(AtomicUsize::new(0)));
    s.core.set_decision_transport(jev.clone());
    s.core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given: true }).expect("consent is served");
    let key = KeychainRef { service: Namespace::rig().reference(SecretKind::ProviderCredential, "jev").unwrap().service, account: "jev".into() };
    let model = DecisionModel { provider: DecisionProviderKind::Jev, model: None, keychain: Some(key), decisions_enabled: vec!["labels".into()], question_words: Default::default(), last_run_ms: None };
    s.core.run(Command::SaveDecisionModel { model }).expect("SaveDecisionModel is served");
    let laya = Twin::new(DecisionProviderKind::Local, 0.95, 0);
    s.core.set_laya_port(Some(laya.clone()));
    deliver(&s.imap, "INBOX", auth3(63));
    let (id, _, _) = scored(&st, "Your account is locked 63");
    wait_until("the on-device model is asked the junk question", || laya.junk() == 1);
    wait_until("its opinion of 0.95 moves the suspect message", || st.junk_of(&id).unwrap().map(|(_, state)| state) == Some(Some(JunkState::Moved)));
    s.core.flush_filing();
    assert_eq!(laya.other(), 0, "the label flow did not ask Laya: the owner chose Jev for labels");
    assert_eq!(jev.0.load(Ordering::SeqCst), 0, "nothing went to Jev");
    nothing_left_this_mac();
}
