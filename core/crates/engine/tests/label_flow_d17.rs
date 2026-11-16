//! The real core runs the labelling order on every message the sync stores (delta 17, docs/PRD.md "A94 pick"; docs/lanes/engine-decision.md):
//! entry rules of every enabled label first, then the model over the enabled labels, then the chosen label's exit filter with its fallback;
//! a flipped fact re-evaluates the exit filter of the message's current label at once; no provider = the rules and the exit filters still run.
//! The model here is a fake DecisionPort (the engine's own provider port); the stores are real, the sync runs against sync-imap's fake.
//!
//! Mutants (single edits; each was run and seen red, the verbatim results are in _captures/engine-decision/mutants.out; the runner is
//! _captures/engine-decision/run-mutants.py):
//! - M1 flow_ctx.rs `FlowCtx::open`: `kind` -> `DecisionProviderKind::Rules` (the model is never asked)
//! - M2 file_arrivals.rs `placement`: `file: filing_of(v, fl)` -> `file: None` (nothing is written)
//! - M3 file_arrivals.rs `ai_step`: the `NoProvider` arm answers `Asked`
//! - M4 refile_flipped.rs: the flip's verdict is always Keep
//! - M5 flow_write.rs `owner_filed`: -> `Ok(false)`
//! - M6 filing_worker.rs `arrived`: drop `self.flip(&mates)`
//! - M7 imap_source.rs `page`: the sync no longer names the messages it is about to store
//! - M8 flow_ctx.rs `FlowCtx::decide`: `engine: self.port.as_deref()` -> `engine: None`
//! - M9 sync.rs `apply_changes`, the change-set branch (CONDSTORE/QRESYNC): the sync no longer names the messages it stores for IDLE mail
//! - M9b sync.rs `apply_changes`, the newest-page branch (a server without CONDSTORE): the same
//! - M12 sync.rs `apply_changes`: the flag changes it stores are no longer handed to the flow (`d.filing.flipped(flipped)`)
//! - M10 flow_write.rs `apply`: the rejection row is never written
//! - M11 file_arrivals.rs `run`: the INBOX-only filter -> every folder (a Sent message would be filed)
//!
//! A140 (owner 2026-11-11, the three-step pipeline): a described label with its AI card on is AI-on, so its entry chain picks what the
//! model is asked about instead of filing; only an AI-off label files by its entry chain. The entry-rule cases below switch the AI card
//! of the label they file into off (`ai_off`), each with a one-line A140 comment; the `a140_` cases at the end are this lane's.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{AiStep, RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

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

fn label(s: &Store, name: &str, description: &str, rules: Vec<RuleCondition>) -> MailLabel {
    s.label_create_full(
        LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules: RuleSet { conditions: rules, match_mode: MatchMode::Any }, pinned: false },
        1,
    ).unwrap()
}
fn unanswered_yes() -> RuleSet { RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }], match_mode: MatchMode::All } }

/// The three labels of the cases. Contracts: entry rule "subject contains contract". Needs reply: no entry rule, exit filter on with the
/// chain "unanswered", fallback Primary. Hidden: described, its AI card off (the model is never offered it).
struct Labels { contracts: String, needs_reply: String, hidden: String }
fn labels(s: &Store) -> Labels {
    let contracts = label(s, "Contracts", "mail about contracts and renewals", vec![RuleCondition::SubjectContains { words: "contract".into() }]);
    let needs_reply = label(s, "Needs reply", "emails waiting for my answer", vec![]);
    let hidden = label(s, "Hidden", "never offered to the model", vec![]);
    s.label_chain_add(needs_reply.id.clone(), ChainPlacement::Exit, unanswered_yes(), 2).unwrap();
    s.label_set_exit_fallback(needs_reply.id.clone(), Some("primary".into()), 3).unwrap();
    s.label_set_card(needs_reply.id.clone(), FlowCard::Exit, true, 4).unwrap();
    s.label_set_card(hidden.id.clone(), FlowCard::Ai, false, 5).unwrap();
    Labels { contracts: contracts.id, needs_reply: needs_reply.id, hidden: hidden.id }
}

/// The model: asked about a subject, answers the label named in `picks` for it (0.9), or nothing it offered.
struct FakeAi { picks: HashMap<String, String>, calls: Mutex<Vec<(DecisionState, Vec<DecisionQuestion>)>> }
impl FakeAi {
    fn new(picks: &[(&str, &str)]) -> Arc<FakeAi> { Arc::new(FakeAi { picks: picks.iter().map(|(s, l)| (s.to_string(), l.to_string())).collect(), calls: Mutex::new(vec![]) }) }
    fn asked(&self) -> usize { self.calls.lock().unwrap().len() }
}
impl DecisionPort for FakeAi {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.calls.lock().unwrap().push((state.clone(), questions.clone()));
        let subject = state.fields.get("subject").cloned().unwrap_or_default();
        // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, self.picks.get(&subject).map(String::as_str), 0.9)).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

/// A core over a store holding the account and the three labels.
fn setup() -> (RustCore, String, Labels, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let l = labels(&store);
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, l, dir)
}
fn why(core: &RustCore, id: &str, place: &str) -> Option<String> { core.state(format!("message/{id}/why/{place}")) }
/// A140: switch a label's AI card off (its entry chain then files by itself).
fn ai_off(core: &RustCore, label_id: &str) { core.run(Command::SetLabelCard { label_id: label_id.into(), card: FlowCard::Ai, enabled: false }).unwrap(); }

