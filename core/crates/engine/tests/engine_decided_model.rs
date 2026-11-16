//! Lane decided-model (SPEC A133, board item abda53d1; the owner, 15 Nov: "how about all the different decision models ? We were supposed to
//! have possibility to choose them"): a mail filed by an on-device model other than Laya names that model. The shell's port answers with
//! the checkpoint "model:<catalogue id>" (Swift `LabelReaderPort.modelCheckpointPrefix`); the engine files the mail decided by
//! `DecidedBy::Model { model }`, the store keeps the id (`decided_by 'laya'`, `decided_model '<id>'`), and the state paths serve
//! "model:<id>". A port with Laya's own checkpoint still files as Laya.
//!
//! The model is a fake handed over exactly as the shell hands its port (`RustCore::set_laya_port`). Real store, real engine, no network.
//!
//! Run: cargo test -p engine --test engine_decided_model
//!
//! Broken copy D1: core/crates/engine/src/decision_model.rs `LayaPort::decide`: the line
//! `if let Some(id) = checkpoint.strip_prefix(MODEL_CHECKPOINT_PREFIX) {` -> `if let Some(id) = None::<&str> {` (the prefix not read)
//! -> red: `a_model_answer_files_the_mail_decided_by_that_model`, `a_model_then_laya_names_each_in_turn`,
//!    `a_rerun_judged_by_a_model_files_the_mail_decided_by_that_model`.
//! Broken copy D2: core/crates/engine/src/flow_write.rs `apply`: `f.by = crate::label_ai_failure::named_model(store, f.by.clone());` ->
//! `let _ = (&f, store);` -> red: the same three tests.
//! The filings of every path are named in `flow_write::apply` (lead's ruling, 15 Nov): the arrival path and the re-run path are both tested.
use std::sync::Arc;

use contracts::*;
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const KNOWLINE: &str = "knowline-0.8b";

#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

fn detector() -> Box<dyn engine::Detect> {
    Box::new(autoconfig::Detector::new(detect_fakes::FakeHttp::default(), detect_fakes::FakeDns::default(), detect_fakes::FakeProbe::default()))
}
fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// An inbound, unread INBOX mail (step 1 of "To read", "unread", picks it).
fn mail(account: &str, key: &str) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:{key}"), thread_id: format!("t-{key}"), account_id: account.into(), sender_name: "Pat".into(), from_address: "pat@other.example".into(),
            to: vec![USER.into()], subject: format!("Subject {key}"), preview: format!("Body of {key}"), received_at_ms: 1_000, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![],
            is_outgoing: false,
        },
        remote_id: Some(key.rsplit(':').next().unwrap_or("1").to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

/// The shell's on-device port as the engine sees it: it picks "To read" and names the model that answered by `checkpoint`; the checkpoint
/// "down" is a model that cannot answer yet (the mail waits).
struct Port { checkpoint: &'static str }
impl DecisionPort for Port {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        if self.checkpoint == "down" {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "KnowLine 0.8B".into(), reason_words: "The model is not loaded yet.".into(), retryable: false, code: None } });
        }
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, Some("To read"), 0.95)).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some(self.checkpoint.into()) })
    }
}

struct Rig { core: RustCore, account: String, label: String, _dir: tempfile::TempDir }
impl Rig {
    /// A real core with one AI-on label "To read" (described; step 1 = unread) and the built-in labels' AI cards off.
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
        for id in ["needs-reply", "updates", "promotions", "social", "primary"] {
            let _ = core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false });
        }
        let label = core.store().label_create_full(
            LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: "mail I still have to read".into(), scope: LabelScope::All,
                        rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false },
            1,
        ).unwrap().id;
        Rig { core, account, label, _dir: dir }
    }
    fn judged_by(&self, checkpoint: &'static str) { self.core.set_laya_port(Some(Arc::new(Port { checkpoint }))); }
    /// One mail arrives and runs the labelling order (as the sync's worker does).
    fn arrive(&self, key: &str) -> String {
        let id = format!("{}:{key}", self.account);
        self.core.store().insert_messages(vec![mail(&self.account, key)]).unwrap();
        self.core.file_stored_messages(&[id.clone()]).unwrap();
        id
    }
    /// The decider as the store reads it back.
    fn stored(&self, id: &str) -> Option<DecidedBy> { self.core.store().label_filing(&self.label, id).unwrap().map(|(by, _)| by) }
    /// The decider as the state path serves it.
    fn served(&self, id: &str) -> Option<String> { self.core.state(format!("message/{id}/label/{}/decided_by", self.label)) }
    fn why(&self, id: &str) -> String { self.core.state(format!("message/{id}/why/{}", self.label)).unwrap_or_default() }
}

#[test]
fn a_model_answer_files_the_mail_decided_by_that_model() {
    let rig = Rig::new();
    rig.judged_by("model:knowline-0.8b");
    let id = rig.arrive("INBOX:1");
    assert_eq!(rig.stored(&id), Some(DecidedBy::Model { model: KNOWLINE.into() }), "the store keeps the model's id");
    assert_eq!(rig.served(&id).as_deref(), Some("\"model:knowline-0.8b\""), "the state path names the model");
    assert!(rig.why(&id).contains("\"decided_by\":\"model:knowline-0.8b\""), "the Why here path names it too: {}", rig.why(&id));
}

#[test]
fn a_laya_answer_still_files_as_laya() {
    let rig = Rig::new();
    rig.judged_by("laya-typed-decisions");
    let id = rig.arrive("INBOX:1");
    assert_eq!(rig.stored(&id), Some(DecidedBy::Laya));
    assert_eq!(rig.served(&id).as_deref(), Some("\"laya\""));
}

#[test]
fn a_model_then_laya_names_each_in_turn() {
    let rig = Rig::new();
    rig.judged_by("model:knowline-0.8b");
    let first = rig.arrive("INBOX:1");
    rig.judged_by("laya-typed-decisions");
    let second = rig.arrive("INBOX:2");
    assert_eq!(rig.served(&first).as_deref(), Some("\"model:knowline-0.8b\""), "the first mail keeps the model that filed it");
    assert_eq!(rig.served(&second).as_deref(), Some("\"laya\""), "the owner went back to Laya: the next mail is Laya's");
}

#[test]
fn a_rerun_judged_by_a_model_files_the_mail_decided_by_that_model() {
    let rig = Rig::new();
    rig.judged_by("down");
    let id = rig.arrive("INBOX:1");
    assert_eq!(rig.served(&id), None, "the model could not answer: not filed on arrival");
    rig.judged_by("model:knowline-0.8b");
    rig.core.run(Command::RerunLabel { label_id: rig.label.clone() }).unwrap();
    for _ in 0..40 {
        rig.core.flush_filing();
        if rig.served(&id).is_some() && !rig.core.state(format!("label/{}/rerun/state", rig.label)).unwrap_or_default().contains("running") { break; }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_ne!(rig.core.state(format!("label/{}/rerun/total", rig.label)).as_deref(), Some("0"), "the re-run looked at the mail");
    assert_eq!(rig.served(&id).as_deref(), Some("\"model:knowline-0.8b\""), "the re-run's filing names the model");
    assert_eq!(rig.stored(&id), Some(DecidedBy::Model { model: KNOWLINE.into() }));
}
