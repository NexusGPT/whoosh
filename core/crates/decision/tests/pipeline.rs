//! A140 (owner 2026-11-11: "If you have the three step. Step 1 does a pre-filter and select some emails (e.g., all unread), then step 2
//! if selected gets AI ruling on each selected emails, then step 3 if selected gets fixed ruling on all AI selected emails"; card "Yes,
//! build it like this"). A label is a pipeline: step 1 (entry chains) picks, step 2 (the AI, from the description) rules only on what
//! step 1 picked, step 3 (exit chains) filters what the AI accepted (what step 1 picked when the AI is off) and sends the rejected to
//! the fallback. Between labels the AI-off labels whose step 1 matched win first, in label order.
//!
//! Each test names what it fails on under the A94 order it replaces (an entry chain that matched filed the message straight in and the
//! model never read it; a flip re-asked the entry cards and left a filing whose entry chain no longer matched where it was).
mod support;
use contracts::{ChainPlacement, DecidedBy, DecisionPort, DecisionProviderKind, DecisionQuestion, MatchMode, MessageFact, Rule, RuleAction, RuleCondition, WhyCard};
use decision::flow::{decide_pipeline, step_one, StepOne};
use decision::{CurrentFiling, DecideMessage, DecisionSettings, FlipAction, FlowPlace, LabelNode, Outcome, OutcomeList, RefileOnFact, RerunAction, RerunLabel, Source, Value};
use smart_inbox::MessageFacts;
use std::sync::atomic::AtomicI64;
use support::*;

const TRUST: f64 = 0.7;
const UNREAD: &str = "l-unread";
const BILLS: &str = "l-bills";
const PAID: &str = "l-paid";

fn chain(id: &str, label: &str, placement: ChainPlacement, chain_id: u32, conditions: Vec<RuleCondition>) -> Rule {
    Rule { id: id.into(), position: chain_id, conditions, tab_id: String::new(), enabled: true, action: RuleAction::Label { label_id: label.into() }, match_mode: MatchMode::All, placement, chain_id }
}
fn unread_yes() -> RuleCondition {
    RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }
}
fn subject(words: &str) -> RuleCondition {
    RuleCondition::SubjectContains { words: words.into() }
}

/// "To read" (AI-on, described): step 1 = "is unread" (entry chain 0). "Bills" (AI-on, described): step 1 = subject "invoice".
/// "Paid" (AI-on unless a test switches it off, described): step 1 = subject "invoice" too. Exit chains are added by the tests.
fn rules(extra: Vec<Rule>) -> Vec<Rule> {
    let mut v = vec![
        chain("e-unread", UNREAD, ChainPlacement::Entry, 0, vec![unread_yes()]),
        chain("e-bills", BILLS, ChainPlacement::Entry, 0, vec![subject("invoice")]),
        chain("e-paid", PAID, ChainPlacement::Entry, 0, vec![subject("invoice")]),
    ];
    v.extend(extra);
    v
}

fn labels(edit: impl Fn(&mut Vec<LabelNode>), rules: &[Rule]) -> Vec<LabelNode> {
    let mut unread = LabelNode::new(UNREAD, "To read", None);
    unread.description = "mail I still have to read".into();
    let mut bills = LabelNode::new(BILLS, "Bills", None);
    bills.description = "bills I have to pay".into();
    let mut paid = LabelNode::new(PAID, "Paid", None);
    paid.description = "receipts of bills already paid".into();
    let mut v = vec![unread, bills, paid];
    edit(&mut v);
    LabelNode::with_chains(&v, rules)
}

fn settings(labels: &[LabelNode], rules: &[Rule]) -> DecisionSettings {
    let mut s = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::Labels]);
    s.page_mut(Outcome::Labels).unwrap().list = OutcomeList::labels_from_records(labels, rules, Some(TRUST));
    s
}

/// The model: answers the label named after "say:" in the subject at 0.9, else "To read" at 0.1 (below the trust level: a "no").
fn model() -> std::sync::Arc<FakeLocal> {
    FakeLocal::new(|_id, _q, state| {
        let s = subject_in(state);
        match s.split_once("say:") {
            Some((_, name)) => choice(name.trim(), 0.9),
            None => choice("To read", 0.1),
        }
    })
}

