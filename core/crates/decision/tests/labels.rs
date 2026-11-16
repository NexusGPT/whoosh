//! The Labels page from the owner's label records (contract delta 9): fixed rules first, in position order and with their
//! match mode, then the label's meaning through the engine, then no label; an account's label only on that account; every
//! verdict names the rule or the model; the re-run after a label changed; the rule count that agrees with the filing.
//!
//! A140 (owner 2026-11-11, the three-step pipeline): a described label with its AI card on is AI-on, so its rule no longer files by
//! itself: it picks the mails the meaning is asked about. The verdicts here run through `decision::flow::decide_pipeline`; the tests
//! about the meaning use `open_labels` (entry cards off: step 1 lets every mail pass); each test changed for A140 says so in one line.
mod support;
use contracts::{ChainPlacement, DecidedBy, DecisionProviderKind, DecisionQuestion, MatchMode, Rule, RuleAction, RuleCondition, WhyCard};
use decision::flow::decide_pipeline;
use decision::{label_rule_count, DecideMessage, DecisionSettings, EntryKind, FileWrite, LabelNode, Outcome, OutcomeList, RerunAction, RerunLabel, Source, TabOption, Value, LABELS_MODEL_ID};
use smart_inbox::{MessageFacts, TabRef};
use std::sync::atomic::AtomicI64;
use support::*;

const TRUST: f64 = 0.7;

fn tabs() -> Vec<TabOption> {
    ["Primary", "Updates"].iter().map(|n| TabOption { tab: TabRef::Builtin(n.to_string()), name: n.to_string() }).collect()
}

/// Clients > Provanta (described), Invoices (described), Work only (acc-2's label, described), Clients (no description).
fn labels() -> Vec<LabelNode> {
    let clients = LabelNode::new("l-clients", "Clients", None);
    let mut provanta = LabelNode::new("l-provanta", "Provanta", Some("l-clients"));
    provanta.description = "mail about the Provanta account: their people, contracts and projects".into();
    let mut invoices = LabelNode::new("l-invoices", "Invoices", None);
    invoices.description = "bills and receipts we have to pay or file".into();
    let mut work = LabelNode::new("l-work", "Work only", None);
    work.description = "mail that belongs to the work mailbox alone".into();
    work.account_id = Some("acc-2".into());
    // delta 17: the nodes carry their chains (the rule records), so a verdict can name its chain
    LabelNode::with_chains(&[clients, provanta, invoices, work], &rules())
}

fn rule(id: &str, position: u32, label: &str, conditions: Vec<RuleCondition>, mode: MatchMode) -> Rule {
    // delta 17: every rule of this file is an entry chain, chain 0 of its label
    Rule { id: id.into(), position, conditions, tab_id: String::new(), enabled: true, action: RuleAction::Label { label_id: label.into() }, match_mode: mode, placement: ChainPlacement::Entry, chain_id: 0 }
}

/// r-provanta: domain provanta-example.com → Provanta · r-invoices: subject "invoice" AND an attachment → Invoices ·
/// r-work: domain work-example.com → Work only (acc-2's label).
fn rules() -> Vec<Rule> {
    vec![
        rule("r-provanta", 1, "l-provanta", vec![RuleCondition::Domain { domain: "provanta-example.com".into() }], MatchMode::Any),
        rule("r-invoices", 2, "l-invoices", vec![RuleCondition::SubjectContains { words: "invoice".into() }, RuleCondition::HasAttachment], MatchMode::All),
        rule("r-work", 3, "l-work", vec![RuleCondition::Domain { domain: "work-example.com".into() }], MatchMode::Any),
    ]
}

fn settings(trust: Option<f64>) -> DecisionSettings {
    let mut s = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    s.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_records(&labels(), &rules(), trust);
    s
}

