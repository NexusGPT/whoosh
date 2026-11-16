//! Lane labels-r5, round 7: five state paths the fixture core serves and the real core did not (found by lane replies-settings' parity test):
//! `label/<id>/rerun/filed|refiled|unfiled`, `label/<id>/rules`, `message/<id>/label/<label>/decided_by`. Without them the inspector's Re-run line showed no
//! counts and "Why here?" named no rule and no model. Real core, real store, a scripted on-device model (no network, no real model).
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out; runner _captures/labels-r5/run-mutants.py):
//! - P1 flow_ctx.rs `rerun_label`: `refiled` not counted -> a_rerun_after_a_description_change_counts_filed_refiled_and_unfiled
//! - P2 flow_ctx.rs `rerun_label`: `unfiled` not counted -> the same case
//! - P3 label_flow.rs the `rules` arm: the match mode always "any" -> the_rules_of_a_label_with_two_chains_are_the_sheets_chain_zero
//! - P4 label_flow.rs the `rules` arm: the chain-0 read replaced by the label's last chain -> the same case
//! - P5 label_flow.rs the `decided_by` arm: always "default" -> decided_by_names_the_rule_the_fallbacks_model_and_the_scripted_model
//! - P6 label_flow.rs the `decided_by` arm: answers for a label the message is not filed under -> the same case
//! - P7 flow_ctx.rs `rerun_candidates`: `beyond` always 0 -> a_rerun_of_more_than_500_says_how_many_are_left_and_continues
//! - P8 flow_ctx.rs `next_skip`: a run never continues, it starts at 0 again -> the same case
//! - P9 flow_ctx.rs `continuation`: a changed description does not restart a sequence -> a_changed_description_restarts_a_sequence_that_still_has_messages_left
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
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




fn spec(name: &str, description: &str, words: Option<&str>) -> LabelSpec {
    let conditions = words.map(|w| vec![RuleCondition::SubjectContains { words: w.into() }]).unwrap_or_default();
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: description.into(), scope: LabelScope::All, rules: RuleSet { conditions, match_mode: MatchMode::Any }, pinned: false }
}

struct Scripted { pick: std::sync::Mutex<String> }
impl DecisionPort for Scripted {
    fn provider(&self) -> DecisionProviderKind { DecisionProviderKind::Local }
    fn decide(&self, _state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let pick = self.pick.lock().unwrap().clone();
        // fix-aisort round 3: the Labels question answered by its form (one choice, or one yes/no per label: LABEL_AI_QUESTION)
        let answers = questions.iter().map(|q| ::decision::test_answers::labels_answer(q, Some(pick.as_str()), 0.95).unwrap_or_else(|| match q {
            DecisionQuestion::Choice { id, .. } => DecisionAnswer::Choice { id: id.clone(), choice: pick.clone(), probabilities: [(pick.clone(), 0.95)].into() },
            DecisionQuestion::Score { id, .. } => DecisionAnswer::Score { id: id.clone(), value: 0.0 },
            DecisionQuestion::Noul { id, .. } => DecisionAnswer::Noul { id: id.clone(), yes: false, probability: 0.1 },
        })).collect();
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: None })
    }
}

fn setup() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    (RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback), account, dir)
}
fn st(core: &RustCore, path: String) -> String { core.state(path).unwrap_or_else(|| "<none>".into()) }
fn rerun(core: &RustCore, label: &str) { core.run(Command::RerunLabel { label_id: label.into() }).unwrap(); core.flush_filing(); }

#[test]
fn a_rerun_after_a_description_change_counts_filed_refiled_and_unfiled() {
    let (core, a, _d) = setup();
    let reply = core.store().label_create_full(spec("Reply soon", "", None), 1).unwrap().id;
    for n in 1..=3 { core.store().insert_messages(vec![mail(&a, &format!("INBOX:{n}"), &format!("t{n}"), "ann@client.example", "Can you answer?", 1_000 + n as i64)]).unwrap(); }
    let at = |leaf: &str| st(&core, format!("label/{reply}/rerun/{leaf}"));
    assert_eq!((at("filed").as_str(), at("refiled").as_str(), at("unfiled").as_str()), ("0", "0", "0"), "before any re-run");
    // the owner writes the description and the model picks Reply soon: three messages join
    let model = Arc::new(Scripted { pick: std::sync::Mutex::new("Reply soon".into()) });
    core.set_laya_port(Some(model.clone()));
    core.store().label_update_full(reply.clone(), spec("Reply soon", "mail that waits for my answer", None), 2).unwrap();
    rerun(&core, &reply);
    assert_eq!((at("filed").as_str(), at("refiled").as_str(), at("unfiled").as_str()), ("3", "3", "0"), "three filed, three re-filed, none unfiled");
    // the description changes and the model now says Updates: the three leave
    *model.pick.lock().unwrap() = "Updates".into();
    core.store().label_update_full(reply.clone(), spec("Reply soon", "mail about nothing at all", None), 3).unwrap();
    rerun(&core, &reply);
    assert_eq!((at("filed").as_str(), at("refiled").as_str(), at("unfiled").as_str()), ("0", "0", "3"), "none left, none re-filed, three unfiled");
}

