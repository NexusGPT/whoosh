//! gap-decisions: `Command::SaveDecisionList` on the real core. A rule of the owner files under ANY node of the label tree (also one whose AI card is
//! off), the mail it catches is filed at once, a rule already on the node is not written twice, and a bad request writes nothing.
//!
//! Mutants (single edits, each to be seen red by the lead's run):
//! - D1 decision_list.rs `save`: the chain is never written (`if false && !held...`) -> a_rule_files_under_a_nested_node_with_its_ai_card_off
//! - D2 decision_list.rs `save`: the held-chain test removed (`!held.iter().any(..)` -> `true`) -> a_rule_already_on_the_node_is_not_written_twice
//! - D3 decision_list.rs `save`: the node check removed -> a_bad_request_is_refused_in_words_and_writes_nothing
//! - D4 rust_core.rs the `SaveDecisionList` arm: the re-run is not asked -> a_rule_files_under_a_nested_node_with_its_ai_card_off
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
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, key: &str, thread: &str, from: &str, subject: &str, at: i64) -> NewMessage {
    let id = format!("{account}:{key}");
    NewMessage {
        message: Message {
            id, thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(),
            to: vec![USER.into()], subject: subject.into(), preview: format!("Body of {subject}"),
            received_at_ms: at, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(key.rsplit(':').next().unwrap_or("1").to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
fn id_of(account: &str, key: &str) -> String { format!("{account}:{key}") }

fn spec(name: &str, parent: Option<String>) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: parent, colour: None, description: String::new(), scope: LabelScope::All, rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false }
}
fn subject_rule(words: &str) -> RuleSet { RuleSet { conditions: vec![RuleCondition::SubjectContains { words: words.into() }], match_mode: MatchMode::Any } }

fn setup() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), account, dir)
}
/// The built-in labels are described, so their AI cards would take mail the rules leave; these cases are about the owner's rule, so the built-ins' AI cards are off.
fn ai_off_for_builtins(core: &RustCore) {
    for id in ["needs-reply", "updates", "promotions", "social"] { core.run(Command::SetLabelCard { label_id: id.into(), card: FlowCard::Ai, enabled: false }).unwrap(); }
}
/// Work > Clients, with the AI card of Clients off: a plain folder of the tree, nested one level down.
fn tree(core: &RustCore) -> (String, String) {
    let work = core.store().label_create_full(spec("Work", None), 1).unwrap().id;
    let clients = core.store().label_create_full(spec("Clients", Some(work.clone())), 2).unwrap().id;
    core.run(Command::SetLabelCard { label_id: clients.clone(), card: FlowCard::Ai, enabled: false }).unwrap();
    (work, clients)
}
fn chains(core: &RustCore, label: &str) -> String { core.state(format!("label/{label}/chains/entry/count")).unwrap_or_default() }
fn list(page: &str, nodes: &[&str], rules: Vec<RuleSet>) -> Command { Command::SaveDecisionList { page_id: page.into(), node_ids: nodes.iter().map(|s| s.to_string()).collect(), rules } }

#[test]
fn a_rule_files_under_a_nested_node_with_its_ai_card_off() {
    let (core, a, _d) = setup();
    ai_off_for_builtins(&core);
    let (work, clients) = tree(&core);
    core.store().insert_messages(vec![
        mail(&a, "INBOX:1", "t1", "ann@client.example", "Quote villa", 1_000),
        mail(&a, "INBOX:2", "t2", "bob@vendor.example", "Hello", 1_100),
    ]).unwrap();
    core.file_stored_messages(&[id_of(&a, "INBOX:1"), id_of(&a, "INBOX:2")]).unwrap();
    assert_eq!(core.label_rerun_count(clients.clone()), 0, "no rule yet: nothing is filed under the nested node");
    core.run(list("labels", &[clients.as_str()], vec![subject_rule("villa")])).expect("the list is served");
    core.flush_filing();
    assert_eq!(core.label_rerun_count(clients.clone()), 1, "the rule's mail is filed under the node it names");
    assert_eq!(core.label_rerun_count(work), 0, "and under no other node");
    assert_eq!(chains(&core, &clients), "1");
    let f = core.state(format!("message/{}/filing", id_of(&a, "INBOX:1"))).unwrap_or_default();
    assert!(f.contains(&format!("\"place\":\"{clients}\"")) && f.contains("\"card\":\"entry\""), "{f}");
}

#[test]
fn a_rule_already_on_the_node_is_not_written_twice() {
    let (core, _a, _d) = setup();
    ai_off_for_builtins(&core);
    let (_work, clients) = tree(&core);
    core.run(list("labels", &[clients.as_str()], vec![subject_rule("villa")])).unwrap();
    core.run(list("labels", &[clients.as_str()], vec![subject_rule("villa")])).unwrap();
    assert_eq!(chains(&core, &clients), "1", "the same rule saved twice is one chain");
    core.run(list("labels", &[clients.as_str()], vec![subject_rule("quote")])).unwrap();
    assert_eq!(chains(&core, &clients), "2", "another rule on the node is a second chain");
}

#[test]
fn a_bad_request_is_refused_in_words_and_writes_nothing() {
    let (core, _a, _d) = setup();
    ai_off_for_builtins(&core);
    let (_work, clients) = tree(&core);
    let unknown = core.run(list("labels", &[clients.as_str(), "no-such-label"], vec![subject_rule("villa"), subject_rule("villa")]));
    assert!(matches!(unknown, Err(CoreError::NotFound { .. })), "{unknown:?}");
    assert_eq!(chains(&core, &clients), "0", "one unknown node and the good one is not written either");
    let page = core.run(list("which-tab", &[clients.as_str()], vec![subject_rule("villa")]));
    assert!(matches!(page, Err(CoreError::Invalid { .. })), "{page:?}");
    let lengths = core.run(list("labels", &[clients.as_str()], vec![]));
    assert!(matches!(lengths, Err(CoreError::Invalid { .. })), "{lengths:?}");
    let empty = core.run(list("labels", &[clients.as_str()], vec![RuleSet { conditions: vec![], match_mode: MatchMode::Any }]));
    assert!(empty.is_err(), "a rule with no condition is refused, as the chain command refuses it");
    assert_eq!(chains(&core, &clients), "0");
}