/// The meaning: "Orbit" in the subject is Provanta (0.9); "bill" is Invoices (0.9); anything else is Clients at 0.2.
fn meaning() -> std::sync::Arc<FakeLocal> {
    FakeLocal::new(|_id, _q, state| {
        let subject = subject_in(state);
        if subject.contains("Orbit") {
            choice("Clients > Provanta", 0.9)
        } else if subject.contains("bill") {
            choice("Invoices", 0.9)
        } else {
            choice("Clients", 0.2)
        }
    })
}

/// A140: the same labels with every entry card off, so step 1 lets every mail pass and the meaning is asked about each.
fn open_labels() -> Vec<LabelNode> {
    labels().into_iter().map(|mut l| { l.entry_enabled = false; l }).collect()
}

fn labels_verdict(settings: &DecisionSettings, engine: &dyn contracts::DecisionPort, m: &MessageFacts) -> decision::Verdict {
    verdict_over(&labels(), settings, engine, m)
}

/// A140: the verdict of the three-step pipeline over `l`.
fn verdict_over(l: &[LabelNode], settings: &DecisionSettings, engine: &dyn contracts::DecisionPort, m: &MessageFacts) -> decision::Verdict {
    let t = tabs();
    let d = decide_pipeline(&DecideMessage { settings, engine: Some(engine), tabs: &t, labels: l }, m);
    d.get(Outcome::Labels).cloned().expect("the Labels page is on")
}

#[test]
fn a_fixed_rule_files_first_and_the_verdict_names_the_rule() {
    // A140: Provanta is AI-on, so its rule picks the message and the meaning rules on it (only Provanta is offered); the rule alone
    // files for an AI-off label (rules_run_by_label_order..., the rules-only page)
    let settings = settings(Some(TRUST));
    let engine = Recording::new(local(meaning()));
    let v = labels_verdict(&settings, &engine, &msg("m1", "ana@provanta-example.com", "Orbit kickoff", "see you Monday"));
    assert_eq!(v.value, Value::Label(Some("l-provanta".into())));
    assert_eq!((v.decided_by.entry_id.as_str(), v.decided_by.position), (LABELS_MODEL_ID, 1));
    assert!(matches!(v.decided_by.source, Source::Model { .. }));
    assert_eq!(v.decided_by.to_contract(), DecidedBy::Laya);
    assert_eq!(v.flow.as_ref().and_then(|fl| fl.entry_chain_id), Some(0), "step 1's chain is named");
    let calls = engine.calls.lock().unwrap();
    assert_eq!(calls.len(), 1, "one question");
    // fix-aisort round 3: in either form of the Labels question
    assert_eq!(decision::test_answers::offered_labels(&calls[0]), vec!["Clients > Provanta".to_string()], "only the label whose step 1 picked it is offered");
}

#[test]
fn a_message_no_rule_catches_is_filed_by_its_meaning_and_the_model_reads_each_label_meaning() {
    let settings = settings(Some(TRUST));
    let engine = Recording::new(local(meaning()));
    // A140: step 1 open (`open_labels`), so the meaning is asked about this message at all
    let v = verdict_over(&open_labels(), &settings, &engine, &msg("m2", "pm@partner-example.com", "Orbit status", "all green"));
    assert_eq!(v.value, Value::Label(Some("l-provanta".into())));
    assert_eq!(v.decided_by.entry_id, LABELS_MODEL_ID);
    // A140: the AI-on labels' rules are no rule entries, and Clients has none: the model entry comes first
    assert_eq!(v.decided_by.position, 1, "no rule entry, then the model entry");
    match &v.decided_by.source {
        Source::Model { provider, answer, probability } => assert_eq!((provider, answer.as_str(), *probability), (&DecisionProviderKind::Local, "Clients > Provanta", 0.9)),
        other => panic!("decided by {other:?}, not the model"),
    }
    assert_eq!(v.decided_by.to_contract(), DecidedBy::Laya);
    // the question sent: the sentence, then one line per described label visible to acc-1; the options are their paths
    let calls = engine.calls.lock().unwrap();
    let call = calls.iter().find(|c| decision::test_answers::asks_labels(c)).expect("the Labels question was sent");
    let (prompt, options) = (decision::test_answers::label_meanings(call).join("\n"), decision::test_answers::offered_labels(call));
    assert!(prompt.contains("Clients > Provanta: mail about the Provanta account"), "{prompt}");
    assert!(prompt.contains("Invoices: bills and receipts"), "{prompt}");
    assert!(!prompt.contains("Clients: "), "a label with no description adds no line: {prompt}");
    // A140: Clients carries no description, so it has no AI step and is not offered
    assert_eq!(options, vec!["Clients > Provanta".to_string(), "Invoices".to_string()], "Work only belongs to acc-2");
}