// ---------------------------------------------------------------- the entry rule

#[test]
fn an_arriving_message_is_caught_by_an_entry_rule_and_the_model_is_never_asked() {
    let (core, a, l, _d) = setup();
    ai_off(&core, &l.contracts); // A140: only an AI-off label's entry rule files by itself
    let ai = FakeAi::new(&[("Contract renewal", "Needs reply")]);
    core.set_decision_port(Some(ai.clone()));
    core.store().insert_messages(vec![mail(&a, "INBOX:1", "t1", "boss@client.example", "Contract renewal", 1_000)]).unwrap();
    let filed = core.file_stored_messages(&[id_of(&a, "INBOX:1")]).unwrap();
    assert_eq!(filed.len(), 1);
    let f = &filed[0];
    assert_eq!((f.place.as_str(), f.card, f.chain_id, &f.ai), (l.contracts.as_str(), Some(WhyCard::Entry), Some(0), &AiStep::EntryRule), "{f:?}");
    assert_eq!(ai.asked(), 0, "a sure entry match stops the walk before the model");
    let w = why(&core, &id_of(&a, "INBOX:1"), &l.contracts).expect("the filing is in the store");
    assert!(w.contains("\"card\":\"entry\"") && w.contains("\"chain_id\":0") && w.contains("\"decided_by\":\"rule:"), "{w}");
    // A140: the counts carry `waiting_for_ai` (0: the AI is off for Contracts)
    assert_eq!(core.state(format!("label/{}/flow/counts", l.contracts)).unwrap(), "{\"caught_by_entry\":1,\"kept_by_exit\":0,\"sent_to_fallback\":0,\"sorted_by_ai\":0,\"waiting_for_ai\":0}");
}

// ---------------------------------------------------------------- the model, kept

#[test]
fn sorted_by_a_fake_ai_over_the_enabled_labels_and_kept_by_the_exit_filter() {
    let (core, a, l, _d) = setup();
    let ai = FakeAi::new(&[("Can you confirm?", "Needs reply")]);
    core.set_decision_port(Some(ai.clone()));
    let id = id_of(&a, "INBOX:2");
    core.store().insert_messages(vec![mail(&a, "INBOX:2", "t2", "pat@other.example", "Can you confirm?", 2_000)]).unwrap();
    let filed = core.file_stored_messages(&[id.clone()]).unwrap();
    let f = &filed[0];
    assert_eq!((f.place.as_str(), f.card, f.chain_id, &f.ai), (l.needs_reply.as_str(), Some(WhyCard::Exit), Some(0), &AiStep::Asked), "unanswered = the newest of its thread, inbound: the exit chain keeps it: {f:?}");
    assert!(f.decided_by.contains("the model said Needs reply (0.90)"), "{}", f.decided_by);
    assert!(why(&core, &id_of(&a, "INBOX:2"), &l.hidden).is_none(), "the label with its AI card off was never offered");
    // the model read the enabled labels' descriptions and was offered only the labels whose AI card is on
    let calls = ai.calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "one engine call per message");
    // fix-aisort round 3: the Labels question in either form
    assert!(::decision::test_answers::asks_labels(&calls[0].1), "the Labels question was asked: {:?}", calls[0].1);
    let (options, prompt) = (::decision::test_answers::offered_labels(&calls[0].1), ::decision::test_answers::label_meanings(&calls[0].1).join("\n"));
    // the built-in categories (Updates, Promotions...) are labels with their AI card on, so they are offered too: the decision crate's rule
    // A140: Contracts' step 1 (subject "contract") did not pick this message, so it is not offered; Needs reply's step 1 is open
    assert!(!options.contains(&"Contracts".to_string()) && options.contains(&"Needs reply".to_string()), "{options:?}");
    assert!(!options.contains(&"Hidden".to_string()), "Hidden has its AI card off: {options:?}");
    assert!(prompt.contains("Needs reply: emails waiting for my answer") && !prompt.contains("never offered"), "{prompt}");
    assert_eq!(calls[0].0.fields.get("subject").map(String::as_str), Some("Can you confirm?"), "the ticked input reaches the model");
    let w = why(&core, &id, &l.needs_reply).expect("filed");
    assert!(w.contains("\"card\":\"exit\"") && w.contains("\"decided_by\":\"laya\""), "{w}");
    assert!(core.state(format!("message/{id}/flow_rejection")).is_none(), "kept: no rejection row");
}

// ---------------------------------------------------------------- the model, rejected

