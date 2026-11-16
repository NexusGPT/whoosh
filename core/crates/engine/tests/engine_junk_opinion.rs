//! junk-opinion: the on-device model as the junk filter's extra signal, through the real sync over the in-process IMAP fake (127.0.0.1 only)
//! and a fake on-device `DecisionPort` (Laya's place: `RustCore::set_laya_port`). Every sync test ends with `nothing_left_this_mac` (netguard
//! closed to this Mac, a resolver stub counting every outside name, the guard's refusal counter at 0): the opinion is asked on this Mac only.
//!
//! The fixture mail is `auth3`: three failed authentication verdicts, 0.755 from the header rules, inside the suspect band [0.70, 0.90).
//!
//! BROKEN COPIES (one edit each; each must turn the named tests red):
//!   P1 junk_filter.rs `store_arrivals`: the `tokio::spawn(late_opinions(..));` call replaced by a `drop(..)` of its arguments (the opinion is never asked)
//!        -> a_suspect_arrival_is_stored_first_and_a_late_opinion_of_095_moves_it
//!   P2 junk_filter.rs `store_arrivals`: the `t.in_suspect_band(s.verdict.score)` filter of `late` removed (every arrival is asked)
//!        -> a_message_outside_the_band_never_asks
//!   P3 junk_filter.rs `LocalOpinion::new`: `(port.provider() == DecisionProviderKind::Local).then(..)` -> `Some(LocalOpinion { port })` (a hosted port is asked)
//!        -> a_hosted_provider_is_never_asked_and_changes_nothing, the_local_opinion_reads_the_yes_probability_and_nothing_else
//!   P4 junk_filter.rs `apply_late_opinion`: `let (new, decision) = lock(..).with_late_opinion(score, Some(chance));` -> `.with_late_opinion(score, None)`
//!        -> a_suspect_arrival_is_stored_first_and_a_late_opinion_of_095_moves_it
#![allow(dead_code)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::junk_filter::LocalOpinion;
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use junk::ModelOpinion;
use secrets::{MemoryKeychain, Namespace};
use store::{JunkState, Store};

const MOVE: f64 = junk::tuning::JUNK_MOVE_THRESHOLD;
const REVIEW: f64 = junk::tuning::JUNK_REVIEW_THRESHOLD;

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

/// The phish: SPF, DKIM and DMARC fail in the receiving server's own (topmost) header, the display name carries a bank's address while
/// the mail comes from another site, and Reply-To is a third site. Alone, the header rules give 1 - 0.7 * 0.7 * 0.5 * 0.45 * 0.75 = 0.917.
fn phish(n: u32) -> Vec<u8> {
    format!("Authentication-Results: mx.owner.example; spf=fail smtp.mailfrom=secure-login.example;\r\n dkim=fail header.d=secure-login.example; dmarc=fail header.from=secure-login.example\r\n\
             Authentication-Results: forged.example; spf=pass; dkim=pass; dmarc=pass\r\n\
             From: \"service@bank.example\" <alerts{n}@secure-login.example>\r\nReply-To: collect@payouts.example\r\nTo: {USER}\r\n\
             Subject: Your account is locked {n}\r\nMessage-ID: <phish-{n}@secure-login.example>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\n\
             Verify your account now or it closes today.\r\n").into_bytes()
}
/// The same phish with no Reply-To: 1 - 0.7 * 0.7 * 0.5 * 0.45 = 0.890, between the two cut-offs.
fn phish_no_reply_to(n: u32) -> Vec<u8> {
    String::from_utf8(phish(n)).unwrap().replace("Reply-To: collect@payouts.example\r\n", "").into_bytes()
}
fn plain(n: u32) -> Vec<u8> {
    format!("From: Ann Lee <ann@friends.example>\r\nTo: {USER}\r\nSubject: Lunch on Friday {n}\r\nMessage-ID: <plain-{n}@friends.example>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\nShall we meet at noon?\r\n").into_bytes()
}