fn m(id: &str, subject: &str, unread: bool) -> MessageFacts {
    let mut f = msg(id, "ann@client-example.com", subject, "the body");
    f.unread = unread;
    f
}

fn decide(l: &[LabelNode], r: &[Rule], engine: Option<&dyn DecisionPort>, f: &MessageFacts) -> decision::Verdict {
    let s = settings(l, r);
    decide_pipeline(&DecideMessage { settings: &s, engine, tabs: &[], labels: l }, f).get(Outcome::Labels).cloned().expect("the Labels page is on")
}

/// The Labels questions the fake port received, as (subject of the message, options offered).
fn questions(engine: &Recording<decision::adapters::local::LocalDecision>) -> Vec<Vec<String>> {
    engine
        .calls
        .lock()
        .unwrap()
        .iter()
        // fix-aisort round 3: the Labels question in either form (one choice, or one yes/no per label)
        .filter(|call| decision::test_answers::asks_labels(call))
        .map(|call| decision::test_answers::offered_labels(call))
        .collect()
}

// ---------------------------------------------------------------- step 2 is asked only about what step 1 picked

#[test]
fn the_ai_is_asked_only_about_the_mails_step_1_picked() {
    // only "To read" is AI-on with a step 1 that can pass here; Bills and Paid need "invoice"
    let r = rules(vec![]);
    let l = labels(|_| {}, &r);
    let engine = Recording::new(local(model()));
    let mails = [m("a1", "hello say:To read", true), m("a2", "lunch say:To read", true), m("a3", "old news say:To read", false)];
    let verdicts: Vec<decision::Verdict> = mails.iter().map(|f| decide(&l, &r, Some(&engine), f)).collect();
    // A94 fails here: "is unread" filed a1 and a2 straight in (0 questions) and a3 was asked over every label (1 question)
    let asked = questions(&engine);
    assert_eq!(asked.len(), 2, "one question per mail step 1 picked, none for the read one: {asked:?}");
    assert!(asked.iter().all(|o| o == &vec!["To read".to_string()]), "only the label whose step 1 passed is offered: {asked:?}");
    for v in &verdicts[..2] {
        assert_eq!(v.value, Value::Label(Some(UNREAD.into())));
        assert!(matches!(v.decided_by.source, Source::Model { .. }), "the AI ruled: {:?}", v.decided_by);
        let fl = v.flow.as_ref().expect("a flow");
        assert_eq!((fl.card, fl.entry_chain_id, fl.ai_pick.as_deref()), (WhyCard::Ai, Some(0), Some(UNREAD)), "the AI's card, step 1's chain named");
    }
    assert_eq!((verdicts[2].value.clone(), verdicts[2].decided_by.source.clone()), (Value::Label(None), Source::Default), "the read mail never reaches the AI");
}

#[test]
fn a_step_1_match_with_the_ai_on_is_not_filed_when_the_ai_says_no() {
    let r = rules(vec![]);
    let l = labels(|_| {}, &r);
    let engine = Recording::new(local(model()));
    // unread: step 1 of "To read" picks it; the model answers "To read" at 0.1, under the trust level: a no
    let v = decide(&l, &r, Some(&engine), &m("n1", "a newsletter", true));
    // A94 fails here: the entry chain filed it under "To read" by the rule
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Label(None), "labels/default"));
    assert_eq!(questions(&engine).len(), 1, "the AI was asked, and said no");
    // and with no model at all, an AI-on label files nothing by its step 1 alone
    let v = decide(&l, &r, None, &m("n2", "a newsletter", true));
    assert_eq!(v.value, Value::Label(None));
}

// ---------------------------------------------------------------- the AI off: step 1 alone, then step 3

