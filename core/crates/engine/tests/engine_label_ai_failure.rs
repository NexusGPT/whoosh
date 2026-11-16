//! Lane fix-aisort (owner 2026-11-12 15:02, a picture of "Needs Reply": "looks like filter not working yet. AI filter seeing 0 messgaes"): a
//! judgement of the label AI that failed, is absent or cannot be read is NEVER kept as the AI's "no" (`ai_label_no`, migration 0022): the
//! mail stays waiting (`label/<id>/flow/counts` `waiting_for_ai`), core.log holds ONE line with the label id, the count waiting and the
//! model's own words, and `label/<id>/flow/ai_refusal` says it. A "yes" is a `label_assignment` row decided by "laya"; a readable "no" is one
//! `ai_label_no` row. The "no" rows Laya wrote before the fix are cleared once at the next start, with a log line giving the count.
//!
//! The model is a fake handed over exactly as the shell hands Laya (`RustCore::set_laya_port`, `decision_model.rs` `LabelModel::set_laya`);
//! with no `decision_model` row the label AI uses it (the owner's Mac: `decision_model` 0 rows). Real store, real engine, no network.
//!
//! Run (the lead, one test at a time):
//!   cargo test -p engine --test engine_label_ai_failure -- --test-threads=1 <test name>
//!
//! On the code before lane fix-aisort (commit 2bc52b3dc) these fail: `a_failing_model_keeps_no_no_row_and_logs_one_line` (no log line),
//! `an_absent_model_keeps_the_mail_waiting_and_logs_one_line` (no log line), `an_answer_with_no_answer_to_the_question_is_not_a_no` and
//! `an_answer_with_no_usable_probability_is_not_a_no` (a "no" row is written), `the_old_laya_no_rows_are_cleared_once_at_the_next_start`
//! (the rows stay). `a_yes_files_the_mail_decided_by_laya` and `a_readable_no_keeps_one_no_row` pass on both (the controls).
//!
//! Broken copies (each one edit; each must turn its test red):
//! - B1 core/crates/decision/src/decide.rs `unreadable`: `let Some(a) = answer else { return Some("the model gave no answer to the question".into()) };`
//!   -> `let Some(a) = answer else { return None };` -> red: `an_answer_with_no_answer_to_the_question_is_not_a_no`.
//! - B2 decide.rs `unreadable`: `Some(p) if !(p.is_finite() && (0.0..=1.0).contains(&p))` -> `Some(p) if false && p.is_nan()` -> red:
//!   `an_answer_with_no_usable_probability_is_not_a_no`.
//! - B3 engine/src/label_ai_failure.rs `note_run`: `lines.push(format!("label AI {l}: ` -> `if false { lines.push(format!("label AI {l}: ` with
//!   its closing `)); }` -> red: `a_failing_model_keeps_no_no_row_and_logs_one_line`, `an_absent_model_keeps_the_mail_waiting_and_logs_one_line`.
//! - B4 engine/src/rust_core.rs `assemble`: delete `crate::label_ai_failure::clear_old_nos_once(&store);` -> red:
//!   `the_old_laya_no_rows_are_cleared_once_at_the_next_start`.
//! - B5 engine/src/decision_model.rs `LabelModel::state`: the `None if label.ai_enabled && crate::label_ai_failure::failure_of(` arm -> `None if false &&`
//!   -> red: `an_answer_with_no_answer_to_the_question_is_not_a_no` (ai_refusal is null).
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use contracts::*;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

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

/// An inbound, unread INBOX mail (step 1 of "To read", "unread", picks it).
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

/// What the fake model does with the Labels question.
#[derive(Clone)]
enum Answer {
    /// `Ok` with this label path at this probability.
    Pick(&'static str, f64),
    /// `Ok` with no answer at all.
    Nothing,
    /// `Err` in the model's own words (Laya's refusal shape).
    Fail(&'static str),
}

struct FakeLaya { answer: Answer, calls: Mutex<usize> }
impl DecisionPort for FakeLaya {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        *self.calls.lock().unwrap() += 1;
        let answers = match &self.answer {
            Answer::Fail(words) => return Err(CoreError::Action { failure: ActionFailed { account_name: "Laya".into(), reason_words: (*words).into(), retryable: false, code: None } }),
            Answer::Nothing => vec![],
            // round 3: by the question's form; a pick that is not an offered label ("nothing fits") is a choice outside the labels, or a no everywhere
            Answer::Pick(choice, p) => questions.iter().filter_map(|q| match q {
                DecisionQuestion::Choice { id, .. } => Some(DecisionAnswer::Choice { id: id.clone(), choice: (*choice).into(), probabilities: HashMap::from([((*choice).to_string(), *p)]) }),
                _ => ::decision::test_answers::labels_answer(q, Some(choice), *p),
            }).collect(),
        };
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some("fake-laya".into()) })
    }
}

