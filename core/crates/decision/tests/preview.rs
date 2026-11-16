//! The preview run on the smart-inbox fixture store: 20 recent messages, verdicts per page, message_category unchanged;
//! the DECISION_PREVIEW_MS bound; and arrival through the smart inbox's hook with only "Which tab" on.
mod support;
use contracts::DecisionProviderKind;
use decision::{DecideMessage, DecisionSettings, EngineAsk, Outcome, PreviewDecisions, PreviewInput, TabOption};
use smart_inbox::load::{facts_for, load_context, open_reader};
use smart_inbox::NoModel;
use std::sync::atomic::AtomicI64;
use support::si::{categories, fixture, seeded, TempDb};
use support::*;

fn fake() -> std::sync::Arc<FakeLocal> {
    FakeLocal::new(|id, _q, _s| match id {
        "which-tab" | "which-tab/model" => choice("Updates", 0.95),
        _ => noul(0.9),
    })
}

fn inputs(store: &store::Store) -> (Vec<PreviewInput>, smart_inbox::TabSet) {
    let conn = open_reader(store.path()).unwrap();
    let ctx = load_context(&conn).unwrap();
    let ids: Vec<String> = conn.prepare("SELECT id FROM message WHERE folder = 'INBOX'").unwrap().query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
    let today = categories(store);
    let facts = facts_for(&conn, &ctx, &ids).unwrap();
    (facts.into_iter().map(|f| PreviewInput { current_tab: today.get(&f.message_id).map(|r| r.tab.clone()), facts: f }).collect(), ctx.tabs)
}

#[test]
fn the_preview_returns_twenty_verdict_rows_and_writes_nothing() {
    let db = TempDb::new("decision-preview");
    let store = seeded(&db, &fixture(), &NoModel);
    let before = categories(&store);
    let (inputs, tabset) = inputs(&store);
    assert!(inputs.len() > decision::tuning::decision_preview_n());
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &Outcome::ALL);
    let engine = local(fake());
    let tabs = TabOption::from_tab_set(&tabset);
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let p = PreviewDecisions { decide: DecideMessage { settings: &settings, engine: Some(&engine), tabs: &tabs, labels: &[] }, clock: &clock, tabs: Some(&tabset), budget_ms: 1 }.execute(inputs);
    assert_eq!(p.rows.len(), decision::tuning::decision_preview_n());
    assert!(p.not_reached.is_empty());
    assert!(p.rows.iter().all(|r| r.verdicts.len() == Outcome::ALL.len() && r.differs_from_today.is_some()));
    assert!(p.rows.iter().any(|r| r.differs_from_today == Some(true)), "the model's Updates differs from today's tab somewhere");
    assert_eq!(categories(&store), before, "message_category is unchanged");
}

#[test]
fn the_bound_stops_the_preview_and_names_what_it_did_not_reach() {
    let db = TempDb::new("decision-bound");
    let store = seeded(&db, &fixture(), &NoModel);
    let (inputs, tabset) = inputs(&store);
    let settings = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::WhichTab]);
    let engine = local(fake());
    let tabs = TabOption::from_tab_set(&tabset);
    // each clock read moves 100 ms; a 500 ms bound starts fewer than 20 messages
    let clock = StepClock { at: AtomicI64::new(0), step: 100 };
    let p = PreviewDecisions { decide: DecideMessage { settings: &settings, engine: Some(&engine), tabs: &tabs, labels: &[] }, clock: &clock, tabs: Some(&tabset), budget_ms: 500 }.execute(inputs);
    assert!(!p.rows.is_empty() && p.rows.len() < decision::tuning::decision_preview_n());
    assert_eq!(p.rows.len() + p.not_reached.len(), decision::tuning::decision_preview_n());
}

#[test]
fn the_preview_bound_comes_from_the_ledger_and_is_refused_while_it_holds_no_number() {
    let settings = DecisionSettings::shipped(DecisionProviderKind::Rules, &[Outcome::WhichTab]);
    let clock = StepClock { at: AtomicI64::new(0), step: 0 };
    let r = PreviewDecisions::from_ledger(DecideMessage { settings: &settings, engine: None, tabs: &[], labels: &[] }, &clock, None);
    match decision::tuning::decision_preview_ms() {
        Some(ms) => assert_eq!(r.ok().map(|p| p.budget_ms), Some(ms)),
        None => assert!(matches!(r, Err(contracts::CoreError::Invalid { ref field, .. }) if field == "DECISION_PREVIEW_MS")),
    }
}

#[test]
fn at_arrival_only_switched_on_pages_ask_the_engine_and_the_row_names_it() {
    let fx = fixture();
    let only_tab = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::WhichTab]);
    let f1 = fake();
    let e1 = local(f1.clone());
    let db = TempDb::new("decision-arrival");
    let store = seeded(&db, &fx, &EngineAsk { settings: &only_tab, engine: Some(&e1) });
    let asked = f1.asked_ids();
    assert!(asked.iter().any(|i| i == "which-tab") && !asked.iter().any(|i| i == "needs-reply"), "only Which tab is asked: {asked:?}");
    let rows = categories(&store);
    assert!(rows.values().any(|r| r.decided_by == "laya"), "a model placement is written as laya");

    let both = DecisionSettings::shipped(DecisionProviderKind::Local, &[Outcome::WhichTab, Outcome::NeedsMyReply]);
    let f2 = fake();
    let e2 = local(f2.clone());
    let db2 = TempDb::new("decision-arrival-both");
    seeded(&db2, &fx, &EngineAsk { settings: &both, engine: Some(&e2) });
    assert!(f2.asked_ids().iter().any(|i| i == "needs-reply"), "switched on, Needs my reply is asked");
}