#[test]
fn with_the_ai_off_step_1_files_the_mail_then_step_3_applies() {
    // "To read" with its AI card off and an exit filter: kept while starred, the rest to Junk
    let r = rules(vec![chain("x-unread", UNREAD, ChainPlacement::Exit, 0, vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }])]);
    let l = labels(|v| { v[0].ai_enabled = false; v[0].exit_enabled = true; v[0].exit_fallback = Some("junk".into()); }, &r);
    let engine = Recording::new(local(model()));
    let mut starred = m("o1", "hello", true);
    starred.starred = true;
    let v = decide(&l, &r, Some(&engine), &starred);
    assert_eq!((v.value.clone(), v.decided_by.to_contract()), (Value::Label(Some(UNREAD.into())), DecidedBy::Rule { rule_id: "rule:e-unread".into() }));
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.place), (WhyCard::Entry, Some(0), FlowPlace::Label(UNREAD.into())), "step 1 filed it, step 3 kept it");
    // not starred: step 3 rejects what step 1 picked. A94 fails here: an entry filing never passed the label's exit filter
    let v = decide(&l, &r, Some(&engine), &m("o2", "hello", true));
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.place.clone(), fl.ai_pick.as_deref()), (WhyCard::Fallback, FlowPlace::Junk, Some(UNREAD)), "rejected by step 3, sent to the fallback");
    assert_eq!(v.decided_by.to_contract(), DecidedBy::Rule { rule_id: "rule:e-unread".into() }, "the rule decided; the AI never spoke");
    assert_eq!(fl.rejection("o2", 7).map(|r| (r.ai_label_id, r.sent_to)), Some((UNREAD.to_string(), "junk".to_string())));
    // a read mail: step 1 does not pick it, nothing files it
    assert_eq!(decide(&l, &r, Some(&engine), &m("o3", "hello", false)).value, Value::Label(None));
    assert!(questions(&engine).is_empty(), "no AI-on label's step 1 passed: the AI was never asked");
}

#[test]
fn a_label_with_no_entry_chain_and_the_ai_off_files_nothing() {
    let r: Vec<Rule> = vec![];
    let l = labels(|v| for n in v.iter_mut() { n.ai_enabled = false; }, &r);
    let engine = Recording::new(local(model()));
    let v = decide(&l, &r, Some(&engine), &m("p1", "say:To read", true));
    assert_eq!(v.value, Value::Label(None));
    assert!(questions(&engine).is_empty());
}

// ---------------------------------------------------------------- step 3 after the AI

#[test]
fn step_3_sends_what_it_rejects_among_the_ai_accepted_mails_to_the_fallback() {
    // "To read": step 1 unread, the AI, step 3 = starred, the rest to Primary
    let r = rules(vec![chain("x-unread", UNREAD, ChainPlacement::Exit, 0, vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }])]);
    let l = labels(|v| { v[0].exit_enabled = true; v[0].exit_fallback = Some("primary".into()); }, &r);
    let engine = Recording::new(local(model()));
    let v = decide(&l, &r, Some(&engine), &m("q1", "hello say:To read", true));
    // A94 fails here: the entry chain filed q1 under "To read" (card Entry) and the exit filter never ran
    assert_eq!(v.value, Value::Label(None), "Primary carries no label");
    let fl = v.flow.expect("a flow");
    assert_eq!((fl.card, fl.place.clone(), fl.ai_pick.as_deref(), fl.entry_chain_id), (WhyCard::Fallback, FlowPlace::Primary, Some(UNREAD), Some(0)));
    assert_eq!(v.decided_by.to_contract(), DecidedBy::Laya, "the AI accepted it; step 3 moved it");
    let mut starred = m("q2", "hello say:To read", true);
    starred.starred = true;
    let fl = decide(&l, &r, Some(&engine), &starred).flow.expect("a flow");
    assert_eq!((fl.card, fl.chain_id, fl.place), (WhyCard::Exit, Some(0), FlowPlace::Label(UNREAD.into())));
}

// ---------------------------------------------------------------- between labels

#[test]
fn of_two_labels_whose_step_1_matched_the_ai_off_one_wins_first() {
    // Bills (AI-on) sits above Paid (AI-off) in the sidebar; both step 1s match "invoice"
    let r = rules(vec![]);
    let l = labels(|v| v[2].ai_enabled = false, &r);
    let engine = Recording::new(local(model()));
    let v = decide(&l, &r, Some(&engine), &m("w1", "invoice 42 say:Bills", false));
    // A94 fails here: the entry rules went by sidebar order and Bills' rule won
    assert_eq!((v.value, v.decided_by.entry_id.as_str()), (Value::Label(Some(PAID.into())), "rule:e-paid"));
    assert!(questions(&engine).is_empty(), "an AI-off label's step 1 settled it: the AI is not asked");
    // no AI-off match: the AI chooses among the AI-on labels whose step 1 passed (Bills and "To read" for an unread invoice)
    let l = labels(|_| {}, &r);
    let engine = Recording::new(local(model()));
    let v = decide(&l, &r, Some(&engine), &m("w2", "invoice 43 say:Bills", true));
    assert_eq!(v.value, Value::Label(Some(BILLS.into())));
    assert_eq!(questions(&engine), vec![vec!["Bills".to_string(), "Paid".to_string(), "To read".to_string()]]);
}