struct Rig { core: RustCore, account: String, label: String, dir: tempfile::TempDir }
impl Rig {
    /// A real core with one AI-on label "To read" (described; step 1 = unread) and the built-in labels' AI cards off; `answer` = None hands no model over.
    fn new(answer: Option<Answer>) -> (Rig, Option<Arc<FakeLaya>>) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
        for id in ["needs-reply", "updates", "promotions", "social", "primary"] {
            let _ = core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false });
        }
        let label = core.store().label_create_full(
            LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
                        rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false },
            1,
        ).unwrap().id;
        let fake = answer.map(|a| Arc::new(FakeLaya { answer: a, calls: Mutex::new(0) }));
        if let Some(f) = &fake { core.set_laya_port(Some(f.clone())); }
        (Rig { core, account, label, dir }, fake)
    }
    /// One mail arrives and runs the labelling order (as the sync's worker does).
    fn arrive(&self, key: &str) -> String {
        let id = format!("{}:{key}", self.account);
        self.core.store().insert_messages(vec![mail(&self.account, key, &format!("Subject {key}"), 1_000)]).unwrap();
        self.core.file_stored_messages(&[id.clone()]).unwrap();
        id
    }
    fn nos(&self) -> Vec<String> { self.core.store().ai_no_messages(&self.label).unwrap() }
    fn counts(&self) -> String { self.core.state(format!("label/{}/flow/counts", self.label)).unwrap() }
    fn refusal(&self) -> String { self.core.state(format!("label/{}/flow/ai_refusal", self.label)).unwrap_or_default() }
    fn log(&self) -> String { std::fs::read_to_string(self.dir.path().join("core.log")).unwrap_or_default() }
    fn label_lines(&self) -> Vec<String> { self.log().lines().filter(|l| l.contains(&format!("label AI {}:", self.label))).map(str::to_string).collect() }
}

fn waiting(counts: &str) -> String { counts.rsplit("\"waiting_for_ai\":").next().unwrap_or_default().trim_end_matches('}').to_string() }

#[test]
fn a_failing_model_keeps_no_no_row_and_logs_one_line() {
    let (rig, fake) = Rig::new(Some(Answer::Fail("The on-device model could not answer: GPU timeout")));
    let id = rig.arrive("INBOX:1");
    rig.arrive("INBOX:2");
    assert_eq!(*fake.unwrap().calls.lock().unwrap(), 2, "the model was asked about each mail");
    assert!(rig.nos().is_empty(), "a failure is never kept as a no: {:?}", rig.nos());
    assert_eq!(waiting(&rig.counts()), "2", "both mails wait: {}", rig.counts());
    let lines = rig.label_lines();
    assert_eq!(lines.len(), 1, "ONE line for the same failure: {lines:?}");
    assert!(lines[0].contains("mails wait for the AI, which could not judge them: Laya: The on-device model could not answer: GPU timeout"), "{lines:?}");
    assert!(!rig.log().contains("Subject INBOX") && !rig.log().contains("Body of"), "no mail text in core.log");
    assert!(rig.refusal().contains("GPU timeout"), "the card says why: {}", rig.refusal());
    assert!(rig.core.state(format!("message/{id}/label/{}/decided_by", rig.label)).is_none(), "not filed");
}

#[test]
fn an_absent_model_keeps_the_mail_waiting_and_logs_one_line() {
    let (rig, _) = Rig::new(None);
    rig.arrive("INBOX:1");
    assert!(rig.nos().is_empty());
    assert_eq!(waiting(&rig.counts()), "1", "{}", rig.counts());
    let lines = rig.label_lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("1 mails wait for the AI, which could not judge them: No model is set for sorting mail"), "{lines:?}");
    assert!(rig.refusal().contains("No model is set for sorting mail: choose one in Settings, Decisions."), "{}", rig.refusal());
}

#[test]
fn an_answer_with_no_answer_to_the_question_is_not_a_no() {
    let (rig, _) = Rig::new(Some(Answer::Nothing));
    rig.arrive("INBOX:1");
    assert!(rig.nos().is_empty(), "an unreadable answer is not a no: {:?}", rig.nos());
    assert_eq!(waiting(&rig.counts()), "1", "{}", rig.counts());
    let lines = rig.label_lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("the model's answer could not be read: the model gave no answer to the question"), "{lines:?}");
    assert_eq!(rig.refusal(), "{\"code\":\"ai_failed\",\"words\":\"The AI could not judge the mail of this label. It tries again in the background; core.log says why.\"}");
}