/// Puts one raw message in a mailbox of the fake and wakes an IDLE on it (the fake announces a changed mailbox).
fn deliver(imap: &FakeImap, mbox: &str, raw: Vec<u8>, flags: &[&str]) -> u32 {
    let uid = imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(imap_fakes::Msg { uid, flags: flags.iter().map(|f| f.to_string()).collect(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
        uid
    });
    imap.change_flags(mbox, &[], "\\Seen");
    uid
}

struct Synced { core: RustCore, account: String, imap: FakeImap, _smtp: FakeSmtp, _rt: tokio::runtime::Runtime, store_path: std::path::PathBuf, _dir: tempfile::TempDir }

/// An account added and synced to its history over the fake, listening by IDLE. `prepare` fills the server before the account is added.
fn synced(condstore: bool, prepare: impl FnOnce(&FakeImap)) -> Synced {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| { s.qresync = condstore; s.cap_condstore = condstore; });
    prepare(&imap);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store_path = dir.path().join("whoosh.sqlite");
    let core = RustCore::with_parts(Store::open(&store_path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(engine::SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    // the fake counts an IDLE as started before it waits for a change, and drops a change announced in between: let it settle
    std::thread::sleep(Duration::from_millis(300));
    Synced { core, account, imap, _smtp: smtp, _rt: rt, store_path, _dir: dir }
}

/// A second read-only view of the core's store (its own connection; the WAL lets it read while the core writes).
fn peek(s: &Synced) -> Store { Store::open(&s.store_path).unwrap() }

/// The id of the stored message with this subject, and the folder the store holds it in.
fn id_by_subject(store: &Store, subject: &str) -> Option<(String, String)> {
    let id = store.junk_texts(&all_ids(store)).unwrap().into_iter().find(|m| m.subject == subject)?.message_id;
    let folder = store.message(&id).unwrap()?.folder;
    Some((id, folder))
}
fn all_ids(store: &Store) -> Vec<String> {
    store.write(|ctx| {
        let mut st = ctx.tx.prepare("SELECT id FROM message")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }).unwrap()
}


/// What the store holds of one arrival once it is scored: (id, folder, score, state).
fn scored(st: &Store, subject: &str) -> (String, String, f64, Option<JunkState>) {
    let mut out = None;
    wait_until(&format!("{subject:?} stored and scored"), || {
        if let Some((id, folder)) = id_by_subject(st, subject) {
            if let Some((Some(score), state)) = st.junk_of(&id).unwrap() { out = Some((id, folder, score, state)); return true; }
        }
        false
    });
    out.unwrap()
}

/// True once the server no longer holds this INBOX message and a MOVE was asked (a batch is one command: "UID MOVE 2,3 Junk").
fn moved_on_server(s: &Synced, uid: u32) -> bool {
    !s.imap.uids("INBOX").contains(&uid) && s.imap.log().iter().any(|l| l.contains("UID MOVE") && l.contains("Junk"))
}

fn auth3(n: u32) -> Vec<u8> {
    String::from_utf8(phish_no_reply_to(n)).unwrap().replace("\"service@bank.example\" ", "").into_bytes()
}

// ---------------------------------------------------------------- the fake on-device model

/// Laya's place: answers one yes/no with a fixed chance, counts its asks, and can be held until released (a slow model).
/// The same port is the label flow's model too (`Filing::port`): the shipped labels ask it a `Choice` for each new INBOX message.
/// `asks` counts the junk filter's question only (`Noul` with id "junk", junk_filter.rs `LocalOpinion::chance_junk`); the label
/// flow's question is refused, as before, and not counted: these tests judge the junk filter, not the label flow.
struct FakePort {
    kind: DecisionProviderKind,
    answer: Result<f64, ()>,
    asks: AtomicUsize,
    hold: AtomicBool,
}
impl FakePort {
    fn new(kind: DecisionProviderKind, answer: Result<f64, ()>) -> Arc<FakePort> {
        Arc::new(FakePort { kind, answer, asks: AtomicUsize::new(0), hold: AtomicBool::new(false) })
    }
    fn local(p: f64) -> Arc<FakePort> { FakePort::new(DecisionProviderKind::Local, Ok(p)) }
    fn asks(&self) -> usize { self.asks.load(Ordering::SeqCst) }
}
impl DecisionPort for FakePort {
    fn provider(&self) -> DecisionProviderKind { self.kind }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let junk_question = matches!(questions.first(), Some(DecisionQuestion::Noul { id, .. }) if id == "junk");
        if junk_question { self.asks.fetch_add(1, Ordering::SeqCst); }
        let t0 = Instant::now();
        while self.hold.load(Ordering::SeqCst) && t0.elapsed() < Duration::from_secs(20) { std::thread::sleep(Duration::from_millis(20)); }
        let Some(DecisionQuestion::Noul { id, .. }) = questions.first().filter(|_| junk_question) else {
            return Err(CoreError::Unexpected { reason_words: "this test's model answers only the junk filter's yes/no".into() });
        };
        match self.answer {
            Ok(p) => Ok(DecisionResult { answers: vec![DecisionAnswer::Noul { id: id.clone(), yes: p > 0.5, probability: p }], answered_by: self.kind, checkpoint: None }),
            Err(()) => Err(CoreError::Unexpected { reason_words: "the weights are not on this Mac".into() }),
        }
    }
}

fn junk_message() -> junk::Message {
    junk::Message { from_name: None, from_email: "a@b.example".into(), reply_to: vec![], subject: "s".into(), text: "t".into(), html: None, authentication_results: vec![] }
}

fn settle() { std::thread::sleep(Duration::from_millis(700)); }

// ---------------------------------------------------------------- the opinion reader

#[test]
fn the_local_opinion_reads_the_yes_probability_and_nothing_else() {
    close_to_this_mac();
    let ask = |port: Arc<FakePort>| LocalOpinion::new(port).expect("a model on this Mac").chance_junk(&junk_message());
    assert_eq!(ask(FakePort::local(0.95)), Some(0.95));
    assert_eq!(ask(FakePort::local(0.0)), Some(0.0));
    assert_eq!(ask(FakePort::new(DecisionProviderKind::Local, Err(()))), None, "a failed model has no opinion");
    assert_eq!(ask(FakePort::local(7.0)), None, "a number that is not a chance is no opinion");
    assert_eq!(ask(FakePort::local(f64::NAN)), None);
    let hosted = FakePort::new(DecisionProviderKind::Jev, Ok(0.99));
    assert!(LocalOpinion::new(hosted.clone()).is_none(), "a hosted provider is never taken as the on-device model");
    assert_eq!(hosted.asks(), 0, "and never asked: the text of the mail stays on this Mac");
    nothing_left_this_mac();
}

// ---------------------------------------------------------------- through the real sync

#[test]
fn a_suspect_arrival_is_stored_first_and_a_late_opinion_of_095_moves_it() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    let port = FakePort::local(0.95);
    port.hold.store(true, Ordering::SeqCst);
    s.core.set_laya_port(Some(port.clone()));
    let uid = deliver(&s.imap, "INBOX", auth3(41), &[]);
    // the model is busy (held): the message is already stored, scored and suspected, in the inbox
    wait_until("the model is asked", || port.asks() == 1);
    let (id, folder, score, state) = scored(&st, "Your account is locked 41");
    assert_eq!((folder.as_str(), state), ("INBOX", Some(JunkState::Suspect)), "stored and suspected while the model is still thinking");
    assert!(score >= REVIEW && score < MOVE, "{score}");
    port.hold.store(false, Ordering::SeqCst);
    wait_until("the late opinion moves it", || st.junk_of(&id).unwrap().map(|(_, state)| state) == Some(Some(JunkState::Moved)));
    assert_eq!(st.message(&id).unwrap().unwrap().folder, "Spam");
    let (late, _) = st.junk_of(&id).unwrap().unwrap();
    assert!(late.unwrap() >= MOVE && late.unwrap() > score, "re-scored: {late:?} against {score}");
    wait_until("moved on the server", || moved_on_server(&s, uid));
    assert_eq!(port.asks(), 1, "one question for one suspect message");
    nothing_left_this_mac();
}

#[test]
fn an_opinion_of_one_half_leaves_the_suspect_arrival_suspected() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    let port = FakePort::local(0.5);
    s.core.set_laya_port(Some(port.clone()));
    let uid = deliver(&s.imap, "INBOX", auth3(42), &[]);
    wait_until("the model is asked", || port.asks() == 1);
    settle();
    let (id, folder, score, state) = scored(&st, "Your account is locked 42");
    assert_eq!((folder.as_str(), state), ("INBOX", Some(JunkState::Suspect)));
    assert!(score < MOVE, "{score}");
    assert!(!moved_on_server(&s, uid));
    let _ = id;
    nothing_left_this_mac();
}