// ---------------------------------------------------------------- a flipped fact

fn refile(l: &[LabelNode], r: &[Rule], f: &MessageFacts, now: CurrentFiling) -> FlipAction {
    let s = settings(l, r);
    RefileOnFact { settings: &s, tabs: &[], labels: l }.execute(f, &now)
}

#[test]
fn a_read_mail_leaves_a_label_whose_step_1_is_is_unread() {
    let r = rules(vec![]);
    // the AI accepted it while unread; the owner reads it
    let l = labels(|_| {}, &r);
    let now = CurrentFiling { place: FlowPlace::Label(UNREAD.into()), card: WhyCard::Ai, chain_id: None, ai_pick: Some(UNREAD.into()), decided_by: DecidedBy::Laya };
    let read = m("f1", "hello", false);
    assert_eq!(step_one(&l[0], &read), StepOne::Missed);
    // A94 fails here: the flip re-ran only the exit card of the AI's pick (off), so the message stayed (Keep)
    match refile(&l, &r, &read, now.clone()) {
        FlipAction::Move { to, decided_by } => {
            assert_eq!((to.card, to.place, to.ai_pick), (WhyCard::Hand, FlowPlace::Primary, None), "it leaves: Primary, no label, no rejection row");
            assert_eq!(decided_by, DecidedBy::Default);
        }
        other => panic!("expected the read mail to leave, got {other:?}"),
    }
    // unread again: step 1 passes, step 3 (off) keeps it where it is
    assert_eq!(refile(&l, &r, &m("f1", "hello", true), now), FlipAction::Keep);
    // the AI off: filed by step 1, read, it leaves too (A94 kept an entry filing whose chain no longer matched)
    let l = labels(|v| v[0].ai_enabled = false, &r);
    let by_rule = CurrentFiling { place: FlowPlace::Label(UNREAD.into()), card: WhyCard::Entry, chain_id: Some(0), ai_pick: None, decided_by: DecidedBy::Rule { rule_id: "rule:e-unread".into() } };
    match refile(&l, &r, &read, by_rule) {
        FlipAction::Move { to, .. } => assert_eq!((to.card, to.place), (WhyCard::Hand, FlowPlace::Primary)),
        other => panic!("expected the read mail to leave, got {other:?}"),
    }
}

#[test]
fn a_mail_that_starts_to_pass_step_1_later_is_named_for_the_ai_and_the_flip_never_asks_it() {
    let r = rules(vec![]);
    let l = labels(|_| {}, &r);
    let engine = Recording::new(local(model()));
    // arrival: read, so no step 1 passes and the AI is not asked
    let v = decide(&l, &r, Some(&engine), &m("g1", "hello say:To read", false));
    assert_eq!(v.value, Value::Label(None));
    assert!(questions(&engine).is_empty());
    // the owner marks it unread: "To read"'s step-1 chain picks it now. The flip names the label and asks nothing (it holds no engine);
    // the filing worker asks the model once (engine tests/label_flow_d17.rs). A94 fails here: the flip's entry cards filed it under
    // "To read" by the rule (Move, card Entry), with no AI
    let in_primary = CurrentFiling { place: FlowPlace::Primary, card: WhyCard::Hand, chain_id: None, ai_pick: None, decided_by: DecidedBy::Default };
    assert_eq!(refile(&l, &r, &m("g1", "hello say:To read", true), in_primary.clone()), FlipAction::AskAi { labels: vec![UNREAD.into()] });
    // an open step 1 (no chain) never makes a "first time": a label with no entry chain is not named
    let open = labels(|v| v[0].entry_enabled = false, &r);
    assert_eq!(refile(&open, &r, &m("g1", "hello", true), in_primary.clone()), FlipAction::Keep);
    // the owner's own filing never moves and never asks
    let by_owner = CurrentFiling { decided_by: DecidedBy::Owner, ..in_primary };
    assert_eq!(refile(&l, &r, &m("g1", "hello", true), by_owner), FlipAction::Keep);
}

