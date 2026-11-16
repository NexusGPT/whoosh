//! Lane labels-r5, round 5: the label AI gets its model from the SAVED DECISION MODEL, at the moment it judges (docs/lanes/ai-settings-core.md
//! "The label AI"; lead ruling: the Decisions pane's choice, Jev or Laya, picks the provider, not the Inbox agent). The core is real over a real
//! migrated store and a `MemoryKeychain` (rig namespace, never the owner's Keychain); no real provider, Keychain or mail server.
//!
//! What can and cannot judge yet (decision_model.rs header): Jev needs its key (stored under the saved row's Keychain reference) AND the owner's
//! consent, which no core record carries, so with its key present the AI step is REFUSED in words and nothing is sent; Laya needs a `LocalModel`
//! nothing hands to the core, so it is left unjudged with the refusal. These cases hold exactly that, through `model_for` for real.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out; runner _captures/labels-r5/run-mutants.py):
//! - D1 decision_model.rs `save`: the row is not written -> a_saved_decision_model_is_kept_and_read_back
//! - D2 decision_model.rs `LabelModel::state`: the refusal never shown -> the refusal cases
//! - D3 decision_model.rs `LabelModel::port`: no port is ever given -> jev_with_its_key_is_asked_and_refused_for_want_of_consent_and_nothing_is_sent
//! - D5 decision_model.rs `LabelModel::state`: an AI card that is off shows the refusal too -> an_ai_card_that_is_off_has_no_refusal
//! - D6 filing_worker.rs `Filing::port`: the saved model is not asked -> the Jev cases
//! - D7 decision_model.rs `jev_key_present`: always true -> a_key_removed_means_the_refusal_again
//! - J1 decision_model.rs `consent_given`: always true -> jev_with_key_and_consent_is_asked_...
//! - J2 decision_model.rs `give_consent`: the row is not written -> the_consent_is_kept_read_...
//! - J3 decision_model.rs `save`: the consent check removed -> the_consent_is_kept_read_...
//! - J5 decision_model.rs `StoredConsent::execute`: a stored consent is not read at the call -> jev_with_key_and_consent_...
//! - D9 decision_model.rs `port`: the Local arm never builds the port -> laya_chosen_files_a_message_on_the_device_...
//! - D10 decision_model.rs `LayaPort::decide`: the failure is not remembered -> without_the_weights_the_refusal_says_so_...
//! - D11 decision_model.rs `state_for_laya`: the thread is left in its field -> the_prompt_laya_gets_is_cut_to_its_window_...
//! - D12 decision_model.rs `state_for_laya`: no room computed (the thread is not cut) -> the_prompt_laya_gets_is_cut_to_its_window_...
//! - D8 decision_model.rs: consent given (a request would go to the connection) -> jev_with_its_key_is_asked_and_refused_for_want_of_consent_and_nothing_is_sent
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::ai_settings::{self, AiJob, AI_NO_MODEL_CODE};
use engine::{AiStep, Keyring, RustCore, TestTransport};
use secrets::{DeleteSecret, MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
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
    fn keyring(&self) -> Arc<dyn Keyring> { self.keys.clone() }
    /// The owner saves the decision model (Settings, Decisions).
    fn choose(&self, provider: DecisionProviderKind, keychain: Option<KeychainRef>) {
        // a hosted provider is chosen only after the owner's yes (delta 23): the helper gives it, the cases about consent do it by hand
        if provider == DecisionProviderKind::Jev { self.core.run(Command::GiveDecisionConsent { provider, given: true }).expect("consent is served"); }
        let model = DecisionModel { provider, model: None, keychain, decisions_enabled: vec!["labels".into()], question_words: Default::default(), last_run_ms: None };
        self.core.run(Command::SaveDecisionModel { model }).expect("SaveDecisionModel is served");
    }
    /// A Jev key lands in the secret store (what the pane's key paste will do); its reference is what the saved row names.
    fn store_key(&self) -> KeychainRef {
        StoreSecret::execute(&*self.keys, SecretKind::ProviderCredential, "jev", &Secret::new("FIXTURE-jev-key-1a2b3c")).unwrap()
    }
    fn remove_key(&self, r: &KeychainRef) { DeleteSecret::execute(&*self.keys, r).unwrap(); }
    /// One inbound message arrives and the label flow runs on it.
    fn arrive(&self, n: u32) -> engine::Filed {
        self.core.store().insert_messages(vec![mail(&self.account, &format!("INBOX:{n}"), &format!("t{n}"), "ann@client.example", "Can you answer?", 1_000 + n as i64)]).unwrap();
        self.core.file_stored_messages(&[id_of(&self.account, &format!("INBOX:{n}"))]).unwrap().pop().expect("the message was filed or left")
    }
    fn refusal(&self) -> String { self.core.state(format!("label/{}/flow/ai_refusal", self.label)).expect("the path is served") }
}