#[test]
fn the_rules_of_a_label_with_two_chains_are_the_sheets_chain_zero() {
    let (core, _a, _d) = setup();
    let first = RuleSet { conditions: vec![RuleCondition::SubjectContains { words: "contract".into() }, RuleCondition::From { words: "boss".into() }], match_mode: MatchMode::All };
    let l = core.store().label_create_full(LabelSpec { name: "Deals".into(), parent_id: None, colour: None, description: String::new(), scope: LabelScope::All, rules: first, pinned: false }, 1).unwrap().id;
    core.store().label_chain_add(l.clone(), ChainPlacement::Entry, RuleSet { conditions: vec![RuleCondition::Domain { domain: "other.example".into() }], match_mode: MatchMode::Any }, 2).unwrap();
    let got: serde_json::Value = serde_json::from_str(&st(&core, format!("label/{l}/rules"))).expect("JSON");
    assert_eq!(got, serde_json::json!({"match": "all", "conditions": [{"kind": "subject", "words": "contract"}, {"kind": "from", "words": "boss"}]}), "chain 0 only, as the sheet writes it");
    // a label with no rule, and one that does not exist: nothing, as the fixture core answers
    let bare = core.store().label_create_full(spec("Bare", "", None), 3).unwrap().id;
    assert_eq!(core.state(format!("label/{bare}/rules")), None);
    assert_eq!(core.state("label/no-such/rules".into()), None);
}

#[test]
fn decided_by_names_the_rule_the_fallbacks_model_and_the_scripted_model() {
    let (core, a, _d) = setup();
    // Deals: an entry rule on the subject. Reply soon: the AI's, with an exit filter (starred: yes) whose fallback is Fallback label.
    let deals = core.store().label_create_full(spec("Deals", "", Some("contract")), 1).unwrap().id;
    let fb = core.store().label_create_full(spec("Parked", "", None), 1).unwrap().id;
    let reply = core.store().label_create_full(spec("Reply soon", "mail that waits for my answer", None), 1).unwrap().id;
    core.store().label_chain_add(reply.clone(), ChainPlacement::Exit, RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }], match_mode: MatchMode::All }, 2).unwrap();
    core.store().label_set_exit_fallback(reply.clone(), Some(fb.clone()), 3).unwrap();
    core.store().label_set_card(reply.clone(), FlowCard::Exit, true, 4).unwrap();
    core.set_laya_port(Some(Arc::new(Scripted { pick: std::sync::Mutex::new("Reply soon".into()) })));
    core.store().insert_messages(vec![
        mail(&a, "INBOX:1", "t1", "boss@client.example", "Contract renewal", 1_000),   // the entry rule
        mail(&a, "INBOX:2", "t2", "ann@client.example", "Can you answer?", 1_100),      // the model, then the exit filter rejects it (not starred)
    ]).unwrap();
    core.file_stored_messages(&[id_of(&a, "INBOX:1"), id_of(&a, "INBOX:2")]).unwrap();
    let by = |m: &str, l: &str| core.state(format!("message/{}/label/{l}/decided_by", id_of(&a, m)));
    // by an entry rule: the rule's own name (the chain's record id)
    assert_eq!(by("INBOX:1", &deals).as_deref(), Some(format!("\"rule:rule-{deals}\"").as_str()));
    // by the exit filter's fallback: the message sits under the fallback label, placed by the model that picked Reply soon
    assert_eq!(by("INBOX:2", &fb).as_deref(), Some("\"laya\""), "the model's pick, sent on by the exit filter");
    assert_eq!(by("INBOX:2", &reply), None, "it is not filed under the label the model picked: the exit filter sent it on");
    // by the scripted model directly: pass the exit filter (starred) and the AI card's own label keeps it
    core.store().insert_messages(vec![mail(&a, "INBOX:3", "t3", "ann@client.example", "Another question", 1_200)]).unwrap();
    core.store().write(|ctx| { ctx.tx.execute("UPDATE message SET starred = 1 WHERE id = ?1", [format!("{}:INBOX:3", ctx.tx.query_row("SELECT id FROM account LIMIT 1", [], |r| r.get::<_, String>(0))?)])?; Ok(()) }).unwrap();
    core.file_stored_messages(&[id_of(&a, "INBOX:3")]).unwrap();
    assert_eq!(by("INBOX:3", &reply).as_deref(), Some("\"laya\""));
    // never filed there, or no such label: nothing
    assert_eq!(by("INBOX:1", &reply), None);
    assert_eq!(by("INBOX:1", "no-such"), None);
}