#[test]
fn an_answer_with_no_usable_probability_is_not_a_no() {
    let (rig, _) = Rig::new(Some(Answer::Pick("To read", f64::NAN)));
    rig.arrive("INBOX:1");
    assert!(rig.nos().is_empty(), "a NaN probability is not a no: {:?}", rig.nos());
    assert_eq!(waiting(&rig.counts()), "1", "{}", rig.counts());
    assert!(rig.label_lines().iter().any(|l| l.contains("carries no probability from 0 to 1")), "{:?}", rig.label_lines());
}

#[test]
fn a_yes_files_the_mail_decided_by_laya() {
    let (rig, _) = Rig::new(Some(Answer::Pick("To read", 0.95)));
    let id = rig.arrive("INBOX:1");
    assert_eq!(rig.core.state(format!("message/{id}/label/{}/decided_by", rig.label)).as_deref(), Some("\"laya\""), "a label_assignment row decided by laya");
    assert!(rig.nos().is_empty());
    assert!(rig.counts().contains("\"sorted_by_ai\":1") && waiting(&rig.counts()) == "0", "{}", rig.counts());
    assert!(rig.log().contains("label AI: judged 1 mails: 1 accepted, 0 under the trust level"), "{}", rig.log());
    assert_eq!(rig.refusal(), "null");
}

#[test]
fn a_readable_no_keeps_one_no_row() {
    let (rig, _) = Rig::new(Some(Answer::Pick("nothing fits", 0.9)));
    let id = rig.arrive("INBOX:1");
    assert_eq!(rig.nos(), vec![id], "one no-row");
    assert_eq!(waiting(&rig.counts()), "0", "{}", rig.counts());
    assert!(rig.label_lines().is_empty(), "a readable no is no failure: {:?}", rig.label_lines());
}

#[test]
fn the_old_laya_no_rows_are_cleared_once_at_the_next_start() {
    // the owner's database before the fix: Laya's "no" rows, no marker
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whoosh.sqlite");
    let (account, label, first) = {
        let store = Store::open(&path).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let label = store.label_create_full(
            LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
                        rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false },
            1,
        ).unwrap().id;
        store.insert_messages(vec![mail(&account, "INBOX:1", "Subject one", 1_000), mail(&account, "INBOX:2", "Subject two", 1_100)]).unwrap();
        let first = format!("{account}:INBOX:1");
        store.ai_no_put(vec![(first.clone(), label.clone()), (format!("{account}:INBOX:2"), label.clone())], DecidedBy::Laya, 5).unwrap();
        assert_eq!(store.ai_no_messages(&label).unwrap().len(), 2);
        (account, label, first)
    };
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert!(core.store().ai_no_messages(&label).unwrap().is_empty(), "cleared at the start");
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert!(log.contains("label AI: 2 kept \"no\" answers of Laya written before an unreadable answer stopped counting as a \"no\" were cleared once"), "{log}");
    // a "no" kept after the fix survives the next start: once only
    core.store().ai_no_put(vec![(first.clone(), label.clone())], DecidedBy::Laya, 6).unwrap();
    drop(core);
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert_eq!(core.store().ai_no_messages(&label).unwrap(), vec![first], "not cleared twice");
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert_eq!(log.lines().filter(|l| l.contains("were cleared once")).count(), 1, "{log}");
    let _ = account;
}

// ---------------------------------------------------------------- round 2: every answer under the trust level (owner's Mac, 12 November:
// "judged 50 mails: 0 accepted, 50 under the trust level 0.80 (highest probability 0.58)")
// Broken copies (round 2): R1 engine/src/label_ai_failure.rs `note_spread`: `e.1 = e.1.max(highest);` -> `e.1 = 0.0;` -> red:
// `an_answer_under_the_trust_level_says_so_on_the_card_and_logs_the_spread` (highest_percent 0). R2 engine/src/decision_model.rs `state`: the arm
// `_ if label.ai_enabled && under.is_some() =>` -> `_ if false =>` -> red: the same test (ai_refusal null).