const NO_MODEL: &str = "No model is set for sorting mail: choose one in Settings, Decisions.";

fn spec(name: &str, description: &str, words: Option<&str>) -> LabelSpec {
    let conditions = words.map(|w| vec![RuleCondition::SubjectContains { words: w.into() }]).unwrap_or_default();
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules: RuleSet { conditions, match_mode: MatchMode::Any }, pinned: false }
}


#[test]
fn a_saved_decision_model_is_kept_and_read_back() {
    let rig = Rig::new();
    assert_eq!(rig.core.decision_model(), None, "nothing is saved yet");
    let m = DecisionModel { provider: DecisionProviderKind::Local, model: Some("laya".into()), keychain: None, decisions_enabled: vec!["labels".into(), "which-tab".into()],
                            question_words: [("labels".to_string(), "Which label fits \"this\" email?".to_string())].into(), last_run_ms: Some(7) };
    rig.core.run(Command::SaveDecisionModel { model: m.clone() }).expect("SaveDecisionModel is served");
    let mut back = rig.core.decision_model().expect("the saved choice is read back");
    back.decisions_enabled.sort();
    assert_eq!(back, DecisionModel { decisions_enabled: vec!["labels".into(), "which-tab".into()], ..m });
    // Jev needs its key saved first
    let jev = DecisionModel { provider: DecisionProviderKind::Jev, model: None, keychain: None, decisions_enabled: vec![], question_words: Default::default(), last_run_ms: None };
    assert!(matches!(rig.core.run(Command::SaveDecisionModel { model: jev }), Err(CoreError::Invalid { .. })));
}

#[test]
fn with_no_model_the_ai_card_judges_nothing_and_the_refusal_is_readable() {
    let rig = Rig::new();
    let f = rig.arrive(1);
    assert_eq!((f.ai.clone(), f.place.as_str()), (AiStep::NoProvider, "primary"), "no model: the AI step did not run ({f:?})");
    let r = rig.refusal();
    assert!(r.contains(&format!("\"code\":\"{AI_NO_MODEL_CODE}\"")) && r.contains(NO_MODEL), "{r}");
    // the model-to-use door says the same, through model_for for real
    let e = ai_settings::model_for(&rig.core.store(), &rig.keyring(), AiJob::Labels).err().expect("refused");
    assert!(matches!(&e, CoreError::Action { failure } if failure.code.as_deref() == Some(AI_NO_MODEL_CODE)) && e.to_string().contains(NO_MODEL), "{e}");
    // rules only, and Laya (no runtime reaches the core): still unjudged, same words
    rig.choose(DecisionProviderKind::Rules, None);
    assert_eq!(rig.arrive(2).ai, AiStep::NoProvider);
    rig.choose(DecisionProviderKind::Local, None);
    assert_eq!(rig.arrive(3).ai, AiStep::NoProvider);
    assert!(rig.refusal().contains(NO_MODEL), "{}", rig.refusal());
}

#[test]
fn a_key_saved_later_works_without_a_restart_and_a_key_removed_means_the_refusal_again() {
    let rig = Rig::new();
    // the owner chose Jev but the key is not there yet
    let key = KeychainRef { service: Namespace::rig().reference(SecretKind::ProviderCredential, "jev").unwrap().service, account: "jev".into() };
    rig.choose(DecisionProviderKind::Jev, Some(key.clone()));
    assert_eq!(rig.arrive(1).ai, AiStep::NoProvider, "no key: unjudged");
    assert!(rig.refusal().contains("Jev has no key"), "{}", rig.refusal());
    // the key lands in the secret store: the very next message reaches the Jev step (no restart)
    let stored = rig.store_key();
    assert_eq!(stored, Namespace::rig().reference(SecretKind::ProviderCredential, "jev").unwrap());
    assert!(matches!(rig.arrive(2).ai, AiStep::Refused(_)), "the key is there now");
    // the key is removed: unjudged again
    rig.remove_key(&stored);
    assert_eq!(rig.arrive(3).ai, AiStep::NoProvider);
    assert!(rig.refusal().contains("Jev has no key"), "{}", rig.refusal());
}