#[test]
fn a_rerun_of_more_than_500_says_how_many_are_left_and_continues() {
    let (core, a, _d) = setup();
    let reply = core.store().label_create_full(spec("Reply soon", "", None), 1).unwrap().id;
    let n = 1_203usize;
    let rows: Vec<_> = (1..=n).map(|i| mail(&a, &format!("INBOX:{i}"), &format!("t{i}"), "ann@client.example", "Can you answer?", 10_000 + i as i64)).collect();
    core.store().insert_messages(rows).unwrap();
    core.set_laya_port(Some(Arc::new(Scripted { pick: std::sync::Mutex::new("Reply soon".into()) })));
    core.store().label_update_full(reply.clone(), spec("Reply soon", "mail that waits for my answer", None), 2).unwrap();
    let at = |leaf: &str| st(&core, format!("label/{reply}/rerun/{leaf}"));
    // before any run: this step looks at 500 and says 703 are left
    assert_eq!((at("candidates").as_str(), at("beyond").as_str()), ("500", "703"));
    rerun(&core, &reply);
    assert_eq!((at("total").as_str(), at("done").as_str(), at("beyond").as_str(), at("filed").as_str()), ("500", "500", "703", "500"), "500 sorted, 703 left, said in a number");
    // the same description continues with the next 500, then the last 203: nothing is skipped and nothing is judged twice
    rerun(&core, &reply);
    assert_eq!((at("total").as_str(), at("beyond").as_str(), at("filed").as_str()), ("500", "203", "1000"));
    rerun(&core, &reply);
    assert_eq!((at("total").as_str(), at("beyond").as_str(), at("filed").as_str()), ("203", "0", "1203"), "the last step reaches the oldest");
    // every message was filed once: the oldest and the newest both sit under the label
    for i in [1, n] { assert!(core.state(format!("message/{}/label/{reply}/decided_by", id_of(&a, &format!("INBOX:{i}")))).is_some(), "message {i}"); }
    // a changed description starts again at the newest 500
    core.store().label_update_full(reply.clone(), spec("Reply soon", "mail that needs an answer from me, not a newsletter", None), 3).unwrap();
    assert_eq!(at("beyond"), "703");
    rerun(&core, &reply);
    assert_eq!((at("total").as_str(), at("beyond").as_str()), ("1203", "703"), "restarted at the newest 500, and the 1203 filed under the label are looked at again (what a re-run always did)");
}

#[test]
fn a_changed_description_restarts_a_sequence_that_still_has_messages_left() {
    let (core, a, _d) = setup();
    let reply = core.store().label_create_full(spec("Reply soon", "", None), 1).unwrap().id;
    let rows: Vec<_> = (1..=1_003usize).map(|i| mail(&a, &format!("INBOX:{i}"), &format!("t{i}"), "ann@client.example", "Can you answer?", 10_000 + i as i64)).collect();
    core.store().insert_messages(rows).unwrap();
    core.set_laya_port(Some(Arc::new(Scripted { pick: std::sync::Mutex::new("Reply soon".into()) })));
    core.store().label_update_full(reply.clone(), spec("Reply soon", "mail that waits for my answer", None), 2).unwrap();
    let at = |leaf: &str| st(&core, format!("label/{reply}/rerun/{leaf}"));
    rerun(&core, &reply);
    assert_eq!(at("beyond"), "503", "500 sorted, 503 left");
    // the owner rewrites the description before pressing Re-run again: the sequence starts over at the newest message, it does not continue
    core.store().label_update_full(reply.clone(), spec("Reply soon", "mail that needs an answer from me, not a newsletter", None), 3).unwrap();
    assert_eq!(at("beyond"), "503", "a fresh step would leave 503 again");
    rerun(&core, &reply);
    assert_eq!((at("total").as_str(), at("beyond").as_str()), ("500", "503"), "restarted at the newest, not continued");
}