#[test]
fn an_answer_under_the_trust_level_says_so_on_the_card_and_logs_the_spread() {
    let (rig, _) = Rig::new(Some(Answer::Pick("To read", 0.58)));
    rig.arrive("INBOX:1");
    rig.arrive("INBOX:2");
    assert_eq!(rig.nos().len(), 2, "a readable answer under the trust level is the AI's no: {:?}", rig.nos());
    assert_eq!(rig.refusal(), "{\"code\":\"ai_under_trust\",\"highest_percent\":58,\"judged\":2,\"trust_percent\":80,\"words\":\"The AI judged %d mails and was sure of none: its best was %d %%, and it files from %d %%.\"}");
    let spread: Vec<String> = rig.log().lines().filter(|l| l.contains(&format!("label AI: {}: 1 judged;", rig.label))).map(str::to_string).collect();
    assert_eq!(spread.len(), 2, "one spread line per label per run: {spread:?}");
    assert!(spread[0].ends_with("probability under 0.2: 0, 0.2 to 0.4: 0, 0.4 to 0.5: 0, 0.5 to 0.6: 1, 0.6 to 0.80: 0, 0.80 and over: 0"), "{spread:?}");
    assert!(!rig.log().contains("Subject INBOX"), "no mail text");
    // the AI becomes sure of one: the card's reason goes
    rig.core.set_laya_port(Some(Arc::new(FakeLaya { answer: Answer::Pick("To read", 0.93), calls: Mutex::new(0) })));
    rig.arrive("INBOX:3");
    assert_eq!(rig.refusal(), "null");
}

// ---------------------------------------------------------------- round 4: a changed question clears the kept "no" at the next start
// Broken copy R4: engine/src/rust_core.rs `assemble`: delete `crate::label_ai_failure::clear_on_question_change(&store);` -> red:
// `a_changed_question_clears_the_kept_no_at_the_next_start`.

#[test]
fn a_changed_question_clears_the_kept_no_at_the_next_start() {
    // the owner's Mac after main97: the one-time clean-up already ran (its marker exists), the kept "no" answers were given under the choice
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whoosh.sqlite");
    let (label, message) = {
        let store = Store::open(&path).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let label = store.label_create_full(
            LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
                        rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false },
            1,
        ).unwrap().id;
        store.insert_messages(vec![mail(&account, "INBOX:1", "Subject one", 1_000)]).unwrap();
        store.ai_no_put(vec![(format!("{account}:INBOX:1"), label.clone())], DecidedBy::Laya, 5).unwrap();
        (label, format!("{account}:INBOX:1"))
    };
    std::fs::write(dir.path().join("ai-label-no-cleared"), "215\n").unwrap();
    std::fs::write(dir.path().join("ai-label-question"), "choice|0.8\n").unwrap();
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert!(core.store().ai_no_messages(&label).unwrap().is_empty(), "the no given under the old question goes");
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert!(log.contains("label AI: the question changed from choice|0.8 to ") && log.contains(": 1 kept \"no\" answers of the model were cleared"), "{log}");
    let now = std::fs::read_to_string(dir.path().join("ai-label-question")).unwrap();
    assert_ne!(now.trim(), "choice|0.8", "the marker holds the question in force");
    // the next start with the same question clears nothing
    core.store().ai_no_put(vec![(message.clone(), label.clone())], DecidedBy::Laya, 6).unwrap();
    drop(core);
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert_eq!(core.store().ai_no_messages(&label).unwrap().len(), 1, "same question: the no stays");
}

// ---------------------------------------------------------------- round 7: a mail removed during the judgement (owner's Mac, main99, twice:
// "the AI's answer could not be kept: database error: FOREIGN KEY constraint failed", after "Some of the requested messages no longer exist")
// Broken copy R7: core/crates/store/src/label_meta.rs `ai_no_put`: the statement back to
// `"INSERT OR REPLACE INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES (?1, ?2, ?3, ?4)"` -> red:
// `a_mail_removed_while_the_batch_is_judged_does_not_stop_the_others_no_from_being_kept` (the run answers Err).

/// Says no to every label, and on its first question removes the OTHER mail of the batch from the store (as a sync that learns the server
/// no longer has it); the batch's facts were read before, so that mail is still judged and its "no" reaches the write.
struct RemovingLaya { store: Arc<Store>, mails: Vec<(String, String)>, removed: Mutex<Option<String>> }
impl DecisionPort for RemovingLaya {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let subject = state.fields.get("subject").cloned().unwrap_or_default();
        let mut removed = self.removed.lock().unwrap();
        if removed.is_none() {
            if let Some((id, _)) = self.mails.iter().find(|(_, s)| *s != subject) {
                self.store.delete_messages(vec![id.clone()]).unwrap();
                *removed = Some(id.clone());
            }
        }
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, None, 0.0)).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