#[test]
fn rejected_by_the_exit_filter_to_the_fallback_primary_with_its_rejection_row() {
    let (core, a, l, _d) = setup();
    core.set_decision_port(Some(FakeAi::new(&[("Are you free?", "Needs reply")])));
    // the owner already answered this thread: the message is no longer unanswered
    core.store().insert_messages(vec![
        mail(&a, "INBOX:3", "t3", "pat@other.example", "Are you free?", 3_000),
        mail(&a, "Sent:1", "t3", USER, "Re: Are you free?", 4_000),
    ]).unwrap();
    let id = id_of(&a, "INBOX:3");
    let f = core.file_stored_messages(&[id.clone()]).unwrap().remove(0);
    assert_eq!((f.place.as_str(), f.card, &f.ai), ("primary", Some(WhyCard::Fallback), &AiStep::Asked), "{f:?}");
    assert!(why(&core, &id, &l.needs_reply).is_none(), "the AI's pick is not filed");
    let w = why(&core, &id, "primary").expect("why here in Primary: the exit filter's fallback");
    assert!(w.contains("\"card\":\"fallback\""), "{w}");
    let r = core.state(format!("message/{id}/flow_rejection")).expect("the rejection row");
    assert!(r.contains(&format!("\"ai_label_id\":\"{}\"", l.needs_reply)) && r.contains("\"sent_to\":\"primary\""), "{r}");
    // A140: the counts carry `waiting_for_ai` (Needs reply has no entry chain: nothing is picked, nothing waits)
    assert_eq!(core.state(format!("label/{}/flow/counts", l.needs_reply)).unwrap(), "{\"caught_by_entry\":0,\"kept_by_exit\":0,\"sent_to_fallback\":1,\"sorted_by_ai\":1,\"waiting_for_ai\":0}");
}

#[test]
fn rejected_to_a_label_files_it_there_by_the_fallback_card_and_applies_no_second_filter() {
    let (core, a, l, _d) = setup();
    let later = label(&core.store(), "Later", "things for later", vec![]);
    core.run(Command::SetLabelExitFallback { label_id: l.needs_reply.clone(), fallback: Some(later.id.clone()) }).unwrap();
    core.set_decision_port(Some(FakeAi::new(&[("Are you free?", "Needs reply")])));
    core.store().insert_messages(vec![mail(&a, "INBOX:4", "t4", "pat@other.example", "Are you free?", 3_000), mail(&a, "Sent:2", "t4", USER, "Re: Are you free?", 4_000)]).unwrap();
    let id = id_of(&a, "INBOX:4");
    let f = core.file_stored_messages(&[id.clone()]).unwrap().remove(0);
    assert_eq!((f.place.as_str(), f.card), (later.id.as_str(), Some(WhyCard::Fallback)), "{f:?}");
    let w = why(&core, &id, &later.id).expect("filed under the fallback label");
    assert!(w.contains("\"card\":\"fallback\"") && w.contains("\"decided_by\":\"laya\""), "the model chose, the exit card moved it: {w}");
    assert!(core.state(format!("message/{id}/flow_rejection")).unwrap().contains(&format!("\"sent_to\":\"{}\"", later.id)));
}

// ---------------------------------------------------------------- a flipped fact

#[test]
fn a_reply_flip_moves_a_message_out_at_once_and_never_asks_the_model() {
    let (core, a, l, _d) = setup();
    let ai = FakeAi::new(&[("Can you confirm?", "Needs reply")]);
    core.set_decision_port(Some(ai.clone()));
    let id = id_of(&a, "INBOX:5");
    core.store().insert_messages(vec![mail(&a, "INBOX:5", "t5", "pat@other.example", "Can you confirm?", 5_000)]).unwrap();
    assert_eq!(core.file_stored_messages(&[id.clone()]).unwrap()[0].place, l.needs_reply, "filed and kept: it is unanswered");
    assert_eq!(ai.asked(), 1);
    // the owner replies: the reply is stored, the fact "unanswered" flips, the exit filter rejects, the message goes to Primary
    core.store().insert_messages(vec![mail(&a, "Sent:3", "t5", USER, "Re: Can you confirm?", 6_000)]).unwrap();
    let moved = core.refile_flipped(&[id.clone()]).unwrap();
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!((moved[0].from.as_str(), moved[0].to.as_str(), moved[0].card), (l.needs_reply.as_str(), "primary", WhyCard::Fallback));
    assert!(why(&core, &id, &l.needs_reply).is_none(), "off its label");
    assert!(core.state(format!("message/{id}/flow_rejection")).unwrap().contains("\"sent_to\":\"primary\""));
    assert_eq!(ai.asked(), 1, "a flip re-asks the cards, never the model");
    assert!(core.refile_flipped(&[id]).unwrap().is_empty(), "a second pass on the same facts moves nothing");
}