#[test]
fn below_the_trust_level_the_list_falls_through_to_no_label() {
    let settings = settings(Some(TRUST));
    let engine = local(meaning());
    let v = labels_verdict(&settings, &engine, &msg("m3", "pm@partner-example.com", "lunch?", ""));
    assert_eq!((v.value, v.decided_by.entry_id.as_str(), &v.decided_by.source), (Value::Label(None), "labels/default", &Source::Default));
}

#[test]
fn rules_run_by_label_order_and_a_rule_set_keeps_its_match_mode() {
    // two rules both match "both ways": the label higher in the sidebar wins (Provanta is above Invoices; lead ruling 2026-11-09),
    // whatever the list order; here the lower position agrees, tests/flow.rs holds the case where they disagree
    let both = vec![
        rule("r-late", 5, "l-invoices", vec![RuleCondition::SubjectContains { words: "both ways".into() }], MatchMode::Any),
        rule("r-early", 2, "l-provanta", vec![RuleCondition::SubjectContains { words: "both ways".into() }], MatchMode::Any),
        Rule { enabled: false, ..rule("r-off", 1, "l-provanta", vec![RuleCondition::SubjectContains { words: "both ways".into() }], MatchMode::Any) },
        rule("r-gone", 0, "l-gone", vec![RuleCondition::SubjectContains { words: "both ways".into() }], MatchMode::Any),
    ];
    let list = OutcomeList::labels_from_records(&labels(), &both, None);
    let ids: Vec<&str> = list.entries.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["rule:r-early", "rule:r-late", "labels/default"], "a switched-off rule and a rule for a label not in the tree file nothing");
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    settings.page_mut(Outcome::Labels).unwrap().list = list;
    let engine = local(meaning());
    let v = labels_verdict(&settings, &engine, &msg("m4", "x@y-example.com", "both ways", ""));
    assert_eq!((v.value, v.decided_by.entry_id.as_str(), v.decided_by.position), (Value::Label(Some("l-provanta".into())), "rule:r-early", 1));

    // match ALL: "invoice" without an attachment is not the rule's; the meaning answers (Invoices at 0.9 on "bill")
    let settings = settings_all();
    let v = labels_verdict(&settings, &engine, &msg("m5", "x@y-example.com", "invoice and bill", "no file"));
    // A140: Invoices is AI-on: an ALL chain with one condition unmet picks nothing, so the meaning is not asked
    assert_eq!(v.decided_by.entry_id, "labels/default", "an ALL rule with one condition unmet does not fire");
    let mut with_file = msg("m6", "x@y-example.com", "invoice and bill", "file attached");
    with_file.has_attachment = true;
    let v = labels_verdict(&settings, &engine, &with_file);
    // A140: the ALL chain picks it, and the meaning rules on it (Invoices at 0.9 on "bill")
    assert_eq!((v.decided_by.entry_id.as_str(), v.value), (LABELS_MODEL_ID, Value::Label(Some("l-invoices".into()))));
}

fn settings_all() -> DecisionSettings {
    settings(Some(TRUST))
}

fn labels_verdict_no_engine(settings: &DecisionSettings, m: &MessageFacts) -> decision::Verdict {
    let t = tabs();
    let l = labels();
    DecideMessage { settings, engine: None, tabs: &t, labels: &l }.execute(m).get(Outcome::Labels).cloned().expect("the Labels page is on")
}