#[test]
fn a_mail_removed_while_the_batch_is_judged_does_not_stop_the_others_no_from_being_kept() {
    let (rig, _) = Rig::new(None);
    let (a, b) = (format!("{}:INBOX:1", rig.account), format!("{}:INBOX:2", rig.account));
    rig.core.store().insert_messages(vec![mail(&rig.account, "INBOX:1", "Subject one", 1_000), mail(&rig.account, "INBOX:2", "Subject two", 1_100)]).unwrap();
    let port = Arc::new(RemovingLaya { store: rig.core.store(), mails: vec![(a.clone(), "Subject one".into()), (b.clone(), "Subject two".into())], removed: Mutex::new(None) });
    rig.core.set_laya_port(Some(port.clone()));
    let filed = rig.core.file_stored_messages(&[a.clone(), b.clone()]).expect("the batch's write does not fail on the removed mail");
    let gone = port.removed.lock().unwrap().clone().expect("one mail was removed during the judgement");
    let kept = if gone == a { b.clone() } else { a.clone() };
    assert_eq!(filed.len(), 2, "both mails were judged: {filed:?}");
    assert_eq!(rig.nos(), vec![kept], "the remaining mail's no is kept; the removed one writes nothing");
}

// ---------------------------------------------------------------- round 8: Apple's on-device model as the Labels judge (through the shell's port)
// The shell's router (Sources/Features/OnDeviceAI/Contract/AppleLabelJudge.swift) answers the per-label Labels question in the form below; these
// fakes stand in for it at the engine's door (`set_laya_port`): a pick is yes 1.0 for the label and 0.0 for the others, "none" is 0.0 for all, an
// unreadable answer THROWS, and a fallback to Laya carries the checkpoint `fallback:<reason>`.
// Broken copies (round 8):
// - J1 engine/src/decision_model.rs `LayaPort::decide`: `let judge = if checkpoint == APPLE_CHECKPOINT { "apple" } else { "laya" };` -> `let judge = "laya";`
//   -> red: `a_change_of_judge_clears_the_kept_no_of_the_other_judge`.
// - J2 engine/src/decision_model.rs `LabelModel::state`: the `ai_needs_apple` arm's `== Some("not_enabled")` -> `== Some("never")` -> red:
//   `when_apple_intelligence_is_off_the_labels_fall_back_and_the_card_says_why`.
// - J3 engine/src/label_ai_failure.rs `note_spread`: delete the `if judge_of(store.path()).as_deref() == Some("apple") { ... return; }` block -> red:
//   `apple_answering_none_keeps_a_no_and_files_nothing` (the card would say "sure of none: its best was 0 %").

/// The shell's router as the engine sees it: `pick` None = "none"; `fail` = the router's unreadable answer; `checkpoint` names the judge.
struct RouterFake { pick: Option<&'static str>, fail: bool, checkpoint: &'static str }
impl DecisionPort for RouterFake {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let _ = state;
        if self.fail {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Apple Intelligence".into(), reason_words: "Apple Intelligence answered with no label of the list.".into(), retryable: true, code: None } });
        }
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, self.pick, 1.0).map(|a| match a {
            // the router answers 1.0 or 0.0, never the fakes' FAKE_NO
            DecisionAnswer::Noul { id, yes, .. } => DecisionAnswer::Noul { id, yes, probability: if yes { 1.0 } else { 0.0 } },
            other => other,
        })).collect();
        // the router names Apple's model as the one that answered (contracts `DecisionProviderKind::Apple`, round 8); a fallback is Laya's
        let answered_by = if self.checkpoint == "apple-on-device" { DecisionProviderKind::Apple } else { DecisionProviderKind::Local };
        Ok(DecisionResult { answers, answered_by, checkpoint: Some(self.checkpoint.into()) })
    }
}

fn judged_by(rig: &Rig, fake: RouterFake) { rig.core.set_laya_port(Some(Arc::new(fake))); }

#[test]
fn an_apple_pick_files_the_mail() {
    let (rig, _) = Rig::new(None);
    judged_by(&rig, RouterFake { pick: Some("To read"), fail: false, checkpoint: "apple-on-device" });
    let id = rig.arrive("INBOX:1");
    assert_eq!(rig.core.state(format!("message/{id}/label/{}/decided_by", rig.label)).as_deref(), Some("\"apple\""), "filed, and named after the model that decided (migration 0031)");
    assert!(rig.nos().is_empty());
    assert_eq!(rig.refusal(), "null");
}

#[test]
fn apple_answering_none_keeps_a_no_and_files_nothing() {
    let (rig, _) = Rig::new(None);
    judged_by(&rig, RouterFake { pick: None, fail: false, checkpoint: "apple-on-device" });
    let id = rig.arrive("INBOX:1");
    assert!(rig.core.state(format!("message/{id}/label/{}/decided_by", rig.label)).is_none(), "not filed");
    assert_eq!(rig.nos(), vec![id.clone()], "a readable none is the judge's no (kept as decided by apple: migration 0031)");
    assert_eq!(rig.refusal(), "null", "no under-trust sentence for a judge that answers yes or no");
}