#[test]
fn a_reply_stored_beside_the_sync_flips_the_older_message_without_being_named() {
    let (core, a, l, _d) = setup();
    // no provider: the exit filter still runs. The message is filed by the AI (set, then removed), the reply flips it.
    core.set_decision_port(Some(FakeAi::new(&[("Can you confirm?", "Needs reply")])));
    let id = id_of(&a, "INBOX:6");
    core.store().insert_messages(vec![mail(&a, "INBOX:6", "t6", "pat@other.example", "Can you confirm?", 7_000)]).unwrap();
    core.note_stored(vec![id.clone()]);
    core.flush_filing();
    assert!(why(&core, &id, &l.needs_reply).is_some(), "the worker filed the new message by itself");
    core.set_decision_port(None);
    let reply = mail(&a, "Sent:4", "t6", USER, "Re: Can you confirm?", 8_000);
    let reply_id = reply.message.id.clone();
    core.store().insert_messages(vec![reply]).unwrap();
    core.note_stored(vec![reply_id]);
    core.flush_filing();
    assert!(why(&core, &id, &l.needs_reply).is_none(), "the older message left its label by itself, with no provider set");
    assert!(core.state(format!("message/{id}/flow_rejection")).is_some());
}

// ---------------------------------------------------------------- no provider

#[test]
fn no_provider_runs_the_rules_and_says_so() {
    let (core, a, l, _d) = setup();
    ai_off(&core, &l.contracts); // A140: only an AI-off label's entry rule files by itself
    core.store().insert_messages(vec![
        mail(&a, "INBOX:7", "t7", "boss@client.example", "Contract renewal", 1_000),
        mail(&a, "INBOX:8", "t8", "pat@other.example", "Can you confirm?", 2_000),
    ]).unwrap();
    let filed = core.file_stored_messages(&[id_of(&a, "INBOX:7"), id_of(&a, "INBOX:8")]).unwrap();
    let rule = filed.iter().find(|f| f.message_id.ends_with("INBOX:7")).unwrap();
    assert_eq!((rule.place.as_str(), &rule.ai), (l.contracts.as_str(), &AiStep::EntryRule), "the entry rules still run");
    let rest = filed.iter().find(|f| f.message_id.ends_with("INBOX:8")).unwrap();
    assert_eq!((rest.place.as_str(), rest.card, &rest.ai), ("primary", None, &AiStep::NoProvider), "{rest:?}");
    assert!(rest.decided_by.contains("no AI provider is set") && rest.decided_by.contains("otherwise"), "decided_by says so: {}", rest.decided_by);
    let st = core.state(format!("message/{}/filing", id_of(&a, "INBOX:8"))).unwrap();
    assert!(st.contains("\"ai\":\"no_provider\"") && st.contains("\"decided_by\":\"default\"") && st.contains("\"place\":\"primary\""), "{st}");
    assert!(core.state(format!("message/{}/filing", id_of(&a, "INBOX:7"))).unwrap().contains("\"ai\":\"not_reached\""));
}

// ---------------------------------------------------------------- the owner's hand

#[test]
fn the_owners_own_filing_is_never_overwritten_by_a_rule() {
    let (core, a, l, _d) = setup();
    ai_off(&core, &l.contracts); // A140: only an AI-off label's entry rule files by itself
    let id = id_of(&a, "INBOX:9");
    core.store().insert_messages(vec![mail(&a, "INBOX:9", "t9", "boss@client.example", "Contract renewal", 1_000)]).unwrap();
    core.store().label_file(l.contracts.clone(), id.clone(), DecidedBy::Owner, None, 5).unwrap();
    core.file_stored_messages(&[id.clone()]).unwrap();
    let (by, _) = core.store().label_filing(&l.contracts, &id).unwrap().expect("still filed");
    assert_eq!(by, DecidedBy::Owner, "the entry rule matched, the owner's decision stays");
}

// ---------------------------------------------------------------- through the real sync

#[test]
fn a_message_the_sync_stores_runs_the_labelling_order() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    // the labels exist BEFORE the account syncs: "Message 2" is caught by an entry rule
    let caught = label(&store, "Second", "the second message", vec![RuleCondition::SubjectContains { words: "Message 2".into() }]);
    store.label_set_card(caught.id.clone(), FlowCard::Ai, false, 2).unwrap(); // A140: only an AI-off label's entry rule files by itself
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let account = out.created_id.unwrap();
    wait_until("three inbox rows", || core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).len() == 3);
    wait_until("history fetched", || core.sync_states().iter().any(|s| s.account_id == account && s.history_fetched));
    core.flush_filing();
    let rows = core.inbox(InboxQuery { tab_id: "all".into(), account_id: None, label_id: Some(caught.id.clone()), mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None });
    assert_eq!(rows.iter().map(|r| r.subject.as_str()).collect::<Vec<_>>(), vec!["Message 2"], "the sync's own page ran the order: {rows:?}");
    let id = rows[0].message_id.clone();
    assert!(why(&core, &id, &caught.id).unwrap().contains("\"card\":\"entry\""));
    let other = core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).into_iter().find(|r| r.subject == "Message 1").unwrap();
    assert!(core.state(format!("message/{}/filing", other.message_id)).unwrap().contains("\"ai\":\"no_provider\""), "no provider is set in this run: the others stay in Primary and say why");
}

fn under(core: &RustCore, label_id: &str) -> Vec<InboxRow> {
    core.inbox(InboxQuery { tab_id: "all".into(), account_id: None, label_id: Some(label_id.into()), mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None })
}

