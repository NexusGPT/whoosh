//! Lane fix-aisort round 17 (the owner's Needs Reply of 12 Nov: "5 accepted · 197 waiting for the AI", then nothing moved until a Save).
//! A label with no step 1 (the built-in Needs Reply as seeded) counted no mail as waiting: `label/<id>/flow/counts` `waiting_for_ai` counted
//! only what step 1 picked, so a mail whose judgement failed at arrival was neither counted nor asked again by the background retry
//! (`label_ai_failure::retry_ids`), with Apple's judge as with Laya. Now such a mail is marked as waiting (`Store::ai_waiting_put`,
//! `decided_model` 'ai-waiting', no "no"), counted, and asked again by the same retry as on other labels, AI_RETRY_MAX at a time. A kept
//! "no" and a mail set aside never wait.
//!
//! The judge is a fake handed over as the shell hands its model (`RustCore::set_laya_port`); real store, real engine, no network.
//!
//! Run: cargo test -p engine --test engine_waiting_no_step_one -- --test-threads=1
//!
//! The card (`flow/counts` and `flow/ai_set_aside`) before and after, as these tests read it:
//! - all judged (one yes, one no): sorted_by_ai 1, waiting_for_ai 0, set aside 0, on both.
//! - some failed (two mails, the judge busy): waiting_for_ai 0 before (the mails invisible, never asked again), 2 after; once the judge
//!   answers again they are judged in the background and waiting_for_ai is 0.
//! - some set aside (one refused for itself, one failed): set aside 1 on both; waiting_for_ai 0 before, 1 after (the failed one only).
//!
//! On the code before round 17 these fail: `some_failed_mails_wait_and_are_asked_again_in_the_background`,
//! `a_set_aside_mail_does_not_wait_and_a_failed_one_does`, `sixty_failed_mails_are_asked_again_fifty_at_a_time_until_none_waits`. The control
//! `when_every_mail_is_judged_nothing_waits` passes on both.
//!
//! Broken copies (each one edit; each must turn its test red):
//! - W1 engine/src/label_flow.rs `waiting_candidates`: `if step_one_chains(store, l).is_empty() {` -> `if false {` -> red:
//!   `some_failed_mails_wait_and_are_asked_again_in_the_background`.
//! - W2 engine/src/file_arrivals.rs `run_with_book`: `.filter(|f| matches!(f.ai, AiStep::Refused(_) | AiStep::NoProvider))` ->
//!   `.filter(|f| matches!(f.ai, AiStep::NoProvider))` -> red: `some_failed_mails_wait_and_are_asked_again_in_the_background`.
//! - W3 store/src/label_meta.rs `ai_no_messages`: `AND decided_model IS NOT ?2 ORDER BY message_id` -> `AND ?2 IS NOT NULL ORDER BY message_id`
//!   (the mark read as a "no") -> red: `some_failed_mails_wait_and_are_asked_again_in_the_background`.
//! - W4 engine/src/file_arrivals.rs `run_with_book`: `store.ai_waiting_clear(judged)` -> `store.ai_waiting_clear(vec![])` -> red:
//!   `sixty_failed_mails_are_asked_again_fifty_at_a_time_until_none_waits` (judged mails stay marked).
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use contracts::*;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const LABEL: &str = "needs-reply";

#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

fn detector() -> Box<dyn engine::Detect> {
    Box::new(autoconfig::Detector::new(detect_fakes::FakeHttp::default(), detect_fakes::FakeDns::default(), detect_fakes::FakeProbe::default()))
}
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// An inbound, unread, unanswered INBOX mail.
fn mail(account: &str, key: &str, subject: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:{key}"), thread_id: format!("t-{key}"), account_id: account.into(), sender_name: "Pat".into(), from_address: "pat@other.example".into(),
            to: vec![USER.into()], subject: subject.into(), preview: format!("Body of {subject}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![],
            is_outgoing: false,
        },
        remote_id: Some(key.rsplit(':').next().unwrap_or("1").to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

/// The judge: while `busy` it fails every call as a model that cannot answer now (no code: a failure that can pass); a subject in `refuse`
/// is refused for the mail itself (set aside); a subject starting "Yes" gets yes for Needs Reply, any other a no.
struct Judge { busy: AtomicBool, refuse: Vec<&'static str>, pick: String, asked: Mutex<HashMap<String, usize>> }
impl DecisionPort for Judge {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let subject = state.fields.get("subject").cloned().unwrap_or_default();
        *self.asked.lock().unwrap().entry(subject.clone()).or_default() += 1;
        if self.busy.load(Ordering::SeqCst) {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Laya".into(), reason_words: "The on-device model is busy: try again".into(), retryable: true, code: None } });
        }
        if self.refuse.contains(&subject.as_str()) {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Apple Intelligence".into(), reason_words: "unsafe".into(), retryable: false, code: Some(::decision::decide::MAIL_REFUSED_CODE.into()) } });
        }
        let pick = subject.starts_with("Yes").then_some(self.pick.as_str());
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, pick, if pick.is_some() { 0.95 } else { 0.0 })).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some("fake-judge".into()) })
    }
}