#[test]
fn an_account_label_is_filed_and_offered_only_on_its_account() {
    let settings = settings(Some(TRUST));
    let engine = Recording::new(local(meaning()));
    // acc-1 (the helper's account): r-work's domain matches, the label is acc-2's: the rule does not fire, the model is asked without it
    let on_acc1 = msg("m7", "boss@work-example.com", "planning", "");
    let v = labels_verdict(&settings, &engine, &on_acc1);
    assert_ne!(v.decided_by.entry_id, "rule:r-work");
    assert_eq!(v.decided_by.entry_id, "labels/default", "Clients at 0.2 is below trust");
    // A140: on acc-1 no AI-on label's step 1 passes (Work only's is acc-2's): nothing is asked
    assert!(engine.ids().is_empty(), "{:?}", engine.ids());
    // acc-2: the rule picks it, and the meaning is asked about Work only alone (it answers Clients at 0.2: no label)
    let mut on_acc2 = on_acc1.clone();
    on_acc2.message_id = "m8".into();
    on_acc2.account_id = "acc-2".into();
    let v = labels_verdict(&settings, &engine, &on_acc2);
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Label(None), "labels/default"));
    let calls = engine.calls.lock().unwrap();
    let call = calls.iter().find(|c| decision::test_answers::asks_labels(c)).expect("asked on acc-2");
    let (options, prompt) = (decision::test_answers::offered_labels(call), decision::test_answers::label_meanings(call).join("\n"));
    assert_eq!(options, vec!["Work only".to_string()]);
    assert!(prompt.contains("Work only"), "{prompt}");
}

#[test]
fn the_model_entry_exists_only_with_a_trust_level_and_a_described_label_and_the_ledger_build_follows_the_row() {
    let rules_only = OutcomeList::labels_from_records(&labels(), &rules(), None);
    assert!(!rules_only.has_model_entry(), "no trust level: your rules only");
    assert_eq!(rules_only.entries.len(), 4, "three rules and the default");
    let with_meaning = OutcomeList::labels_from_records(&labels(), &rules(), Some(TRUST));
    let model = with_meaning.model_entries().next().expect("a model entry");
    // A140: the AI-on labels' rules are no rule entries (Clients, the AI-off one, has none): the model entry is first
    assert_eq!((model.0, model.1.id.as_str()), (0, LABELS_MODEL_ID));
    assert!(matches!(model.1.kind, EntryKind::Model { trust_from, .. } if trust_from == TRUST));
    let mute: Vec<LabelNode> = labels().into_iter().map(|mut l| { l.description.clear(); l }).collect();
    assert!(!OutcomeList::labels_from_records(&mute, &rules(), Some(TRUST)).has_model_entry(), "no described label: nothing to ask");
    // the ledger build carries the MODEL_TRUST_LABEL row's level (contracts delta 12), read, never retyped
    let level = decision::tuning::model_trust_label().expect("docs/CONSTANTS.md has MODEL_TRUST_LABEL");
    assert!((0.0..=1.0).contains(&level), "a fraction: {level}");
    let list = OutcomeList::labels_from_ledger(&labels(), &rules()).expect("built from the ledger");
    assert!(matches!(list.model_entries().next().map(|(_, e)| &e.kind), Some(EntryKind::Model { trust_from, .. }) if *trust_from == level));
}

#[test]
fn the_labels_model_entry_trusts_from_the_ledger_row() {
    let level = decision::tuning::model_trust_label().expect("docs/CONSTANTS.md has MODEL_TRUST_LABEL");
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    settings.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_ledger(&labels(), &rules()).unwrap();
    let at = |p: f64| {
        let engine = local(FakeLocal::new(move |_, _, _| choice("Clients > Provanta", p)));
        // A140: a Provanta sender, so Provanta' step 1 picks the message and the meaning is asked
        labels_verdict(&settings, &engine, &msg("m", "ana@provanta-example.com", "Orbit status", ""))
    };
    assert_eq!(at(level).value, Value::Label(Some("l-provanta".into())), "at the row's level the meaning files");
    assert_eq!(at(level - 0.01).decided_by.entry_id, "labels/default", "just below it the list falls through");
}