/// A synced account whose labels exist before it syncs; `condstore` false = a server with no CONDSTORE or QRESYNC (the sync then
/// re-reads the newest page on a change: the other branch of `apply_changes`).
fn idle_case(condstore: bool) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    imap.with(|s| { s.qresync = condstore; s.cap_condstore = condstore; });
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let fourth = label(&store, "Fourth", "the fourth message", vec![RuleCondition::SubjectContains { words: "Message 4".into() }]);
    store.label_set_card(fourth.id.clone(), FlowCard::Ai, false, 2).unwrap(); // A140: only an AI-off label's entry rule files by itself
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(engine::SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    // the fake counts an IDLE as started before it waits for a change, and drops a change announced in between: let it settle
    std::thread::sleep(Duration::from_millis(300));
    imap.add_message("INBOX");
    wait_until("the message that IDLE brought is filed by its entry rule", || { core.flush_filing(); under(&core, &fourth.id).len() == 1 });
    assert_eq!(under(&core, &fourth.id)[0].subject, "Message 4");
}

#[test]
fn new_mail_that_arrives_by_idle_runs_the_labelling_order_too() {
    idle_case(true);
}

#[test]
fn new_mail_from_a_server_without_condstore_runs_the_labelling_order_too() {
    idle_case(false);
}

#[test]
fn a_read_stored_by_the_sync_moves_the_message_out_of_a_label_whose_exit_filter_wants_unread() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    // "Unread mail": the model files into it, the exit filter keeps only what is still unread, the rest goes to Primary
    let unread = label(&store, "Unread mail", "mail I have not read yet", vec![]);
    store.label_chain_add(unread.id.clone(), ChainPlacement::Exit, RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::All }, 2).unwrap();
    store.label_set_exit_fallback(unread.id.clone(), Some("primary".into()), 3).unwrap();
    store.label_set_card(unread.id.clone(), FlowCard::Exit, true, 4).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let ai = FakeAi::new(&[("Message 1", "Unread mail"), ("Message 2", "Unread mail"), ("Message 3", "Unread mail")]);
    core.set_decision_port(Some(ai.clone()));
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(engine::SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    std::thread::sleep(Duration::from_millis(300)); // the same settle as idle_case
    wait_until("the three messages are sorted and kept by the exit filter", || { core.flush_filing(); under(&core, &unread.id).len() == 3 });
    let asked = ai.asked();
    assert_eq!(asked, 3, "one engine call per stored message");
    // the server marks the second message read: the sync stores the flip, the exit filter re-evaluates at once, the model is not asked
    let uid = imap.uids("INBOX")[1];
    imap.change_flags("INBOX", &[uid], "\\Seen");
    wait_until("the read message left its label", || { core.flush_filing(); under(&core, &unread.id).len() == 2 });
    assert!(!under(&core, &unread.id).iter().any(|r| r.subject == "Message 2"), "{:?}", under(&core, &unread.id));
    assert_eq!(ai.asked(), asked, "a flipped fact never asks the model");
}

#[test]
fn a_message_in_sent_is_never_filed_by_the_flow() {
    let (core, a, l, _d) = setup();
    core.store().insert_messages(vec![mail(&a, "Sent:9", "t9s", USER, "Contract draft", 1_000)]).unwrap();
    let id = id_of(&a, "Sent:9");
    assert!(core.file_stored_messages(&[id.clone()]).unwrap().is_empty(), "only INBOX messages run the order");
    assert!(why(&core, &id, &l.contracts).is_none(), "the entry rule matches the subject, the folder says no");
}

// ---------------------------------------------------------------- A140, the three-step pipeline (lane flow3-core)

/// A140 cases: "To read" (described, AI card on, step 1 = "is unread"); every other label's AI card off, so the model's questions are
/// about "To read" alone and can be counted.
fn to_read_setup() -> (RustCore, String, String, Arc<FakeAi>, tempfile::TempDir) {
    let (core, a, l, d) = setup();
    for id in [l.contracts.as_str(), l.needs_reply.as_str(), "needs-reply", "updates", "promotions", "social"] { ai_off(&core, id); }
    let to_read = label(&core.store(), "To read", "mail I still have to read", vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }]).id;
    let ai = FakeAi::new(&[("Read me", "To read"), ("Unread one", "To read"), ("Unread two", "To read")]);
    core.set_decision_port(Some(ai.clone()));
    (core, a, to_read, ai, d)
}
/// How many questions the fake model was asked about the message with this subject.
fn asked_about(ai: &FakeAi, subject: &str) -> usize {
    ai.calls.lock().unwrap().iter().filter(|(s, _)| s.fields.get("subject").map(String::as_str) == Some(subject)).count()
}
fn read_mail(account: &str, key: &str, thread: &str, subject: &str, at: i64) -> NewMessage {
    let mut m = mail(account, key, thread, "pat@other.example", subject, at);
    m.message.read = true;
    m
}