#[test]
fn an_unreadable_apple_answer_is_a_failure_not_a_no() {
    let (rig, _) = Rig::new(None);
    judged_by(&rig, RouterFake { pick: None, fail: true, checkpoint: "apple-on-device" });
    rig.arrive("INBOX:1");
    assert!(rig.nos().is_empty(), "{:?}", rig.nos());
    assert_eq!(waiting(&rig.counts()), "1", "{}", rig.counts());
    assert!(rig.label_lines().iter().any(|l| l.contains("Apple Intelligence answered with no label of the list.")), "{:?}", rig.label_lines());
}

#[test]
fn when_apple_intelligence_is_off_the_labels_fall_back_and_the_card_says_why() {
    let (rig, _) = Rig::new(None);
    // Laya answered (the fallback), under the trust level
    judged_by(&rig, RouterFake { pick: None, fail: false, checkpoint: "fallback:not_enabled" });
    rig.arrive("INBOX:1");
    assert_eq!(rig.refusal(), "{\"code\":\"ai_needs_apple\",\"words\":\"Sorting mail into labels needs Apple Intelligence: turn it on in System Settings, then Apple Intelligence and Siri.\"}");
    // Apple's model is back: the sentence goes
    judged_by(&rig, RouterFake { pick: Some("To read"), fail: false, checkpoint: "apple-on-device" });
    rig.arrive("INBOX:2");
    assert_eq!(rig.refusal(), "null");
}

#[test]
fn a_change_of_judge_clears_the_kept_no_of_the_other_judge() {
    let (rig, _) = Rig::new(None);
    // Laya judged first (no mark yet: its answers are the ones kept, nothing goes) and said no
    judged_by(&rig, RouterFake { pick: None, fail: false, checkpoint: "laya-typed-decisions" });
    let first = rig.arrive("INBOX:1");
    assert_eq!(rig.nos(), vec![first.clone()]);
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("ai-label-judge")).unwrap().trim(), "laya");
    // Apple's model answers next: Laya's kept no goes, the log says so, the mail waits to be judged again
    judged_by(&rig, RouterFake { pick: None, fail: false, checkpoint: "apple-on-device" });
    let second = rig.arrive("INBOX:2");
    assert_eq!(rig.nos(), vec![second], "only the new judge's no is kept");
    assert!(rig.log().contains("label AI: the judge changed from laya to apple: 1 kept \"no\" answers of the model were cleared"), "{}", rig.log());
    assert_eq!(std::fs::read_to_string(rig.dir.path().join("ai-label-judge")).unwrap().trim(), "apple");
}

// ---------------------------------------------------------------- round 9: no judge mark at start (owner's Mac, main102: every mail held Laya's
// kept "no", so none waited, none was asked, and Apple's model never gave the first answer that clears them)
// Broken copy R9: engine/src/rust_core.rs `assemble`: delete `crate::label_ai_failure::clear_without_judge_mark(&store);` -> red:
// `a_store_with_kept_no_and_no_judge_mark_asks_its_mails_again_after_a_start`.

/// The owner's database after main100: one AI-on label (step 1 = unread), one unread mail, Laya's kept "no" for it, the round-1 and round-4
/// marks, and `judge` as the judge mark (None = no mark).
fn owners_store(judge: Option<&str>) -> (tempfile::TempDir, std::path::PathBuf, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("whoosh.sqlite");
    let (label, message) = {
        let store = Store::open(&path).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let label = store.label_create_full(
            LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
                        rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false },
            1,
        ).unwrap().id;
        store.insert_messages(vec![mail(&account, "INBOX:1", "Subject one", 1_000)]).unwrap();
        let message = format!("{account}:INBOX:1");
        store.ai_no_put(vec![(message.clone(), label.clone())], DecidedBy::Laya, 5).unwrap();
        (label, message)
    };
    std::fs::write(dir.path().join("ai-label-no-cleared"), "215\n").unwrap();
    // the question in force (per-label, the ledger's MODEL_TRUST_LABEL 0.8), so the round-4 clean-up has nothing to do and only the judge mark decides
    std::fs::write(dir.path().join("ai-label-question"), "per-label|0.8\n").unwrap();
    if let Some(j) = judge { std::fs::write(dir.path().join("ai-label-judge"), format!("{j}\n")).unwrap(); }
    (dir, path, label, message)
}