#[test]
fn no_model_ready_the_opinion_is_absent_and_changes_nothing() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    let uid = deliver(&s.imap, "INBOX", auth3(43), &[]);
    let (_, folder, score, state) = scored(&st, "Your account is locked 43");
    settle();
    let (_, _, again, state_again) = scored(&st, "Your account is locked 43");
    assert_eq!((folder.as_str(), state, state_again), ("INBOX", Some(JunkState::Suspect), Some(JunkState::Suspect)));
    assert_eq!(score, again, "the score is what the other signals made it");
    assert!(!moved_on_server(&s, uid));
    // a model that fails is the same as none
    let failing = FakePort::new(DecisionProviderKind::Local, Err(()));
    s.core.set_laya_port(Some(failing.clone()));
    deliver(&s.imap, "INBOX", auth3(44), &[]);
    wait_until("the failing model is asked", || failing.asks() == 1);
    settle();
    let (_, folder, _, state) = scored(&st, "Your account is locked 44");
    assert_eq!((folder.as_str(), state), ("INBOX", Some(JunkState::Suspect)), "a failed ask changes nothing");
    nothing_left_this_mac();
}

#[test]
fn a_hosted_provider_is_never_asked_and_changes_nothing() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    let hosted = FakePort::new(DecisionProviderKind::Jev, Ok(0.99));
    s.core.set_decision_port(Some(hosted.clone()));
    deliver(&s.imap, "INBOX", auth3(45), &[]);
    let (_, folder, _, state) = scored(&st, "Your account is locked 45");
    settle();
    assert_eq!((folder.as_str(), state), ("INBOX", Some(JunkState::Suspect)));
    assert_eq!(hosted.asks(), 0, "the junk filter never sends mail text to a hosted model");
    nothing_left_this_mac();
}

#[test]
fn a_message_outside_the_band_never_asks() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    let port = FakePort::local(0.95);
    s.core.set_laya_port(Some(port.clone()));
    // plain mail (score 0, below the band) and a phish whose header rules alone reach 0.917 (at or above the move cut-off)
    deliver(&s.imap, "INBOX", plain(46), &[]);
    let moved = deliver(&s.imap, "INBOX", phish(47), &[]);
    let (_, plain_folder, _, plain_state) = scored(&st, "Lunch on Friday 46");
    let (_, phish_folder, _, phish_state) = scored(&st, "Your account is locked 47");
    assert_eq!((plain_folder.as_str(), plain_state), ("INBOX", None));
    assert_eq!((phish_folder.as_str(), phish_state), ("Spam", Some(JunkState::Moved)));
    wait_until("moved on the server", || moved_on_server(&s, moved));
    settle();
    assert_eq!(port.asks(), 0, "no ask for a message below the band or already past the move cut-off");
    // inside the band: exactly one
    deliver(&s.imap, "INBOX", auth3(48), &[]);
    wait_until("one ask for the suspect message", || port.asks() == 1);
    settle();
    assert_eq!(port.asks(), 1);
    nothing_left_this_mac();
}