#[test]
fn a140_on_arrival_the_ai_is_asked_only_about_the_mails_step_1_picked() {
    let (core, a, to_read, ai, _d) = to_read_setup();
    core.store().insert_messages(vec![
        mail(&a, "INBOX:21", "t21", "pat@other.example", "Unread one", 1_000),
        mail(&a, "INBOX:22", "t22", "pat@other.example", "Unread two", 1_100),
        read_mail(&a, "INBOX:23", "t23", "Read me", 1_200),
    ]).unwrap();
    let filed = core.file_stored_messages(&[id_of(&a, "INBOX:21"), id_of(&a, "INBOX:22"), id_of(&a, "INBOX:23")]).unwrap();
    // A94 fails here: "is unread" filed the two unread mails by the rule (0 questions) and the read one was asked (1 question)
    assert_eq!(ai.asked(), 2, "one question per mail step 1 picked");
    assert_eq!((asked_about(&ai, "Unread one"), asked_about(&ai, "Unread two"), asked_about(&ai, "Read me")), (1, 1, 0));
    let by_key = |k: &str| filed.iter().find(|f| f.message_id == id_of(&a, k)).unwrap_or_else(|| panic!("{k} in {filed:?}"));
    for f in [by_key("INBOX:21"), by_key("INBOX:22")] {
        assert_eq!((f.place.as_str(), f.card, &f.ai, f.asked.clone()), (to_read.as_str(), Some(WhyCard::Ai), &AiStep::Asked, vec![to_read.clone()]), "{f:?}");
        assert!(f.decided_by.contains("step 1 picked it by entry chain 0") && f.decided_by.contains("the model said To read (0.90)"), "{}", f.decided_by);
    }
    let read = by_key("INBOX:23");
    assert_eq!((read.place.as_str(), &read.ai), ("primary", &AiStep::NoLabelOffered), "{read:?}");
    // part 2: step 1 picks the two unread mails (caught_by_entry), the AI accepted both, none waits
    assert_eq!(core.state(format!("label/{to_read}/flow/counts")).unwrap(), "{\"caught_by_entry\":2,\"kept_by_exit\":0,\"sent_to_fallback\":0,\"sorted_by_ai\":2,\"waiting_for_ai\":0}");
}

#[test]
fn a140_a_read_mail_leaves_the_label_whose_step_1_is_unread_at_once_without_the_model() {
    let (core, a, to_read, ai, _d) = to_read_setup();
    let id = id_of(&a, "INBOX:31");
    core.store().insert_messages(vec![mail(&a, "INBOX:31", "t31", "pat@other.example", "Unread one", 1_000)]).unwrap();
    assert_eq!(core.file_stored_messages(&[id.clone()]).unwrap()[0].place, to_read, "the AI accepted it");
    assert_eq!(ai.asked(), 1);
    core.store().set_read(vec![id.clone()], true).unwrap();
    let moved = core.refile_flipped(&[id.clone()]).unwrap();
    // A94 fails here: the flip re-read only the exit card of the AI's pick (off), so the read mail stayed
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert_eq!((moved[0].from.as_str(), moved[0].to.as_str(), moved[0].card), (to_read.as_str(), "primary", WhyCard::Hand));
    assert!(why(&core, &id, &to_read).is_none(), "off its label");
    assert!(core.state(format!("message/{id}/flow_rejection")).is_none(), "leaving by step 1 writes no rejection row");
    assert_eq!(ai.asked(), 1, "a flip never asks the model");
}

#[test]
fn a140_a_mail_that_starts_to_pass_step_1_later_is_asked_once() {
    let (core, a, _to_read, ai, _d) = to_read_setup();
    // a read mail arrives: step 1 ("is unread") does not pick it, the model is not asked
    let first = id_of(&a, "INBOX:41");
    core.store().insert_messages(vec![read_mail(&a, "INBOX:41", "t41", "Old question", 1_000)]).unwrap();
    core.note_stored(vec![first.clone()]);
    core.flush_filing();
    assert_eq!(asked_about(&ai, "Old question"), 0, "A94 fails here: the model was asked about every mail no entry rule caught");
    // the owner marks it unread; a newer message of its thread arrives, and the worker re-reads the older one (a flip)
    core.store().set_read(vec![first.clone()], false).unwrap();
    core.store().insert_messages(vec![mail(&a, "INBOX:42", "t41", "pat@other.example", "Unread one", 2_000)]).unwrap();
    core.note_stored(vec![id_of(&a, "INBOX:42")]);
    core.flush_filing();
    assert_eq!(asked_about(&ai, "Old question"), 1, "step 1 picks it now: the model is asked, once, on the filing worker");
    // another flip on the same facts: the model ruled on it already ("nothing fits"), it is not asked again
    core.store().insert_messages(vec![mail(&a, "INBOX:43", "t41", "pat@other.example", "Unread two", 3_000)]).unwrap();
    core.note_stored(vec![id_of(&a, "INBOX:43")]);
    core.flush_filing();
    assert_eq!(asked_about(&ai, "Old question"), 1, "asked once");
    assert_eq!((asked_about(&ai, "Unread one"), asked_about(&ai, "Unread two")), (1, 1), "each new mail once, on arrival");
}