#[test]
fn an_ai_card_that_is_off_has_no_refusal() {
    let rig = Rig::new();
    rig.core.run(Command::SetLabelCard { label_id: rig.label.clone(), card: FlowCard::Ai, enabled: false }).unwrap();
    assert_eq!(rig.refusal(), "null", "the label's AI card is off: nothing waits for a model");
    assert_eq!(rig.core.state("label/no-such-label/flow/ai_refusal".into()), None);
}

/// The shell's on-device decision model as the core is handed it: a scripted `DecisionPort` (no real model). It records every state and question it is
/// asked and answers the choice with "Reply soon" at 0.95; `fail` makes it refuse in words (the weights are not on this Mac).
struct ScriptedLaya { seen: std::sync::Mutex<Vec<(DecisionState, Vec<DecisionQuestion>)>>, fail: std::sync::Mutex<Option<String>>, delay_ms: std::sync::atomic::AtomicU64 }
impl ScriptedLaya {
    fn new() -> Arc<ScriptedLaya> { Arc::new(ScriptedLaya { seen: Default::default(), fail: Default::default(), delay_ms: Default::default() }) }
    fn calls(&self) -> usize { self.seen.lock().unwrap().len() }
    fn last_state(&self) -> DecisionState { self.seen.lock().unwrap().last().expect("the model was asked").0.clone() }
}
impl DecisionPort for ScriptedLaya {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.seen.lock().unwrap().push((state, questions.clone()));
        std::thread::sleep(std::time::Duration::from_millis(self.delay_ms.load(std::sync::atomic::Ordering::SeqCst)));
        if let Some(words) = self.fail.lock().unwrap().clone() {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Laya".into(), reason_words: words, retryable: false, code: None } });
        }
        // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
        let answers = questions.iter().map(|q| ::decision::test_answers::labels_answer(q, Some("Reply soon"), 0.95).unwrap_or_else(|| match q {
            DecisionQuestion::Choice { id, .. } => DecisionAnswer::Choice { id: id.clone(), choice: "Reply soon".into(), probabilities: [("Reply soon".to_string(), 0.95)].into() },
            DecisionQuestion::Score { id, .. } => DecisionAnswer::Score { id: id.clone(), value: 0.0 },
            DecisionQuestion::Noul { id, .. } => DecisionAnswer::Noul { id: id.clone(), yes: false, probability: 0.1 },
        })).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some("laya-scripted".into()) })
    }
}

/// A listener that only counts the connections made to it: where a provider's HTTPS would arrive (the Anthropic address is pointed here).
fn counting_server() -> (u16, Arc<std::sync::atomic::AtomicUsize>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = n.clone();
    std::thread::spawn(move || { for s in l.incoming().flatten() { seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst); drop(s); } });
    (port, n)
}

#[test]
fn laya_chosen_files_a_message_on_the_device_and_nothing_goes_to_a_network() {
    let rig = Rig::new();
    // a network route that WOULD be used by a cloud model: a saved key and a provider address, both counted
    let (port, hits) = counting_server();
    ai_settings::set_provider_address(&rig.core.store().path().to_path_buf(), "anthropic", Some(format!("http://127.0.0.1:{port}")));
    let laya = ScriptedLaya::new();
    rig.core.set_laya_port(Some(laya.clone()));
    // no decision model saved yet: the shipped engine is Local, so the shell's Laya judges
    let f = rig.arrive(1);
    assert_eq!((f.place.as_str(), f.card, &f.ai), (rig.label.as_str(), Some(WhyCard::Ai), &AiStep::Asked), "{f:?}");
    // and with Local saved explicitly
    rig.choose(DecisionProviderKind::Local, None);
    let f2 = rig.arrive(2);
    assert_eq!((f2.place.as_str(), &f2.ai), (rig.label.as_str(), &AiStep::Asked), "{f2:?}");
    assert_eq!(laya.calls(), 2, "one question per message, to the on-device model");
    // the filing names Laya as the decider
    let (by, p) = rig.core.store().label_filing(&rig.label, &id_of(&rig.account, "INBOX:1")).unwrap().expect("filed");
    assert_eq!(by, DecidedBy::Laya, "decided_by says Laya");
    assert_eq!(p, Some(0.95));
    assert_eq!(rig.refusal(), "null", "it can sort: no refusal");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 0, "nothing was sent to any network");
}