// ---------------------------------------------------------------- the Re-run and the preview

#[test]
fn a_rerun_asks_the_ai_only_about_what_step_1_picks_and_is_pending_only_there() {
    let r = rules(vec![]);
    let l = labels(|_| {}, &r);
    let s = settings(&l, &r);
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let engine = Recording::new(local(model()));
    let mut filed_read = m("r1", "hello say:To read", false);
    filed_read.label_ids = vec![UNREAD.into()];
    let fresh = m("r2", "hello say:To read", true);
    let out = RerunLabel { settings: &s, engine: Some(&engine), tabs: &[], labels: &l, label_id: UNREAD.into(), clock: &clock, budget_ms: None }.execute(vec![filed_read.clone(), fresh.clone()]);
    let row = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id).clone();
    // A94 fails here: r2 was filed by the rule (Entry) without a question, and r1 was asked about over every label
    assert_eq!(row("r1").action, RerunAction::Unfile, "read: step 1 no longer picks it, it leaves");
    assert_eq!(row("r2").action, RerunAction::File);
    assert_eq!(row("r2").file_write().map(|w| (w.card, w.by)), Some((WhyCard::Ai, DecidedBy::Laya)));
    assert_eq!(questions(&engine).len(), 1, "only r2 reached the AI");
    // the engine off: r2 (picked by step 1) waits for the AI; r1 (not picked) is settled at once
    let out = RerunLabel { settings: &s, engine: None, tabs: &[], labels: &l, label_id: UNREAD.into(), clock: &clock, budget_ms: None }.execute(vec![filed_read, fresh]);
    let action = |id: &str| out.rows.iter().find(|r| r.message_id == id).expect(id).action;
    assert_eq!((action("r1"), action("r2")), (RerunAction::Unfile, RerunAction::Pending));
    assert!(out.model_pending);
}

#[test]
fn the_preview_runs_the_same_pipeline() {
    let r = rules(vec![]);
    let l = labels(|_| {}, &r);
    let s = settings(&l, &r);
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let engine = Recording::new(local(model()));
    let inputs = vec![
        decision::PreviewInput { facts: m("v1", "hello say:To read", true), current_tab: None },
        decision::PreviewInput { facts: m("v2", "hello say:To read", false), current_tab: None },
    ];
    let p = decision::PreviewDecisions { decide: DecideMessage { settings: &s, engine: Some(&engine), tabs: &[], labels: &l }, clock: &clock, tabs: None, budget_ms: 10_000 }.execute(inputs);
    let label_of = |id: &str| p.rows.iter().find(|r| r.message_id == id).and_then(|r| r.verdicts.iter().find(|v| v.outcome == Outcome::Labels)).map(|v| v.value.clone());
    // A94 fails here: v1 by the rule with no question, v2 asked over every label
    assert_eq!((label_of("v1"), label_of("v2")), (Some(Value::Label(Some(UNREAD.into()))), Some(Value::Label(None))));
    assert_eq!(questions(&engine).len(), 1, "only the unread one reached the AI");
}

// ---------------------------------------------------------------- why here

#[test]
fn why_here_names_step_1s_chain_and_the_ais_answer() {
    let r = rules(vec![]);
    let l = labels(|_| {}, &r);
    let f = m("y1", "hello say:To read", true);
    let v = decide(&l, &r, Some(&local(model())), &f);
    let fl = v.flow.as_ref().expect("a flow");
    let w = fl.why_here(&l, &f, v.decided_by.to_contract());
    // A94 fails here: an AI filing named no chain (card AI, chain None), and this one was an entry filing
    assert_eq!((w.card, w.chain_id, w.matched, w.decided_by), (WhyCard::Ai, Some(0), vec![unread_yes()], DecidedBy::Laya));
    let words = fl.steps_words(&v.decided_by.why());
    assert!(words.contains("step 1 picked it by entry chain 0") && words.contains("the model said To read (0.90)") && words.contains("step 3 is off"), "{words}");
}