#[test]
fn a_store_with_kept_no_and_no_judge_mark_asks_its_mails_again_after_a_start() {
    let (dir, path, label, message) = owners_store(None);
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert!(core.store().ai_no_messages(&label).unwrap().is_empty(), "the kept no goes at the start, not at a first answer");
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert!(log.contains("label AI: no judge is marked yet: 1 kept \"no\" answers of the model were cleared; their mails are judged again in the background"), "{log}");
    assert!(dir.path().join("ai-label-judge").exists(), "the mark is written, so the next start does not clear again");
    // the shell hands its model over: the waiting mail is asked, and Apple's model files it
    for id in ["needs-reply", "updates", "promotions", "social", "primary"] {
        let _ = core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false });
    }
    core.set_laya_port(Some(Arc::new(RouterFake { pick: Some("To read"), fail: false, checkpoint: "apple-on-device" })));
    core.flush_filing();
    assert_eq!(core.state(format!("message/{message}/label/{label}/decided_by")).as_deref(), Some("\"apple\""), "asked again and filed by Apple's model");
    assert_eq!(std::fs::read_to_string(dir.path().join("ai-label-judge")).unwrap().trim(), "apple");
}

#[test]
fn a_store_with_a_judge_mark_keeps_its_kept_no_at_a_start() {
    let (dir, path, label, message) = owners_store(Some("laya"));
    let core = RustCore::with_parts(Store::open(&path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    assert_eq!(core.store().ai_no_messages(&label).unwrap(), vec![message], "with a judge mark the kept no stays");
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert!(!log.contains("no judge is marked yet"), "{log}");
}

// ---------------------------------------------------------------- round 11 (owner 2026-11-12 23:44, "Why here doesn't": one mail Apple's model refused
// left 206 waiting): a refusal of ONE mail sets it aside; the judging runs to its end after a Save
// Broken copies (round 11):
// - S1 engine/src/file_arrivals.rs `ai_step`: `Some(reason) => AiStep::SetAside(reason),` -> `Some(reason) => AiStep::Refused(reason),` -> red:
//   `a_mail_the_judge_refuses_for_itself_is_set_aside_once_and_the_others_are_judged` (mail 3 waits and is asked again).
// - S2 engine/src/filing_worker.rs `retry`: `if progressed && !next.is_empty() {` -> `if false {` -> red:
//   `after_a_save_that_picks_120_mails_every_one_is_judged_with_no_sync` (only the first batch of 50 is judged).

/// The judge as the shell's router answers: a refusal of mail `refuse` (by subject) for its content, "none" for every other mail.
struct RefusingJudge { refuse: Option<&'static str>, down: bool, asked: Mutex<HashMap<String, usize>> }
impl DecisionPort for RefusingJudge {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let subject = state.fields.get("subject").cloned().unwrap_or_default();
        *self.asked.lock().unwrap().entry(subject.clone()).or_default() += 1;
        if self.down {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Apple Intelligence".into(), reason_words: "Apple Intelligence could not answer: the model is not ready".into(), retryable: true, code: None } });
        }
        if Some(subject.as_str()) == self.refuse {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Apple Intelligence".into(), reason_words: "unsafe".into(), retryable: false, code: Some(::decision::decide::MAIL_REFUSED_CODE.into()) } });
        }
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, None, 0.0).map(|a| match a {
            DecisionAnswer::Noul { id, .. } => DecisionAnswer::Noul { id, yes: false, probability: 0.0 },
            other => other,
        })).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Apple, checkpoint: Some("apple-on-device".into()) })
    }
}
impl RefusingJudge {
    fn asked_about(&self, subject: &str) -> usize { self.asked.lock().unwrap().get(subject).copied().unwrap_or(0) }
}

fn ten_mails(rig: &Rig) -> Vec<String> {
    let mails: Vec<NewMessage> = (1..=10).map(|n| mail(&rig.account, &format!("INBOX:{n}"), &format!("Subject {n}"), 1_000 + n)).collect();
    rig.core.store().insert_messages(mails).unwrap();
    (1..=10).map(|n| format!("{}:INBOX:{n}", rig.account)).collect()
}
fn set_aside(rig: &Rig) -> String { rig.core.state(format!("label/{}/flow/ai_set_aside", rig.label)).unwrap_or_default() }