#[test]
fn the_prompt_laya_gets_is_cut_to_its_window_oldest_thread_messages_first() {
    let rig = Rig::new();
    let laya = ScriptedLaya::new();
    rig.core.set_laya_port(Some(laya.clone()));
    rig.core.store().label_set_ai_reads(rig.label.clone(), AiReads { subject: true, body: true, rest_of_thread: true, attachments: false }, 2).unwrap();
    let pad = "x".repeat(900);
    let mut batch: Vec<NewMessage> = (0..30).map(|i| {
        let mut m = mail(&rig.account, &format!("INBOX:{}", 100 + i), "t1", "ann@client.example", &format!("earlier {i:02} {pad}"), 1_000 + i as i64);
        m.message.read = true;
        m
    }).collect();
    batch.push(mail(&rig.account, "INBOX:1", "t1", "ann@client.example", "Can you answer?", 9_000));
    rig.core.store().insert_messages(batch).unwrap();
    rig.core.file_stored_messages(&[id_of(&rig.account, "INBOX:1")]).unwrap();
    let state = laya.last_state();
    assert!(!state.fields.contains_key(::decision::thread_context::THREAD_FIELD), "the thread rides in the text, not in a field the model would read as JSON");
    let window = ::decision::thread_context::chars_for_tokens(::decision::tuning::on_device_context_tokens());
    let total = state.text.chars().count() + state.fields.values().map(|v| v.chars().count()).sum::<usize>();
    assert!(total <= window, "the state is {total} characters, the on-device window {window}");
    assert!(state.text.starts_with("Body of Can you answer?"), "the body comes first (the shell's cut keeps the start): {:?}", &state.text.chars().take(60).collect::<String>());
    assert!(state.text.contains(::decision::thread_context::THREAD_HEAD) && state.text.contains("earlier 29"), "the newest earlier message is read");
    assert!(!state.text.contains("earlier 00"), "the oldest is cut first");
}

#[test]
fn without_the_weights_the_refusal_says_so_and_a_judgement_that_works_clears_it() {
    let rig = Rig::new();
    let laya = ScriptedLaya::new();
    let words = "The on-device decision model is not on this Mac yet: download it in Settings, Decisions.";
    *laya.fail.lock().unwrap() = Some(words.into());
    rig.core.set_laya_port(Some(laya.clone()));
    let f = rig.arrive(1);
    match &f.ai { AiStep::Refused(why) => assert!(why.contains(words), "{why}"), other => panic!("the AI step should be refused: {other:?}") }
    assert_eq!(f.place, "primary", "left unjudged, not sorted into nothing");
    let r = rig.refusal();
    assert!(r.contains(AI_NO_MODEL_CODE) && r.contains(words), "{r}");
    // the weights arrive: the next judgement works and the sentence goes
    *laya.fail.lock().unwrap() = None;
    assert_eq!(rig.arrive(2).ai, AiStep::Asked);
    assert_eq!(rig.refusal(), "null");
    // the shell takes the model back (or never handed one): the refusal is the general one
    rig.core.set_laya_port(None);
    assert_eq!(rig.arrive(3).ai, AiStep::NoProvider);
    assert!(rig.refusal().contains(NO_MODEL), "{}", rig.refusal());
}

fn rerun_state(rig: &Rig, leaf: &str) -> String { rig.core.state(format!("label/{}/rerun/{leaf}", rig.label)).unwrap_or_default() }

