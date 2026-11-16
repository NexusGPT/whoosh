//! Lane labels-r5, round 2: the three label reads and arms of the real core that were stubs or missing. `rule_set_count` and
//! `label_rerun_count` answered a constant zero, and `Command::RerunLabel` had no arm (it answered "not built yet"). The stores are
//! real; the model is a fake DecisionPort.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants-engine.out; runner _captures/labels-r5/run-mutants.py):
//! - E1 rust_core.rs `rule_set_count`: back to the constant zero -> the_live_count_is_the_real_one
//! - E2 rust_core.rs `label_rerun_count`: back to the constant zero -> the_rerun_offer_counts_what_is_filed
//! - E3 rust_core.rs the `Command::RerunLabel` arm answers Ok and does nothing -> a_rerun_files_what_the_rules_now_catch_and_unfiles_what_they_no_longer_do
//! - E4 flow_ctx.rs `rerun_label`: `unassign` never set (`FlowPlace::Label(held) if false && file`) -> a_rerun_files_what_the_rules_now_catch_and_unfiles_what_they_no_longer_do
//! - E5 flow_ctx.rs `current_filing`: drop `AND flow_card IS NOT NULL` (the owner's hand filing would read as a held label and leave) -> the_owners_own_filing_never_leaves
//! - E6 flow_ctx.rs `rerun_label`: pending rows are written too -> a_message_the_model_could_not_judge_keeps_its_filing
//! - F1 filing_worker.rs `older_in_threads`: `m.is_outgoing || ` dropped (a late Sent reply flips only the OLDER messages again) -> a_late_sent_reply_flips_the_newer_inbound_message_of_its_thread
//! - M1 rust_core.rs `rerun_state`: always "false" -> a_rerun_says_whether_the_model_part_was_pending
//! - G1 flow_ctx.rs `with_thread`: `n.ai_enabled && n.rest_of_thread` -> `false` (the thread is never loaded) -> a_label_with_the_thread_switch_on_sends_the_earlier_messages_cut_to_the_budget
//! - G2 decision entry.rs `LabelNode::from_label`: the card's switch ignored (`rest_of_thread: false`) -> the same test
//! - G3 decision entry.rs `LabelNode::from_label`: `rest_of_thread: true` for every label -> a_label_with_the_switch_off_gets_no_thread
//! - E7 flow_ctx.rs `rerun_label`: the entry-rule candidates are not listed -> a_rerun_files_what_the_rules_now_catch_and_unfiles_what_they_no_longer_do
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
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


fn spec(name: &str, description: &str, words: Option<&str>) -> LabelSpec {
    let conditions = words.map(|w| vec![RuleCondition::SubjectContains { words: w.into() }]).unwrap_or_default();
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules: RuleSet { conditions, match_mode: MatchMode::Any }, pinned: false }
}

/// A model that answers the label named for a subject (0.9), or declines.
struct FakeAi { picks: HashMap<String, String>, calls: Mutex<usize> }
impl FakeAi {
    fn new(picks: &[(&str, &str)]) -> Arc<FakeAi> { Arc::new(FakeAi { picks: picks.iter().map(|(s, l)| (s.to_string(), l.to_string())).collect(), calls: Mutex::new(0) }) }
}
impl DecisionPort for FakeAi {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        *self.calls.lock().unwrap() += 1;
        let subject = state.fields.get("subject").cloned().unwrap_or_default();
        // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, self.picks.get(&subject).map(String::as_str), 0.9)).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

fn setup() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, dir)
}
/// The built-in labels are described since round 6, so the Labels page asks the model and a re-run with no model keeps filings (pending); the cases about
/// rules and hand filings switch the built-ins' AI cards off.
fn ai_off_for_builtins(core: &RustCore) {
    for id in ["needs-reply", "updates", "promotions", "social"] { core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false }).unwrap(); }
}
fn filing(core: &RustCore, id: &str) -> String { core.state(format!("message/{id}/filing")).unwrap_or_default() }
/// The re-run answers at once and runs on the filing worker: wait for it.
fn rerun(core: &RustCore, label: &str) { core.run(Command::RerunLabel { label_id: label.into() }).expect("the re-run is served"); core.flush_filing(); }