#[test]
fn a140_the_ai_off_label_whose_step_1_matched_wins_over_a_higher_ai_on_label() {
    let (core, a, l, _d) = setup();
    // Contracts (AI on, higher) and Deals (AI off, lower) both pick "contract"
    let deals = label(&core.store(), "Deals", "", vec![RuleCondition::SubjectContains { words: "contract".into() }]).id;
    let ai = FakeAi::new(&[("Contract renewal", "Contracts")]);
    core.set_decision_port(Some(ai.clone()));
    core.store().insert_messages(vec![mail(&a, "INBOX:51", "t51", "boss@client.example", "Contract renewal", 1_000)]).unwrap();
    let f = core.file_stored_messages(&[id_of(&a, "INBOX:51")]).unwrap().remove(0);
    // A94 fails here: the entry rules went by sidebar order and Contracts' rule won
    assert_eq!((f.place.as_str(), f.card, &f.ai), (deals.as_str(), Some(WhyCard::Entry), &AiStep::EntryRule), "{f:?}");
    assert_eq!(ai.asked(), 0, "the AI-off label settled it before the model");
    assert!(why(&core, &id_of(&a, "INBOX:51"), &l.contracts).is_none());
}

// ---------------------------------------------------------------- A140 part 2: the served counts and "why here" (lane flow3-core)

#[test]
fn a140_the_counts_say_picked_accepted_and_waiting_and_why_names_step_1_and_the_ai() {
    let (core, a, to_read, _ai, _d) = to_read_setup();
    core.store().insert_messages(vec![
        mail(&a, "INBOX:61", "t61", "pat@other.example", "Unread one", 1_000),
        mail(&a, "INBOX:62", "t62", "pat@other.example", "Unread two", 1_100),
        read_mail(&a, "INBOX:63", "t63", "Read me", 1_200),
    ]).unwrap();
    // only the first is run through the flow: the second is picked by step 1 and waits for the AI
    let id = id_of(&a, "INBOX:61");
    assert_eq!(core.file_stored_messages(&[id.clone()]).unwrap()[0].place, to_read);
    // part 1 served (0,0,0,1) with no `waiting_for_ai`: the picks of an AI-on label were never counted
    assert_eq!(core.state(format!("label/{to_read}/flow/counts")).unwrap(), "{\"caught_by_entry\":2,\"kept_by_exit\":0,\"sent_to_fallback\":0,\"sorted_by_ai\":1,\"waiting_for_ai\":1}");
    let w = why(&core, &id, &to_read).expect("filed by the AI");
    assert!(w.starts_with("{\"ai\":\"yes\",\"card\":\"ai\",\"chain_id\":null,\"decided_by\":\"laya\",\"entry\":{\"chain_id\":0,\"matched\":["), "{w}");
    assert!(!w.contains("\"matched\":[]},"), "step 1's condition that held is named: {w}");
}

#[test]
fn a140_an_ai_off_labels_step_3_rejection_is_not_counted_as_sorted_by_the_ai() {
    let (core, a, l, _d) = setup();
    ai_off(&core, &l.contracts);
    let starred = RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }], match_mode: MatchMode::All };
    core.store().label_chain_add(l.contracts.clone(), ChainPlacement::Exit, starred, 6).unwrap();
    core.store().label_set_exit_fallback(l.contracts.clone(), Some("primary".into()), 7).unwrap();
    core.store().label_set_card(l.contracts.clone(), FlowCard::Exit, true, 8).unwrap();
    let id = id_of(&a, "INBOX:71");
    core.store().insert_messages(vec![mail(&a, "INBOX:71", "t71", "boss@client.example", "Contract renewal", 1_000)]).unwrap();
    let f = core.file_stored_messages(&[id.clone()]).unwrap().remove(0);
    assert_eq!((f.place.as_str(), f.card, &f.ai), ("primary", Some(WhyCard::Fallback), &AiStep::EntryRule), "step 1 picked it, step 3 sent it on: {f:?}");
    // the contract's formula counts this fallback filing as sorted by the AI too (part 1 served sorted_by_ai 1); the AI never spoke
    assert_eq!(core.state(format!("label/{}/flow/counts", l.contracts)).unwrap(), "{\"caught_by_entry\":1,\"kept_by_exit\":0,\"sent_to_fallback\":1,\"sorted_by_ai\":0,\"waiting_for_ai\":0}");
    let w = why(&core, &id, "primary").expect("why here in Primary");
    assert!(w.contains("\"card\":\"fallback\"") && !w.contains("\"ai\":"), "no AI answer for an AI-off label: {w}");
}

// ---------------------------------------------------------------- A140 part 3: the AI's "no", kept in the store (lane flow3-core)

fn counts_of(core: &RustCore, label: &str) -> String { core.state(format!("label/{label}/flow/counts")).unwrap() }
fn to_read_spec(description: &str) -> LabelSpec {
    LabelSpec { name: "To read".into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All,
                rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::Any }, pinned: false }
}
/// One unread mail "Unread three" that step 1 of "To read" picks and the fake AI refuses (it has no pick for that subject).
fn refused_mail(core: &RustCore, a: &str, key: &str, thread: &str) -> String {
    let id = id_of(a, key);
    core.store().insert_messages(vec![mail(a, key, thread, "pat@other.example", "Unread three", 1_000)]).unwrap();
    let f = core.file_stored_messages(&[id.clone()]).unwrap().remove(0);
    assert_eq!((f.place.as_str(), &f.ai), ("primary", &AiStep::Asked), "the AI was asked and said no: {f:?}");
    id
}