#[test]
fn a_saved_description_then_a_rerun_sorts_the_mail_already_there_on_the_device_and_never_blocks() {
    let rig = Rig::new();
    // seven inbound messages, unsorted (no model yet), then the owner saves a description and the shell's Laya is there
    for n in 1..=7 { rig.arrive(n); }
    let laya = ScriptedLaya::new();
    laya.delay_ms.store(100, std::sync::atomic::Ordering::SeqCst);      // a real model takes its time: 7 x 100 ms must not be waited for
    rig.core.set_laya_port(Some(laya.clone()));
    assert_eq!(rerun_state(&rig, "candidates"), "7", "the Save offer's number: every inbound message the model would judge");
    // the command answers at once, whatever the model takes; the result is read from state afterwards
    let started = std::time::Instant::now();
    rig.core.run(Command::RerunLabel { label_id: rig.label.clone() }).expect("served");
    assert!(started.elapsed() < std::time::Duration::from_millis(50), "the command returned in {:?} while the model takes 700 ms in all", started.elapsed());
    rig.core.flush_filing();
    assert_eq!((rerun_state(&rig, "state").as_str(), rerun_state(&rig, "total").as_str(), rerun_state(&rig, "done").as_str()), ("\"done\"", "7", "7"));
    assert_eq!((rerun_state(&rig, "refiled").as_str(), rerun_state(&rig, "filed").as_str(), rerun_state(&rig, "model_pending").as_str()), ("7", "7", "false"));
    assert_eq!(laya.calls(), 7, "one question per message, on the device");
    for n in 1..=7 {
        let (by, _) = rig.core.store().label_filing(&rig.label, &id_of(&rig.account, &format!("INBOX:{n}"))).unwrap().expect("filed by the re-run");
        assert_eq!(by, DecidedBy::Laya);
    }
}

#[test]
fn a_rerun_without_the_weights_keeps_every_filing_and_says_pending() {
    let rig = Rig::new();
    for n in 1..=3 { rig.arrive(n); }
    let laya = ScriptedLaya::new();
    *laya.fail.lock().unwrap() = Some("The on-device decision model is not on this Mac yet: download it in Settings, Decisions.".into());
    rig.core.set_laya_port(Some(laya.clone()));
    rig.core.run(Command::RerunLabel { label_id: rig.label.clone() }).unwrap();
    rig.core.flush_filing();
    assert_eq!((rerun_state(&rig, "model_pending").as_str(), rerun_state(&rig, "filed").as_str(), rerun_state(&rig, "refiled").as_str()), ("true", "0", "0"));
    assert!(rig.refusal().contains("not on this Mac yet"), "{}", rig.refusal());
}

use ai_providers::transport::{Body, Request, Response, Transport};

/// The hosted provider as a fake: it records every request and answers the questions it is asked with "Reply soon" at 0.95.
struct FakeJev { seen: std::sync::Mutex<Vec<Request>> }
impl FakeJev { fn new() -> Arc<FakeJev> { Arc::new(FakeJev { seen: Default::default() }) } fn calls(&self) -> usize { self.seen.lock().unwrap().len() } }
impl Transport for FakeJev {
    fn send(&self, request: &Request) -> Result<Response, String> {
        self.seen.lock().unwrap().push(request.clone());
        let Body::Json(body) = &request.body else { return Err("no body".into()) };
        let v: serde_json::Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
        let mut answers = serde_json::Map::new();
        for (id, q) in v["questions"].as_object().cloned().unwrap_or_default() {
            let pick = q["criteria"].as_object().and_then(|o| o.keys().find(|k| k.as_str() == "Reply soon").cloned()).unwrap_or_default();
            // fix-aisort round 3: a per-label yes/no question gets the yes-probability of the label it names
            let noul = ::decision::decide::statement_label(q["instructions"].as_str().unwrap_or("")).map_or(0.1, |(label, _)| ::decision::test_answers::yes_for(label, Some("Reply soon"), 0.95));
            answers.insert(id, serde_json::json!({"choice": pick, "confidence": 0.95, "noul": noul}));
        }
        Ok(Response { status: 200, body: serde_json::json!({"answers": answers, "model": "jev-fake"}).to_string() })
    }
    // the request's state text, as the provider reads it
}
fn state_of(r: &Request) -> String {
    let Body::Json(body) = &r.body else { panic!("json body") };
    serde_json::from_str::<serde_json::Value>(body).unwrap()["state"].as_str().unwrap().to_string()
}

fn consent(rig: &Rig, given: bool) -> Result<CommandOutcome, CoreError> { rig.core.run(Command::GiveDecisionConsent { provider: DecisionProviderKind::Jev, given }) }