#[test]
fn the_live_count_is_the_real_one() {
    let (core, a, _d) = setup();
    // thread t1: I wrote (500), then a new message arrived unread (1000); thread t2: one unread message, I never wrote in it
    core.store().insert_messages(vec![
        mail(&a, "Sent:1", "t1", USER, "Re: villa", 500),
        mail(&a, "INBOX:1", "t1", "ann@client.example", "villa again", 1_000),
        mail(&a, "INBOX:2", "t2", "bob@vendor.example", "invoice", 1_100),
    ]).unwrap();
    let both = RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }, RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: true }], match_mode: MatchMode::All };
    let c = core.rule_set_count(both);
    assert_eq!((c.count, c.unread, c.newest_at_ms), (1, 1, Some(1_000)), "the unread thread I already wrote in");
    let unread = RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], match_mode: MatchMode::All };
    assert_eq!(core.rule_set_count(unread).count, 2, "both inbound messages are unread; the Sent copy is not in the corpus");
}

#[test]
fn the_rerun_offer_counts_what_is_filed() {
    let (core, a, _d) = setup();
    let contracts = core.store().label_create_full(spec("Contracts", "", Some("contract")), 1).unwrap().id;
    core.store().insert_messages(vec![
        mail(&a, "INBOX:1", "t1", "boss@client.example", "Contract A", 1_000),
        mail(&a, "INBOX:2", "t2", "boss@client.example", "Contract B", 1_100),
        mail(&a, "INBOX:3", "t3", "friend@home.example", "Hello", 1_200),
    ]).unwrap();
    assert_eq!(core.label_rerun_count(contracts.clone()), 0, "nothing is filed yet");
    core.file_stored_messages(&[id_of(&a, "INBOX:1"), id_of(&a, "INBOX:2"), id_of(&a, "INBOX:3")]).unwrap();
    assert_eq!(core.label_rerun_count(contracts), 2);
    assert_eq!(core.label_rerun_count("no-such-label".into()), 0);
}

#[test]
fn a_rerun_files_what_the_rules_now_catch_and_unfiles_what_they_no_longer_do() {
    let (core, a, _d) = setup();
    ai_off_for_builtins(&core);
    let clients = core.store().label_create_full(spec("Clients", "", None), 1).unwrap().id;
    core.store().insert_messages(vec![
        mail(&a, "INBOX:1", "t1", "ann@client.example", "Quote villa", 1_000),
        mail(&a, "INBOX:2", "t2", "bob@vendor.example", "Hello", 1_100),
    ]).unwrap();
    let (m1, m2) = (id_of(&a, "INBOX:1"), id_of(&a, "INBOX:2"));
    core.file_stored_messages(&[m1.clone(), m2.clone()]).unwrap();
    assert_eq!(core.label_rerun_count(clients.clone()), 0, "no rule yet: nothing is filed");
    // the owner gives the label a rule and re-runs: the message the rule now catches joins, the other stays out
    core.store().label_update_full(clients.clone(), spec("Clients", "", Some("villa")), 2).unwrap();
    rerun(&core, &clients);
    assert_eq!(core.label_rerun_count(clients.clone()), 1);
    let f = filing(&core, &m1);
    assert!(f.contains(&format!("\"place\":\"{clients}\"")) && f.contains("\"card\":\"entry\""), "{f}");
    assert!(!filing(&core, &m2).contains(&clients), "{}", filing(&core, &m2));
    // the rule changes to one nothing matches: a re-run takes the message the entry card had filed back out
    core.store().label_update_full(clients.clone(), spec("Clients", "", Some("zzz")), 3).unwrap();
    rerun(&core, &clients);
    assert_eq!(core.label_rerun_count(clients.clone()), 0, "the entry rule no longer catches it");
    assert!(filing(&core, &m1).contains("\"place\":\"primary\""), "{}", filing(&core, &m1));
}