#[test]
fn a_mail_the_judge_refuses_for_itself_is_set_aside_once_and_the_others_are_judged() {
    let (rig, _) = Rig::new(None);
    let judge = Arc::new(RefusingJudge { refuse: Some("Subject 3"), down: false, asked: Mutex::new(HashMap::new()) });
    rig.core.set_laya_port(Some(judge.clone()));
    let ids = ten_mails(&rig);
    let filed = rig.core.file_stored_messages(&ids).unwrap();
    assert_eq!(filed.iter().filter(|f| f.ai == engine::AiStep::Asked).count(), 9, "nine are judged in the one run: {filed:?}");
    assert!(filed.iter().any(|f| matches!(&f.ai, engine::AiStep::SetAside(r) if r == "unsafe")), "mail 3 is set aside: {filed:?}");
    assert_eq!(set_aside(&rig), "1");
    assert_eq!(waiting(&rig.counts()), "0", "nothing waits: {}", rig.counts());
    assert_eq!(rig.refusal(), "null", "no failure sentence when every other mail was judged");
    let lines: Vec<String> = rig.log().lines().filter(|l| l.contains("mails set aside")).map(str::to_string).collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(&format!("{} 1 (unsafe 1)", rig.label)) && !lines[0].contains("Subject"), "{lines:?}");
    // the label's AI step is saved again: its waiting mails are judged now; mail 3 is not asked again
    rig.core.run(Command::SetLabelCard { label_id: rig.label.clone(), card: FlowCard::Ai, enabled: true }).unwrap();
    rig.core.flush_filing();
    rig.core.flush_filing();
    assert_eq!(judge.asked_about("Subject 3"), 1, "asked once, set aside, never asked again");
}

#[test]
fn when_the_judge_is_down_every_mail_waits_and_the_card_says_why() {
    let (rig, _) = Rig::new(None);
    let judge = Arc::new(RefusingJudge { refuse: None, down: true, asked: Mutex::new(HashMap::new()) });
    rig.core.set_laya_port(Some(judge));
    let ids = ten_mails(&rig);
    rig.core.file_stored_messages(&ids).unwrap();
    assert_eq!(waiting(&rig.counts()), "10", "{}", rig.counts());
    assert_eq!(set_aside(&rig), "0", "a model that is down sets nothing aside");
    assert!(rig.refusal().contains("could not answer"), "the failure sentence shows: {}", rig.refusal());
}

#[test]
fn after_a_save_that_picks_120_mails_every_one_is_judged_with_no_sync() {
    let (rig, _) = Rig::new(None);
    // 120 unread mails stored before the label's AI step is saved (no arrival judged them)
    let mails: Vec<NewMessage> = (1..=120).map(|n| mail(&rig.account, &format!("INBOX:{n}"), &format!("Subject {n}"), 1_000 + n)).collect();
    rig.core.store().insert_messages(mails).unwrap();
    rig.core.set_laya_port(Some(Arc::new(RouterFake { pick: Some("To read"), fail: false, checkpoint: "apple-on-device" })));
    assert_eq!(waiting(&rig.counts()), "120", "{}", rig.counts());
    // the Save: the label's AI step written again
    rig.core.run(Command::SetLabelCard { label_id: rig.label.clone(), card: FlowCard::Ai, enabled: true }).unwrap();
    // no sync runs; the worker takes batch after batch (flush only waits for what is queued at the time, so read until it ends)
    for _ in 0..20 {
        rig.core.flush_filing();
        if waiting(&rig.counts()) == "0" { break; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(waiting(&rig.counts()), "0", "{}", rig.counts());
    assert!(rig.counts().contains("\"sorted_by_ai\":120"), "{}", rig.counts());
}

// ---------------------------------------------------------------- round 12 (main110's --slice-aisort: the label served
// `{"code":"ai_no_model","words":"unsafe"}` after the LAST mail of the run was set aside)
// Broken copy R12: engine/src/decision_model.rs `LayaPort::decide`: the arm `Err(e) if decision::decide::mail_refusal(e).is_some() => {}` removed
// -> red: `a_set_aside_mail_last_in_the_run_never_becomes_the_labels_refusal`.

#[test]
fn a_set_aside_mail_last_in_the_run_never_becomes_the_labels_refusal() {
    let (rig, _) = Rig::new(None);
    rig.core.set_laya_port(Some(Arc::new(RefusingJudge { refuse: Some("Subject 3"), down: false, asked: Mutex::new(HashMap::new()) })));
    let mails: Vec<NewMessage> = (1..=3).map(|n| mail(&rig.account, &format!("INBOX:{n}"), &format!("Subject {n}"), 1_000 + n)).collect();
    rig.core.store().insert_messages(mails).unwrap();
    let ids: Vec<String> = (1..=3).map(|n| format!("{}:INBOX:{n}", rig.account)).collect();
    rig.core.file_stored_messages(&ids).unwrap();
    assert_eq!(set_aside(&rig), "1");
    assert_eq!(rig.refusal(), "null", "the label works: no refusal, and never the reason id as words");
    assert!(!rig.refusal().contains("unsafe"));
}
