//! Lane fix-aisort round 16 (main114, the hidden copy with the real Laya weights; for the owner: "download the model, press Re-run, and the AI
//! still judges nothing"). Mail that arrives before Laya's weights are on this Mac is asked of Laya, which refuses (not on this Mac yet); the
//! core kept that failure (`decision_model.rs` `laya_failure`) until a judgement succeeded, and while it stood `ai_can_answer` was false, so a
//! Re-run took the skip-the-AI path (`flow_ctx.rs` `rerun_label`, `ai_skipped`: total 0, model_pending true) and nothing asked the model again.
//!
//! Now a Laya failure that only waits for the model (the shell's codes `laya_not_ready` and `laya_load_failed`, LayaOnDevice.swift) never
//! stops the asking: a Re-run asks the model, an answer after the failure has the worker ask the mails that waited (in the background, no
//! Re-run), and a saved decision model (the shell re-saves the model in force when Laya's weights arrive) clears the failure and asks again.
//! A failure of another kind blocks as before. The model is a fake handed over as the shell hands Laya (`RustCore::set_laya_port`).
//! Two labels: the built-in Needs Reply as seeded (no step 1: its mails reach a re-run only as the AI's candidates, `flow_ctx.rs`
//! `candidate_parts`, which the skip path drops: main114's case), and "To read" (step 1 = unread), whose picked mails wait
//! (`label/<id>/flow/counts` `waiting_for_ai`) and are the ones the background asks again (`label_ai_failure::retry_ids`, as for the Apple
//! judge). A label with no step 1 counts no waiting mail, so its earlier mails are reached by a re-run (round 16's report).
//!
//! Run: cargo test -p engine --test engine_laya_after_download -- --test-threads=1
//!
//! On the code before round 16 these fail: `a_rerun_after_the_model_arrives_asks_it` (model_pending true: the model was not asked),
//! `the_mails_that_waited_are_judged_when_the_model_answers_again` (the two first mails still wait),
//! `saving_the_decision_model_asks_the_waiting_mails_again` (both still wait). The controls pass on both:
//! `a_failure_of_another_kind_still_skips_the_ai_on_rerun`, `the_card_says_why_until_the_model_answers`.
//!
//! Broken copies (each one edit; each must turn its test red):
//! - D1 engine/src/decision_model.rs `LabelModel::blocks_asking`: `!(local && waits)` -> `true` -> red: `a_rerun_after_the_model_arrives_asks_it`.
//! - D2 engine/src/filing_worker.rs `Filing::start`: `if work.model.take_recovered() {` -> `if false && work.model.take_recovered() {` -> red:
//!   `the_mails_that_waited_are_judged_when_the_model_answers_again`.
//! - D3 engine/src/rust_core.rs `SaveDecisionModel`: delete `self.deps.filing.retry_waiting();` -> red: `saving_the_decision_model_asks_the_waiting_mails_again`.
//! - D4 engine/src/decision_model.rs `waits_for_the_model`: `Some(LAYA_NOT_READY_CODE) | Some(LAYA_LOAD_FAILED_CODE)` -> `Some("none")` -> red:
//!   `a_rerun_after_the_model_arrives_asks_it`.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use contracts::*;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const NOT_READY_WORDS: &str = "The on-device decision model is not on this Mac yet: download it in Settings, Decisions.";

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

/// Laya as the shell hands it over at launch: until `ready` it refuses as LayaOnDevice does without its weights (`code`), then it answers
/// yes at 0.93 for the label `pick` (Needs Reply's path) in every Labels question.
struct ArrivingLaya { ready: AtomicBool, code: Option<&'static str>, words: &'static str, pick: String, calls: Mutex<usize> }
impl DecisionPort for ArrivingLaya {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        *self.calls.lock().unwrap() += 1;
        if !self.ready.load(Ordering::SeqCst) {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Laya".into(), reason_words: self.words.into(), retryable: false, code: self.code.map(str::to_string) } });
        }
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, Some(&self.pick), 0.93)).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some("fake-laya".into()) })
    }
}