#[test]
fn the_owners_own_filing_never_leaves() {
    let (core, a, _d) = setup();
    ai_off_for_builtins(&core);
    let clients = core.store().label_create_full(spec("Clients", "", Some("zzz")), 1).unwrap().id;
    core.store().insert_messages(vec![mail(&a, "INBOX:1", "t1", "ann@client.example", "Hello", 1_000)]).unwrap();
    let m1 = id_of(&a, "INBOX:1");
    core.store().label_file(clients.clone(), m1.clone(), DecidedBy::Owner, None, 2).unwrap();
    rerun(&core, &clients);
    assert_eq!(core.label_rerun_count(clients), 1, "the owner filed it by hand: no rule or model takes it back");
}

#[test]
fn a_message_the_model_could_not_judge_keeps_its_filing() {
    let (core, a, _d) = setup();
    let reply = core.store().label_create_full(spec("Reply soon", "mail that waits for my answer", None), 1).unwrap().id;
    core.store().insert_messages(vec![mail(&a, "INBOX:1", "t1", "ann@client.example", "Can you answer?", 1_000)]).unwrap();
    let m1 = id_of(&a, "INBOX:1");
    core.set_decision_port(Some(FakeAi::new(&[("Can you answer?", "Reply soon")])));
    core.file_stored_messages(&[m1.clone()]).unwrap();
    assert_eq!(core.label_rerun_count(reply.clone()), 1, "the model filed it");
    // the provider goes away: the meaning part cannot run, so a re-run must leave the model's filing where it is
    core.set_decision_port(None);
    rerun(&core, &reply);
    assert_eq!(core.label_rerun_count(reply), 1, "pending, not unfiled");
}

#[test]
fn a_late_sent_reply_flips_the_newer_inbound_message_of_its_thread() {
    // Owner ruling 2026-11-10: "already answered by me" = I wrote EARLIER in the thread. A reply stored late (a backfill reaches Sent after
    // INBOX) is dated before the second inbound message: that message's fact must turn yes when the reply is stored.
    let (core, a, _d) = setup();
    let answered = core.store().label_create_full(
        LabelSpec { name: "Answered".into(), parent_id: None, colour: None, description: String::new(), scope: LabelScope::All,
                    rules: RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: true }], match_mode: MatchMode::Any }, pinned: false }, 1).unwrap().id;
    let (first, second, reply) = (id_of(&a, "INBOX:1"), id_of(&a, "INBOX:2"), id_of(&a, "Sent:1"));
    core.store().insert_messages(vec![mail(&a, "INBOX:1", "t1", "ann@client.example", "Question", 1_000)]).unwrap();
    core.note_stored(vec![first.clone()]); core.flush_filing();
    core.store().insert_messages(vec![mail(&a, "INBOX:2", "t1", "ann@client.example", "Re: Question", 3_000)]).unwrap();
    core.note_stored(vec![second.clone()]); core.flush_filing();
    assert_eq!(core.label_rerun_count(answered.clone()), 0, "I have not written in the thread yet");
    // my reply is stored LAST and dated between the two inbound messages
    core.store().insert_messages(vec![mail(&a, "Sent:1", "t1", USER, "Re: Question", 2_000)]).unwrap();
    core.note_stored(vec![reply]); core.flush_filing();
    assert!(filing(&core, &second).contains(&format!("\"place\":\"{answered}\"")), "the newer message reads 'I wrote earlier': {}", filing(&core, &second));
    assert!(!filing(&core, &first).contains(&answered), "the first message has no message of mine before it: {}", filing(&core, &first));
    assert_eq!(core.label_rerun_count(answered), 1);
}