#[test]
fn the_consent_is_kept_read_and_refused_where_none_is_needed() {
    let rig = Rig::new();
    assert_eq!(rig.core.state("decision_consent/jev".into()).as_deref(), Some("{\"needed\":true,\"given\":false}"));
    assert!(!rig.core.decision_consent(DecisionProviderKind::Jev));
    consent(&rig, true).unwrap();
    assert_eq!(rig.core.state("decision_consent/jev".into()).as_deref(), Some("{\"needed\":true,\"given\":true}"));
    assert!(rig.core.decision_consent(DecisionProviderKind::Jev));
    consent(&rig, false).unwrap();
    assert!(!rig.core.decision_consent(DecisionProviderKind::Jev), "withdrawing deletes it");
    // Laya and Rules send nothing off the Mac: a consent for them is refused in words
    for p in [DecisionProviderKind::Local, DecisionProviderKind::Rules] {
        match rig.core.run(Command::GiveDecisionConsent { provider: p, given: true }) {
            Err(CoreError::Invalid { field, .. }) => assert_eq!(field, "decision_consent_not_needed"),
            other => panic!("{other:?}"),
        }
        assert_eq!(rig.core.state(format!("decision_consent/{}", if p == DecisionProviderKind::Local { "local" } else { "rules" })).as_deref(), Some("{\"needed\":false,\"given\":false}"));
    }
    // choosing Jev with no consent is refused with the contract's words; with the key but no yes nothing is saved
    let key = rig.store_key();
    let jev = DecisionModel { provider: DecisionProviderKind::Jev, model: None, keychain: Some(key), decisions_enabled: vec![], question_words: Default::default(), last_run_ms: None };
    match rig.core.run(Command::SaveDecisionModel { model: jev.clone() }) {
        Err(CoreError::Invalid { field, reason_words }) => { assert_eq!(field, "decision_consent_needed"); assert!(reason_words.contains("Allow the text of the mail your labels read"), "{reason_words}"); }
        other => panic!("{other:?}"),
    }
    assert_eq!(rig.core.decision_model(), None);
}

#[test]
fn jev_with_key_and_consent_is_asked_with_exactly_the_ticked_fields_and_never_before_all_three() {
    let rig = Rig::new();
    let jev = FakeJev::new();
    rig.core.set_decision_transport(jev.clone());
    // key alone, no model chosen, no consent: nothing goes out
    let key = rig.store_key();
    assert_eq!(rig.arrive(1).ai, AiStep::NoProvider);
    assert_eq!(jev.calls(), 0);
    // model chosen but the yes withdrawn after choosing: the judgement is refused and NOTHING is sent
    rig.choose(DecisionProviderKind::Jev, Some(key.clone()));
    consent(&rig, false).unwrap();
    match rig.arrive(2).ai { AiStep::Refused(why) => assert!(why.contains("not agreed yet") && why.contains("nothing was sent"), "{why}"), other => panic!("{other:?}") }
    assert_eq!(jev.calls(), 0, "no consent, no request");
    assert!(rig.refusal().contains("agree"), "{}", rig.refusal());
    // consent given: the very next message goes out, with the fields the card ticks (subject and body), no attachment text
    consent(&rig, true).unwrap();
    let f = rig.arrive(3);
    assert_eq!((f.place.as_str(), &f.ai), (rig.label.as_str(), &AiStep::Asked), "{f:?}");
    assert_eq!(jev.calls(), 1);
    let sent = jev.seen.lock().unwrap()[0].clone();
    assert_eq!(sent.url, ::decision::adapters::jev::JEV_URL);
    assert!(sent.headers.iter().any(|(k, v)| k == "authorization" && v.starts_with("Bearer ")), "the key travels as a bearer");
    let state = state_of(&sent);
    assert!(state.contains("subject: Can you answer?") && state.contains("Body of Can you answer?"), "subject and body are ticked: {state}");
    assert!(!state.to_lowercase().contains("attachment"), "attachments are not ticked: {state}");
    // untick the body: the next request carries the subject and no body text
    // one call judges every label with its AI card on, so what is sent is the union of their ticks (decision-r2 gap 2): every described label unticks the body
    for id in [rig.label.as_str(), "needs-reply", "updates", "promotions", "social"] {
        rig.core.store().label_set_ai_reads(id.to_string(), AiReads { subject: true, body: false, rest_of_thread: false, attachments: false }, 9).unwrap();
    }
    rig.arrive(4);
    let state = state_of(&jev.seen.lock().unwrap()[1]);
    assert!(state.contains("subject: Can you answer?") && !state.contains("Body of Can you answer?"), "{state}");
    // withdrawn again: refused, nothing more is sent
    consent(&rig, false).unwrap();
    assert!(matches!(rig.arrive(5).ai, AiStep::Refused(_)));
    assert_eq!(jev.calls(), 2, "no request after the withdrawal");
}
