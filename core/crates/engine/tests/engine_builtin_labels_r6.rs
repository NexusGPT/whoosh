//! Lane labels-r5, round 6: the built-in labels are no longer empty shells (owner on main65: "need reply is seems like empty and not configured").
//! Migration 0004 created Needs Reply, Updates, Promotions and Social with no description and no rule, so on a new installation nothing sorted mail.
//! `Store::seed_builtin_labels` (run when the core opens) fills each shell once and never touches what the owner wrote. Real core, real store, no
//! provider, no model: the entry rules and the exit filter of the seeded labels run with no AI, and the AI card says why it cannot.
//!
//! Mutants (single edits; each seen red, logs in _captures/labels-r5/mutants.out; runner _captures/labels-r5/run-mutants.py):
//! - S1 label_meta.rs `builtin_seeds`: Promotions loses its mailing-list entry chain -> a_new_installation_has_the_builtin_labels_filled, without_a_model_...
//! - S2 label_meta.rs `seed_builtin`: the "no description" condition is dropped (the owner's words would be overwritten) -> what_the_owner_wrote_is_never_overwritten
//! - S3 rust_core.rs `assemble`: the seeding call is removed -> a_new_installation_has_the_builtin_labels_filled
//! - S4 label_meta.rs `seed_builtin`: the exit filter is not switched on -> a_new_installation_has_the_builtin_labels_filled
//! - S5 label_meta.rs `seed_builtin`: the "no chain" condition is dropped (an owner's rule would be doubled) -> what_the_owner_wrote_is_never_overwritten
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

#[test]
fn a_new_installation_has_the_builtin_labels_filled() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let store = core.store();
    // Needs Reply: the approved example (docs/design/boards/labels-flow-r1)
    let nr = label_of(&core, "needs-reply");
    assert_eq!(nr.description, "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for.");
    assert!(nr.entry_enabled && nr.ai_enabled && nr.exit_enabled, "{nr:?}");
    assert_eq!(nr.exit_fallback.as_deref(), Some("primary"), "what the exit filter rejects goes to Primary");
    assert!(store.label_chains("needs-reply", ChainPlacement::Entry).unwrap().is_empty(), "no entry rule: the AI sorts it");
    let exit = store.label_chains("needs-reply", ChainPlacement::Exit).unwrap();
    assert_eq!(exit.len(), 1);
    assert_eq!(exit[0].rules.conditions, vec![RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }], "kept while still unanswered");
    // Promotions fills with no AI: a mailing list (List-Id or List-Unsubscribe) is caught by its entry rule
    let entry = store.label_chains("promotions", ChainPlacement::Entry).unwrap();
    assert_eq!(entry.len(), 1);
    assert_eq!(entry[0].rules.conditions, vec![RuleCondition::Fact { fact: MessageFact::MailingList, wanted: true }]);
    assert!(!label_of(&core, "promotions").description.is_empty());
    // A140, lead's default 2026-11-11: shipped with an entry chain, so shipped with its AI card off (step 1 alone files); Updates and Social keep theirs on
    assert!(!label_of(&core, "promotions").ai_enabled && label_of(&core, "updates").ai_enabled && label_of(&core, "social").ai_enabled);
    // Updates and Social: described for the AI, no deterministic rule
    for id in ["updates", "social"] {
        let l = label_of(&core, id);
        assert!(!l.description.trim().is_empty() && !l.exit_enabled, "{id}: {l:?}");
        assert!(store.label_chains(id, ChainPlacement::Entry).unwrap().is_empty());
    }
    // a second launch changes nothing
    assert_eq!(store.seed_builtin_labels(99).unwrap(), Vec::<String>::new(), "idempotent");
}

#[test]
fn what_the_owner_wrote_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    // the owner described Updates in their own words and gave Social a rule but no description
    store.label_update_full("updates".into(), spec("Updates", "my own words about updates", None), 5).unwrap();
    store.label_update_full("social".into(), spec("Social", "", Some("friends.example")), 5).unwrap();
    let filled = store.seed_builtin_labels(10).unwrap();
    assert_eq!(filled, vec!["needs-reply".to_string(), "promotions".to_string()], "only the empty shells are filled");
    let labels = store.labels_full().unwrap();
    let get = |id: &str| labels.iter().find(|l| l.id == id).unwrap().clone();
    assert_eq!(get("updates").description, "my own words about updates");
    assert_eq!(get("social").description, "", "an owner's rule makes the label theirs: no description is added");
    assert_eq!(store.label_chains("social", ChainPlacement::Entry).unwrap().len(), 1, "their one rule, not doubled");
    assert_eq!(store.label_chains("updates", ChainPlacement::Entry).unwrap().len(), 0);
    assert_eq!(store.seed_builtin_labels(11).unwrap(), Vec::<String>::new());
}

#[test]
fn without_a_model_the_seeded_labels_still_sort_and_the_ai_card_says_why() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    core.store().insert_messages(vec![
        list_mail(&account, "INBOX:1", "t1", "Weekly deals", 1_000),
        mail(&account, "INBOX:2", "t2", "ann@client.example", "Can you answer?", 1_100),
    ]).unwrap();
    let filed = core.file_stored_messages(&[id_of(&account, "INBOX:1"), id_of(&account, "INBOX:2")]).unwrap();
    let (promo, reply) = (&filed[0], &filed[1]);
    // the mailing-list mail is caught by Promotions' entry rule: no AI, no model
    // A140, lead's default 2026-11-11: a built-in label seeded with an entry chain ships with its AI card off, so its step 1 alone files it
    assert_eq!((promo.place.as_str(), promo.card, promo.chain_id, &promo.ai), ("promotions", Some(WhyCard::Entry), Some(0), &AiStep::EntryRule), "{promo:?}");
    // the question stays in Primary: Needs Reply is the AI's, and there is no model
    assert_eq!((reply.place.as_str(), &reply.ai), ("primary", &AiStep::NoProvider), "{reply:?}");
    let counts = core.state("label/needs-reply/flow/counts".into()).unwrap();
    assert!(counts.contains("\"caught_by_entry\":0") && counts.contains("\"sorted_by_ai\":0") && counts.contains("\"kept_by_exit\":0"), "{counts}");
    // and it SAYS why
    let why = core.state("label/needs-reply/flow/ai_refusal".into()).unwrap();
    assert!(why.contains("ai_no_model") && why.contains("No model is set for sorting mail: choose one in Settings, Decisions."), "{why}");
    assert_eq!(core.label_rerun_count("promotions".into()), 1);
    assert_eq!(core.label_rerun_count("needs-reply".into()), 0);
}