/// The filed set of Provanta before the re-run: m1 by rule, m2 and m3 by an older meaning; m4 is not filed yet.
fn filed() -> Vec<MessageFacts> {
    let mut m1 = msg("m1", "ana@provanta-example.com", "contract renewal", "");
    let mut m2 = msg("m2", "shop@store-example.com", "your bill for March", "");
    let mut m3 = msg("m3", "pm@partner-example.com", "Orbit status", "");
    let m4 = msg("m4", "pm@partner-example.com", "Orbit kickoff", "");
    for m in [&mut m1, &mut m2, &mut m3] {
        m.label_ids = vec!["l-provanta".into()];
    }
    vec![m1, m2, m3, m4]
}

#[test]
fn a_rerun_refiles_by_the_new_rules_and_meaning_and_every_row_names_who_decided() {
    let settings = settings(Some(TRUST));
    let engine = Recording::new(local(meaning()));
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let t = tabs();
    // A140: step 1 open (`open_labels`), so the meaning rules on every message
    let l = open_labels();
    let run = RerunLabel { settings: &settings, engine: Some(&engine), tabs: &t, labels: &l, label_id: "l-provanta".into(), clock: &clock, budget_ms: None };
    let out = run.execute(filed());
    assert_eq!(out.label_id, "l-provanta");
    assert!(!out.model_pending && out.engine_error.is_none() && out.not_reached.is_empty());
    let by_id = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id);
    let m1 = by_id("m1");
    // A140: Provanta is AI-on, so its rule no longer settles m1: the meaning says Clients at 0.2, below trust, and m1 leaves
    assert_eq!((m1.filed_today, m1.action), (true, RerunAction::Unfile));
    assert_eq!(m1.verdict.as_ref().unwrap().decided_by.entry_id, "labels/default");
    let m2 = by_id("m2");
    assert_eq!((m2.filed_today, m2.action), (true, RerunAction::Unfile));
    let v2 = m2.verdict.as_ref().unwrap();
    assert_eq!((&v2.value, v2.decided_by.entry_id.as_str()), (&Value::Label(Some("l-invoices".into())), LABELS_MODEL_ID), "the meaning now says Invoices: it leaves Provanta");
    let m3 = by_id("m3");
    assert_eq!((m3.filed_today, m3.action), (true, RerunAction::Keep));
    assert!(matches!(m3.verdict.as_ref().unwrap().decided_by.source, Source::Model { .. }), "kept by the meaning, decided_by updated to the model");
    let m4 = by_id("m4");
    assert_eq!((m4.filed_today, m4.action), (false, RerunAction::File));
    // A140: m1 is no longer settled by its rule: the four messages reach the engine
    assert_eq!(engine.calls.lock().unwrap().len(), 4, "every message reaches the engine once");
}

#[test]
fn with_the_engine_off_a_rerun_keeps_the_filing_of_what_only_the_meaning_can_judge() {
    let settings = settings(Some(TRUST));
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let t = tabs();
    // A140: step 1 open (`open_labels`): every message is the meaning's to judge
    let open = open_labels();
    let run = RerunLabel { settings: &settings, engine: None, tabs: &t, labels: &open, label_id: "l-provanta".into(), clock: &clock, budget_ms: None };
    let out = run.execute(filed());
    assert!(out.model_pending);
    let action = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id).action;
    // A140: Provanta is AI-on: its rule picks m1 for the meaning, which cannot run, so m1 keeps its filing (pending)
    assert_eq!(action("m1"), RerunAction::Pending, "the rule picks m1 for the meaning, which cannot run");
    assert_eq!((action("m2"), action("m3")), (RerunAction::Pending, RerunAction::Pending), "no rule, no meaning: the old filing stays");
    assert_eq!(action("m4"), RerunAction::Pending);
    assert!(out.rows.iter().filter(|r| r.action == RerunAction::Pending).all(|r| r.verdict.is_none()));

    // rules only (no described label asked): a rule miss leaves the label, as the contract's plan_rerun does
    let l = labels();
    let settings = settings_rules_only();
    let run = RerunLabel { settings: &settings, engine: None, tabs: &t, labels: &l, label_id: "l-provanta".into(), clock: &clock, budget_ms: None };
    let out = run.execute(filed());
    assert!(!out.model_pending);
    let action = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id).action;
    assert_eq!((action("m1"), action("m2"), action("m4")), (RerunAction::Keep, RerunAction::Unfile, RerunAction::Leave));
}