#[test]
fn a_rerun_says_whether_the_model_part_was_pending() {
    let (core, a, _d) = setup();
    let reply = core.store().label_create_full(spec("Reply soon", "mail that waits for my answer", None), 1).unwrap().id;
    let path = format!("label/{reply}/rerun/model_pending");
    assert_eq!(core.state(path.clone()).as_deref(), Some("false"), "no re-run yet");
    assert_eq!(core.state("label/no-such-label/rerun/model_pending".into()), None);
    core.store().insert_messages(vec![mail(&a, "INBOX:1", "t1", "ann@client.example", "Can you answer?", 1_000)]).unwrap();
    let m1 = id_of(&a, "INBOX:1");
    core.set_decision_port(Some(FakeAi::new(&[("Can you answer?", "Reply soon")])));
    core.file_stored_messages(&[m1]).unwrap();
    // with the model gone the filed message is pending: the path says so
    core.set_decision_port(None);
    rerun(&core, &reply);
    assert_eq!(core.state(path.clone()).as_deref(), Some("true"));
    // with the model back the re-run settles it and the path clears
    core.set_decision_port(Some(FakeAi::new(&[("Can you answer?", "Reply soon")])));
    rerun(&core, &reply);
    assert_eq!(core.state(path).as_deref(), Some("false"));
}

/// A model that records what it was sent.
struct Seen { states: Mutex<Vec<DecisionState>> }
impl DecisionPort for Seen {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        self.states.lock().unwrap().push(state);
        let answers = questions.iter().filter_map(|q| ::decision::test_answers::labels_answer(q, None, 0.0)).collect(); // fix-aisort round 3: by the question's form
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

/// A described label with the thread switch on or off, a thread of 30 earlier messages (about 900 characters each), then the arrival
/// the model is asked about. Returns what the model saw for the arrival.
fn thread_sent(switch: bool) -> Vec<DecisionState> {
    let (core, a, _d) = setup();
    let l = core.store().label_create_full(spec("Reply soon", "mail that waits for my answer", None), 1).unwrap().id;
    core.store().label_set_ai_reads(l, AiReads { subject: true, body: true, rest_of_thread: switch, attachments: false }, 2).unwrap();
    let pad = "x".repeat(900);
    let mut batch: Vec<NewMessage> = (0..30).map(|i| {
        let mut m = mail(&a, &format!("INBOX:{}", 100 + i), "t1", "ann@client.example", &format!("earlier {i:02} {pad}"), 1_000 + i as i64);
        m.message.read = true;
        m
    }).collect();
    batch.push(mail(&a, "INBOX:1", "t1", "ann@client.example", "Can you answer?", 9_000));
    core.store().insert_messages(batch).unwrap();
    let seen = Arc::new(Seen { states: Mutex::new(vec![]) });
    core.set_decision_port(Some(seen.clone()));
    core.file_stored_messages(&[id_of(&a, "INBOX:1")]).unwrap();
    let out = seen.states.lock().unwrap().clone();
    out
}

#[test]
fn a_label_with_the_thread_switch_on_sends_the_earlier_messages_cut_to_the_budget() {
    let states = thread_sent(true);
    assert_eq!(states.len(), 1, "the Labels question went in one call");
    let field = states[0].fields.get(::decision::thread_context::THREAD_FIELD).expect("the earlier messages ride in the state");
    let parts = ::decision::thread_context::decode(field);
    assert!(!parts.is_empty() && parts.len() < 30, "the loader stops at the biggest budget a model can use, not at the whole thread: {}", parts.len());
    assert!(parts.last().unwrap().text.contains("earlier 29"), "the newest earlier message is kept");
    assert!(!parts.iter().any(|p| p.text.contains("earlier 00")), "the oldest is not loaded");
    let loaded: usize = parts.iter().map(|p| p.text.chars().count()).sum();
    assert!(loaded < ::decision::thread_context::load_up_to_chars() + 1_000, "{loaded} characters loaded");
    // each engine cuts it again to its own budget, oldest first
    let cut = ::decision::thread_context::fit(&parts, ::decision::thread_context::jev_budget_chars());
    assert!(cut.len() < parts.len() && cut.last() == parts.last());
}

#[test]
fn a_label_with_the_switch_off_gets_no_thread() {
    let states = thread_sent(false);
    assert_eq!(states.len(), 1);
    assert!(!states[0].fields.contains_key(::decision::thread_context::THREAD_FIELD), "the earlier messages never leave unless the card ticks them");
}