struct Rig { core: RustCore, account: String, label: String, laya: Arc<ArrivingLaya>, _dir: tempfile::TempDir }
impl Rig {
    /// A real core, the other built-in labels' AI cards off, and Laya handed over WITHOUT its weights (`code` on its refusal). `needs_reply`:
    /// the label is the built-in Needs Reply as seeded (AI on, described, no step 1); else "To read" (AI on, described, step 1 = unread) and
    /// Needs Reply's AI card off.
    fn new(needs_reply: bool, code: Option<&'static str>, words: &'static str) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
        for id in ["updates", "promotions", "social", "primary"].into_iter().chain((!needs_reply).then_some("needs-reply")) {
            let _ = core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false });
        }
        let label = if needs_reply { "needs-reply".to_string() } else {
            core.store().label_create_full(
                LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
                            rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false },
                1,
            ).unwrap().id
        };
        let pick = core.store().labels_full().unwrap().into_iter().find(|l| l.id == label).expect("the label exists").name;
        let laya = Arc::new(ArrivingLaya { ready: AtomicBool::new(false), code, words, pick, calls: Mutex::new(0) });
        core.set_laya_port(Some(laya.clone()));
        Rig { core, account, label, laya, _dir: dir }
    }
    /// One mail arrives and runs the labelling order (as the sync's worker does).
    fn arrive(&self, key: &str) -> String {
        let id = format!("{}:{key}", self.account);
        self.core.store().insert_messages(vec![mail(&self.account, key, &format!("Subject {key}"), 1_000)]).unwrap();
        self.core.file_stored_messages(&[id.clone()]).unwrap();
        id
    }
    fn state(&self, path: String) -> String { self.core.state(path).unwrap_or_default() }
    fn counts(&self) -> String { self.state(format!("label/{}/flow/counts", self.label)) }
    fn waiting(&self) -> String { self.counts().rsplit("\"waiting_for_ai\":").next().unwrap_or_default().trim_end_matches('}').to_string() }
    fn decided_by(&self, id: &str) -> String { self.state(format!("message/{id}/label/{}/decided_by", self.label)) }
    /// The worker's jobs, until no mail waits or 20 rounds pass (a flush waits only for what is queued at the time).
    fn settle(&self) {
        for _ in 0..20 {
            self.core.flush_filing();
            if self.waiting() == "0" { break; }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    /// The weights arrive (the download's end).
    fn model_arrives(&self) { self.laya.ready.store(true, Ordering::SeqCst); }
    fn calls(&self) -> usize { *self.laya.calls.lock().unwrap() }
}

fn local_model() -> DecisionModel {
    DecisionModel { provider: DecisionProviderKind::Local, model: Some("laya".into()), keychain: None, decisions_enabled: vec![], question_words: Default::default(), last_run_ms: None }
}

#[test]
fn a_rerun_after_the_model_arrives_asks_it() {
    let rig = Rig::new(true, Some("laya_not_ready"), NOT_READY_WORDS);
    let a = rig.arrive("INBOX:1");
    let b = rig.arrive("INBOX:2");
    assert_eq!(rig.calls(), 2, "each arrival asked Laya, which refused (no weights)");
    assert_eq!((rig.decided_by(&a).as_str(), rig.decided_by(&b).as_str()), ("", ""), "nothing filed while the weights are missing");
    rig.model_arrives();
    rig.core.run(Command::RerunLabel { label_id: rig.label.clone() }).unwrap();
    rig.settle();
    let total = rig.state(format!("label/{}/rerun/total", rig.label));
    let pending = rig.state(format!("label/{}/rerun/model_pending", rig.label));
    assert!(total != "0" && pending == "false", "the re-run asked the model: total {total}, model_pending {pending}");
    assert_eq!((rig.decided_by(&a).as_str(), rig.decided_by(&b).as_str()), ("\"laya\"", "\"laya\""), "both filed by laya; counts {}", rig.counts());
    assert_eq!(rig.state(format!("label/{}/flow/ai_refusal", rig.label)), "null", "the card no longer says the model is missing");
}

#[test]
fn the_mails_that_waited_are_judged_when_the_model_answers_again() {
    let rig = Rig::new(false, Some("laya_not_ready"), NOT_READY_WORDS);
    let a = rig.arrive("INBOX:1");
    let b = rig.arrive("INBOX:2");
    assert_eq!(rig.waiting(), "2", "{}", rig.counts());
    rig.model_arrives();
    // a new mail arrives after the download: its answer is the first after the failure; no Re-run, no Save
    let c = rig.arrive("INBOX:3");
    assert_eq!(rig.decided_by(&c), "\"laya\"", "the new mail is judged at once");
    rig.settle();
    assert_eq!((rig.decided_by(&a).as_str(), rig.decided_by(&b).as_str()), ("\"laya\"", "\"laya\""), "the mails that waited are judged in the background: {}", rig.counts());
    assert_eq!(rig.waiting(), "0", "{}", rig.counts());
}

#[test]
fn saving_the_decision_model_asks_the_waiting_mails_again() {
    let rig = Rig::new(false, Some("laya_load_failed"), "The decision model config has no hidden_size. Delete the model and download it again.");
    let a = rig.arrive("INBOX:1");
    let b = rig.arrive("INBOX:2");
    assert_eq!(rig.waiting(), "2", "{}", rig.counts());
    rig.model_arrives();
    // the shell re-saves the model in force when Laya's weights arrive (the download's end)
    rig.core.run(Command::SaveDecisionModel { model: local_model() }).unwrap();
    rig.settle();
    assert_eq!((rig.decided_by(&a).as_str(), rig.decided_by(&b).as_str()), ("\"laya\"", "\"laya\""), "{}", rig.counts());
    assert_eq!(rig.waiting(), "0", "{}", rig.counts());
}

#[test]
fn a_failure_of_another_kind_still_skips_the_ai_on_rerun() {
    // the control: a failure that is not about the missing model (no code) keeps today's rule: a Re-run does not ask the model
    let rig = Rig::new(true, None, "The on-device model could not answer: GPU timeout");
    let a = rig.arrive("INBOX:1");
    let asked = rig.calls();
    rig.core.run(Command::RerunLabel { label_id: rig.label.clone() }).unwrap();
    rig.settle();
    // a failure of another kind (no code): the re-run skips the AI as before; Needs Reply has no step 1, so it looks at no message
    assert_eq!(rig.state(format!("label/{}/rerun/total", rig.label)), "0");
    assert_eq!(rig.state(format!("label/{}/rerun/model_pending", rig.label)), "true");
    assert_eq!(rig.calls(), asked, "the re-run did not ask the failing model");
    assert_eq!(rig.decided_by(&a), "", "nothing filed");
}

#[test]
fn the_card_says_why_until_the_model_answers() {
    // the control: the missing weights are still said on the card while they are missing (the asking is allowed, the words stay)
    let rig = Rig::new(true, Some("laya_not_ready"), NOT_READY_WORDS);
    rig.arrive("INBOX:1");
    let refusal = rig.state(format!("label/{}/flow/ai_refusal", rig.label));
    assert!(refusal.contains("not on this Mac yet"), "{refusal}");
}
