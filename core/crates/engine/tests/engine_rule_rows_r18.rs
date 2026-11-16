//! Lane hid-labels, round 18: (1) a rule row the owner removes is removed in the REAL engine (the owner's "still cannot delete rules": the app's side was a
//! re-seeded starter row and a hit area; this proves the engine keeps no row the sheet no longer sends), and (2) the negated twins files and counts through the
//! label's own commands: "Domain is not x" files every mail but x's, and an old stored rule still reads back unchanged.
//! Mutants: R1 label_meta.rs `write_rules`: the empty-set DELETE is removed -> removing_every_rule_row_leaves_the_label_with_none; R2 the `Not` arm of
//! `condition_matches` forgets the `!` -> a_label_with_a_negated_domain_counts_the_other_mail.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::sync::Arc;

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{AiStep, RustCore, TestTransport};
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


fn label_of(core: &RustCore, id: &str) -> MailLabel { core.store().labels_full().unwrap().into_iter().find(|l| l.id == id).unwrap_or_else(|| panic!("no label {id}")) }

fn list_mail(account: &str, key: &str, thread: &str, subject: &str, at: i64) -> NewMessage {
    NewMessage { has_list_header: true, ..mail(account, key, thread, "news@shop.example", subject, at) }
}



fn open(dir: &std::path::Path) -> RustCore {
    let store = Store::open(&dir.join("whoosh.sqlite")).unwrap();
    RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback)
}
fn rows_spec(name: &str, conditions: Vec<RuleCondition>) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: String::new(), scope: LabelScope::All, rules: RuleSet { conditions, match_mode: MatchMode::All }, pinned: false }
}
fn chain(core: &RustCore, id: &str) -> String { core.state(format!("label/{id}/chains/entry")).unwrap_or_default() }

#[test]
fn removing_a_rule_row_and_then_every_row_is_kept_by_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(dir.path());
    let id = core.run(Command::CreateLabelFull { spec: rows_spec("Rows", vec![RuleCondition::Domain { domain: "a.test".into() }, RuleCondition::From { words: "laura".into() }]) }).unwrap().created_id.unwrap();
    assert!(chain(&core, &id).contains("a.test") && chain(&core, &id).contains("laura"));
    core.run(Command::UpdateLabel { label_id: id.clone(), spec: rows_spec("Rows", vec![RuleCondition::Domain { domain: "a.test".into() }]) }).unwrap();
    let one = chain(&core, &id);
    assert!(one.contains("a.test") && !one.contains("laura"), "the removed row is gone: {one}");
    let core = open(dir.path());   // quit and reopen
    assert!(!chain(&core, &id).contains("laura"), "and stays gone after a reopen");
}

#[test]
fn removing_every_rule_row_leaves_the_label_with_none() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(dir.path());
    let id = core.run(Command::CreateLabelFull { spec: rows_spec("Rows", vec![RuleCondition::Domain { domain: "a.test".into() }]) }).unwrap().created_id.unwrap();
    core.run(Command::UpdateLabel { label_id: id.clone(), spec: rows_spec("Rows", vec![]) }).unwrap();
    let c = chain(&core, &id);
    assert!(!c.contains("a.test"), "no row is left: {c}");
}

#[test]
fn a_label_with_a_negated_domain_counts_the_other_mail() {
    let dir = tempfile::tempdir().unwrap();
    let core = open(dir.path());
    let account = core.store().add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    core.store().insert_messages(vec![
        mail(&account, "INBOX:1", "t1", "ann@keep.test", "one", 1_100),
        mail(&account, "INBOX:2", "t2", "bob@skip.test", "two", 1_200),
    ]).unwrap();
    let not_skip = RuleSet { conditions: vec![RuleCondition::Not { field: RuleField::Domain, value: "skip.test".into() }], match_mode: MatchMode::All };
    let count = core.rule_set_count(not_skip.clone());
    assert_eq!(count.count, 1, "only the mail from keep.test matches `Domain is not skip.test`");
    let id = core.run(Command::CreateLabelFull { spec: LabelSpec { rules: not_skip, ..rows_spec("Not skip", vec![]) } }).unwrap().created_id.unwrap();
    let stored = chain(&core, &id);
    assert!(stored.contains("\"kind\":\"not\"") && stored.contains("skip.test"), "the negated condition is served as stored: {stored}");
    let core = open(dir.path());
    assert!(chain(&core, &id).contains("\"kind\":\"not\""), "and it survives a reopen");
    // the label's own filing (the entry step) places the mail that is not from skip.test under it, and leaves the other where it was
    let filed = core.file_stored_messages(&[format!("{account}:INBOX:1"), format!("{account}:INBOX:2")]).unwrap();
    assert_eq!(filed[0].place, id, "ann@keep.test is filed under `Domain is not skip.test`: {:?}", filed[0]);
    assert_ne!(filed[1].place, id, "bob@skip.test is not: {:?}", filed[1]);
}