fn settings_rules_only() -> DecisionSettings {
    settings(None)
}

#[test]
fn a_rerun_is_bounded_and_lists_what_it_did_not_reach() {
    let settings = settings(Some(TRUST));
    let engine = local(meaning());
    let clock = StepClock { at: AtomicI64::new(0), step: 100 };
    let t = tabs();
    let l = labels();
    let run = RerunLabel { settings: &settings, engine: Some(&engine), tabs: &t, labels: &l, label_id: "l-provanta".into(), clock: &clock, budget_ms: Some(150) };
    let out = run.execute(filed());
    assert_eq!(out.rows.iter().map(|r| r.message_id.as_str()).collect::<Vec<_>>(), vec!["m1"]);
    assert_eq!(out.not_reached, vec!["m2".to_string(), "m3".into(), "m4".into()]);
}

#[test]
fn a_rerun_from_the_ledger_takes_its_bound_from_the_row() {
    let ms = decision::tuning::decision_rerun_ms().expect("docs/CONSTANTS.md has DECISION_RERUN_MS");
    assert!(ms > 0);
    let settings = settings(Some(TRUST));
    let engine = local(meaning());
    // the clock moves half the row per read: the first message starts inside the bound, the second does not
    let clock = StepClock { at: AtomicI64::new(0), step: (ms / 2) as i64 };
    let t = tabs();
    let l = labels();
    let run = RerunLabel::from_ledger(&settings, Some(&engine), &t, &l, "l-provanta".into(), &clock).expect("built from the ledger");
    assert_eq!(run.budget_ms, Some(ms));
    let out = run.execute(filed());
    assert_eq!((out.rows.len(), out.not_reached.len()), (1, 3));
}

#[test]
fn a_rerun_row_is_the_store_write_with_the_model_entry_only_when_the_model_decided() {
    let settings = settings(Some(TRUST));
    let engine = local(meaning());
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let t = tabs();
    // A140: step 1 open (`open_labels`), so the meaning rules on every message; the rule write comes from the rules-only page below
    let l = open_labels();
    let out = RerunLabel { settings: &settings, engine: Some(&engine), tabs: &t, labels: &l, label_id: "l-provanta".into(), clock: &clock, budget_ms: None }.execute(filed());
    let write = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id).file_write();
    // m1 by its rule: the rule id, no entry, no probability (A140: an AI-off page, the rules-only one, files by the rule alone)
    let rules_l = labels();
    let rules_only = settings_rules_only();
    let by_rule = RerunLabel { settings: &rules_only, engine: None, tabs: &t, labels: &rules_l, label_id: "l-provanta".into(), clock: &clock, budget_ms: None }.execute(filed());
    let rule_write = by_rule.rows.iter().find(|r| r.message_id == "m1").expect("m1").file_write();
    assert_eq!(rule_write, Some(FileWrite { label_id: "l-provanta".into(), message_id: "m1".into(), by: DecidedBy::Rule { rule_id: "rule:r-provanta".into() }, entry: None, probability: None, card: WhyCard::Entry, chain_id: Some(0) }));
    assert_eq!(write("m1"), None, "A140: on the AI-on Provanta the meaning says Clients at 0.2: no label to write");
    // m3 kept by the meaning, m4 filed by it: Laya, the entry labels/model, its probability
    for id in ["m3", "m4"] {
        assert_eq!(write(id), Some(FileWrite { label_id: "l-provanta".into(), message_id: id.into(), by: DecidedBy::Laya, entry: Some(LABELS_MODEL_ID.into()), probability: Some(0.9), card: WhyCard::Ai, chain_id: None }), "{id}");
    }
    // m2 moved to Invoices by the meaning: the write names Invoices (unfiling Provanta is the caller's)
    assert_eq!(write("m2").map(|w| (w.label_id, w.entry)), Some(("l-invoices".to_string(), Some(LABELS_MODEL_ID.to_string()))));
    // the engine off: a pending row writes nothing, and a default verdict carries no entry
    let out = RerunLabel { settings: &settings, engine: None, tabs: &t, labels: &l, label_id: "l-provanta".into(), clock: &clock, budget_ms: None }.execute(filed());
    assert_eq!(out.rows.iter().find(|r| r.message_id == "m2").unwrap().file_write(), None);
    let by_default = labels_verdict_no_engine(&settings_rules_only(), &msg("m9", "x@y-example.com", "nothing", ""));
    assert_eq!((by_default.decided_by.decided_entry_id(), by_default.decided_by.to_contract()), (None, DecidedBy::Default));
}