struct Rig { core: RustCore, account: String, judge: Arc<Judge>, _dir: tempfile::TempDir }
impl Rig {
    /// A real core with the built-in Needs Reply as seeded (AI on, described, no step 1) and the other built-in labels' AI cards off.
    fn new(busy: bool, refuse: Vec<&'static str>) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
        for id in ["updates", "promotions", "social", "primary"] {
            let _ = core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false });
        }
        let pick = core.store().labels_full().unwrap().into_iter().find(|l| l.id == LABEL).expect("Needs Reply is seeded").name;
        let judge = Arc::new(Judge { busy: AtomicBool::new(busy), refuse, pick, asked: Mutex::new(HashMap::new()) });
        core.set_laya_port(Some(judge.clone()));
        Rig { core, account, judge, _dir: dir }
    }
    /// Mails arrive and run the labelling order (as the sync's worker does).
    fn arrive(&self, subjects: &[&str]) -> Vec<String> {
        let n0 = self.core.store().labels_full().map(|_| 0).unwrap_or(0);
        let mails: Vec<NewMessage> = subjects.iter().enumerate().map(|(i, s)| mail(&self.account, &format!("INBOX:{}", s.replace(' ', "-")), s, 1_000 + (n0 + i) as i64)).collect();
        let ids: Vec<String> = mails.iter().map(|m| m.message.id.clone()).collect();
        self.core.store().insert_messages(mails).unwrap();
        self.core.file_stored_messages(&ids).unwrap();
        ids
    }
    fn state(&self, path: String) -> String { self.core.state(path).unwrap_or_default() }
    fn counts(&self) -> String { self.state(format!("label/{LABEL}/flow/counts")) }
    fn field(&self, key: &str) -> String {
        let c = self.counts();
        c.split(&format!("\"{key}\":")).nth(1).unwrap_or_default().split([',', '}']).next().unwrap_or_default().to_string()
    }
    fn set_aside(&self) -> String { self.state(format!("label/{LABEL}/flow/ai_set_aside")) }
    fn decided_by(&self, id: &str) -> String { self.state(format!("message/{id}/label/{LABEL}/decided_by")) }
    fn settle(&self) {
        for _ in 0..40 {
            self.core.flush_filing();
            if self.field("waiting_for_ai") == "0" { break; }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

#[test]
fn when_every_mail_is_judged_nothing_waits() {
    let rig = Rig::new(false, vec![]);
    let ids = rig.arrive(&["Yes A", "No B"]);
    assert_eq!(rig.decided_by(&ids[0]), "\"laya\"", "{}", rig.counts());
    assert_eq!((rig.field("sorted_by_ai").as_str(), rig.field("waiting_for_ai").as_str(), rig.set_aside().as_str()), ("1", "0", "0"), "{}", rig.counts());
}

#[test]
fn some_failed_mails_wait_and_are_asked_again_in_the_background() {
    let rig = Rig::new(true, vec![]);
    let ids = rig.arrive(&["Yes A", "No B"]);
    assert_eq!(rig.field("waiting_for_ai"), "2", "the two mails the judge could not answer wait on the card: {}", rig.counts());
    // the judge answers again; the next mail's answer is the first after the failure: the waiting ones are asked in the background
    rig.judge.busy.store(false, Ordering::SeqCst);
    rig.arrive(&["No C"]);
    rig.settle();
    assert_eq!(rig.decided_by(&ids[0]), "\"laya\"", "the waiting yes is filed: {}", rig.counts());
    assert_eq!(rig.field("waiting_for_ai"), "0", "{}", rig.counts());
    assert_eq!(rig.field("sorted_by_ai"), "1", "{}", rig.counts());
    assert_eq!(rig.judge.asked.lock().unwrap().get("No B").copied(), Some(2), "B was asked twice: at arrival (busy) and in the background");
}

#[test]
fn a_set_aside_mail_does_not_wait_and_a_failed_one_does() {
    let rig = Rig::new(false, vec!["Refused A"]);
    rig.arrive(&["Refused A"]);
    rig.judge.busy.store(true, Ordering::SeqCst);
    rig.arrive(&["Yes B"]);
    assert_eq!(rig.set_aside(), "1", "{}", rig.counts());
    assert_eq!(rig.field("waiting_for_ai"), "1", "only the failed mail waits, not the one set aside: {}", rig.counts());
}

#[test]
fn sixty_failed_mails_are_asked_again_fifty_at_a_time_until_none_waits() {
    let rig = Rig::new(true, vec![]);
    let subjects: Vec<String> = (1..=60).map(|n| format!("Yes {n:02}")).collect();
    let refs: Vec<&str> = subjects.iter().map(String::as_str).collect();
    let ids = rig.arrive(&refs);
    assert_eq!(rig.field("waiting_for_ai"), "60", "{}", rig.counts());
    rig.judge.busy.store(false, Ordering::SeqCst);
    rig.arrive(&["No last"]);
    rig.settle();
    assert_eq!(rig.field("waiting_for_ai"), "0", "{}", rig.counts());
    assert!(ids.iter().all(|id| rig.decided_by(id) == "\"laya\""), "every waiting yes is filed: {}", rig.counts());
    // read straight from the table (the same SQL on the code before round 17, where no row has this mark)
    let db = rusqlite::Connection::open(rig.core.store().path()).unwrap();
    let marks: i64 = db.query_row("SELECT count(*) FROM ai_label_no WHERE decided_model = 'ai-waiting'", [], |r| r.get(0)).unwrap();
    assert_eq!(marks, 0, "no mail stays marked as waiting once judged");
}