#[test]
fn a140_a_mail_the_ai_refused_is_not_counted_as_waiting() {
    let (core, a, to_read, ai, _d) = to_read_setup();
    refused_mail(&core, &a, "INBOX:81", "t81");
    assert_eq!(asked_about(&ai, "Unread three"), 1);
    // 666d88c6 served waiting_for_ai 1 here: the store kept no "no"
    assert_eq!(counts_of(&core, &to_read), "{\"caught_by_entry\":1,\"kept_by_exit\":0,\"sent_to_fallback\":0,\"sorted_by_ai\":0,\"waiting_for_ai\":0}");
}

#[test]
fn a140_after_a_restart_a_refused_mail_that_flips_back_in_is_not_asked() {
    let (core, a, _to_read, ai, d) = to_read_setup();
    let first = id_of(&a, "INBOX:91");
    core.store().insert_messages(vec![mail(&a, "INBOX:91", "t91", "pat@other.example", "Unread three", 1_000)]).unwrap();
    core.note_stored(vec![first.clone()]);
    core.flush_filing();
    assert_eq!(asked_about(&ai, "Unread three"), 1, "asked once on arrival, and it said no");
    // a restart: a new engine over the same store (the in-memory book is gone)
    drop(core);
    let core = RustCore::with_parts(Store::open(&d.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let ai = FakeAi::new(&[]);
    core.set_decision_port(Some(ai.clone()));
    // a newer message of its thread arrives: the worker re-reads the older one (a flip) and step 1 still picks it
    core.store().insert_messages(vec![mail(&a, "INBOX:92", "t91", "pat@other.example", "Unread two", 2_000)]).unwrap();
    core.note_stored(vec![id_of(&a, "INBOX:92")]);
    core.flush_filing();
    // 666d88c6 asked it again here: its "no" lived in memory only
    assert_eq!(asked_about(&ai, "Unread three"), 0, "the kept no holds across the restart");
    assert_eq!(asked_about(&ai, "Unread two"), 1, "the new mail is asked, once");
}

#[test]
fn a140_a_rerun_asks_a_refused_mail_again() {
    let (core, a, to_read, _ai, _d) = to_read_setup();
    let id = refused_mail(&core, &a, "INBOX:101", "t101");
    assert!(counts_of(&core, &to_read).ends_with("\"waiting_for_ai\":0}"), "{}", counts_of(&core, &to_read));
    // the model now says yes; the owner presses Re-run on the label: judge again
    let ai = FakeAi::new(&[("Unread three", "To read")]);
    core.set_decision_port(Some(ai.clone()));
    core.run(Command::RerunLabel { label_id: to_read.clone() }).expect("served");
    core.flush_filing();
    assert_eq!(asked_about(&ai, "Unread three"), 1, "Re-run asks the refused mail again");
    assert!(why(&core, &id, &to_read).is_some(), "and files it now the model says yes");
    assert_eq!(counts_of(&core, &to_read), "{\"caught_by_entry\":1,\"kept_by_exit\":0,\"sent_to_fallback\":0,\"sorted_by_ai\":1,\"waiting_for_ai\":0}");
}

#[test]
fn a140_a_changed_description_or_ai_reads_clears_the_labels_refusals_and_a_changed_step_1_does_not() {
    let (core, a, to_read, _ai, _d) = to_read_setup();
    let id = refused_mail(&core, &a, "INBOX:111", "t111");
    let waiting = |core: &RustCore| counts_of(core, &to_read).rsplit(':').next().unwrap().trim_end_matches('}').to_string();
    assert_eq!(waiting(&core), "0", "refused: not waiting (666d88c6: 1)");
    // the description changes: the question changed, the refusal goes and the mail waits for the AI again
    core.store().label_update_full(to_read.clone(), to_read_spec("mail I still have to read, newsletters left out"), 9).unwrap();
    assert_eq!(waiting(&core), "1");
    // asked again (arrival path), refused again; then the AI-reads ticks change: cleared again
    core.file_stored_messages(&[id.clone()]).unwrap();
    assert_eq!(waiting(&core), "0");
    core.store().label_set_ai_reads(to_read.clone(), AiReads { subject: true, body: false, rest_of_thread: false, attachments: false }, 10).unwrap();
    assert_eq!(waiting(&core), "1");
    // refused again; a new step-1 chain changes who is asked, not what was answered: nothing is cleared
    core.file_stored_messages(&[id.clone()]).unwrap();
    assert_eq!(waiting(&core), "0");
    core.store().label_chain_add(to_read.clone(), ChainPlacement::Entry, RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }], match_mode: MatchMode::All }, 11).unwrap();
    assert_eq!(waiting(&core), "0");
}