#[test]
fn the_rule_count_equals_what_the_rules_part_of_the_page_files() {
    let mut corpus = vec![
        msg("c1", "a@provanta-example.com", "hello", ""),
        msg("c2", "b@mail.provanta-example.com", "sub-domain", ""),
        msg("c3", "c@provanta-example.com", "three", ""),
        msg("c4", "d@other-example.com", "invoice", "no attachment"),
        msg("c5", "e@other-example.com", "invoice", "with attachment"),
    ];
    corpus[4].has_attachment = true;
    assert_eq!(label_rule_count(&rules(), "l-provanta", &corpus).count, 3);
    assert_eq!(label_rule_count(&rules(), "l-invoices", &corpus).count, 1, "ALL: subject and attachment");
    assert_eq!(label_rule_count(&rules(), "l-gone", &corpus).count, 0);
    let settings = settings(None);
    let filed_by_rule = corpus.iter().filter(|m| labels_verdict_no_engine(&settings, m).decided_by.entry_id == "rule:r-provanta").count();
    assert_eq!(filed_by_rule as u32, label_rule_count(&rules(), "l-provanta", &corpus).count, "the count and the filing read one function");
}

#[test]
fn a_from_rule_matches_the_sender_name_and_the_count_reads_unread_and_newest() {
    // contracts delta 12: the smart inbox's facts carry the sender's name and the unread bit; the bridge maps both
    let by_name = vec![rule("r-name", 1, "l-provanta", vec![RuleCondition::From { words: "Karen".into() }], MatchMode::Any)];
    let mut m1 = msg("n1", "k.bosch@other-example.com", "hello", "");
    m1.from_name = "Karen Bosch".into();
    m1.unread = true;
    m1.received_at_ms = 5_000;
    let mut m2 = msg("n2", "someone@other-example.com", "hello", "");
    m2.from_name = "Someone Else".into();
    m2.unread = true;
    let mut m3 = msg("n3", "karen@other-example.com", "by address", "");
    m3.unread = false;
    m3.received_at_ms = 9_000;
    let corpus = vec![m1, m2, m3];
    let count = label_rule_count(&by_name, "l-provanta", &corpus);
    assert_eq!((count.count, count.unread, count.newest_at_ms), (2, 1, Some(9_000)), "n1 by name, n3 by address; one unread; the newest of the two");
    let facts = decision::rule_facts(&corpus[0]);
    assert_eq!((facts.from_name.as_str(), facts.unread), ("Karen Bosch", true));
    // and arrival files it by the same rule
    let mut settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    settings.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_records(&labels(), &by_name, None);
    assert_eq!(labels_verdict_no_engine(&settings, &corpus[0]).decided_by.entry_id, "rule:r-name");
    assert_eq!(labels_verdict_no_engine(&settings, &corpus[1]).decided_by.entry_id, "labels/default");
}
