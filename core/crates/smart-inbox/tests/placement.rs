//! fn-smart-inbox-rules: placement on fixtures/smart-inbox.json (labelled), through the real arrival path.
mod support;
use contracts::{DecidedBy, DecisionProviderKind};
use smart_inbox::{AskModel, DecisionList, DecisionPage, MessageFacts, NoModel, Placer, TabSet};
use std::collections::BTreeSet;
use support::*;

fn wrong_ids(fx: &serde_json::Value, model: &dyn AskModel, tag: &str) -> (usize, Vec<String>) {
    let db = TempDb::new(tag);
    let store = seeded(&db, fx, model);
    let cats = categories(&store);
    let labels = labelled(fx);
    let wrong: Vec<String> = labels.iter().filter(|(id, tab)| cats.get(id).map(|r| &r.tab) != Some(&tab_id_of_label(tab))).map(|(id, _)| id.clone()).collect();
    (labels.len(), wrong)
}
fn tab_id_of_label(label: &str) -> String {
    label.to_string()
}

#[test]
fn rules_alone_meet_the_ledger_bar_and_miss_exactly_the_model_cases() {
    let fx = fixture();
    let (n, wrong) = wrong_ids(&fx, &NoModel, "rules");
    let bar = constant_f64("CATEGORY_RULE_MIN");
    let accuracy = (n - wrong.len()) as f64 / n as f64;
    eprintln!("rules only: {} of {} match the label = {:.4} (bar {bar}); misses {:?}", n - wrong.len(), n, accuracy, wrong);
    assert!(accuracy >= bar, "rule placement {accuracy:.4} is under CATEGORY_RULE_MIN {bar}; misses {wrong:?}");
    // The measure can fail: the messages only a model can place are the ones it misses, and no others.
    let needs: BTreeSet<String> = strs(&fx["expected"], "needs_model").into_iter().collect();
    assert_eq!(wrong.iter().cloned().collect::<BTreeSet<_>>(), needs, "rules must miss exactly the labelled model-only messages");
}

#[test]
fn with_the_model_every_label_matches() {
    let fx = fixture();
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let (n, wrong) = wrong_ids(&fx, &model, "model");
    eprintln!("rules + model: {} of {}", n - wrong.len(), n);
    assert!(wrong.is_empty(), "misplaced with the model on: {wrong:?}");
}

#[test]
fn every_placement_names_who_decided() {
    let fx = fixture();
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let db = TempDb::new("decided");
    let store = seeded(&db, &fx, &model);
    let cats = categories(&store);
    let inbox: Vec<&serde_json::Value> = fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "folder") == "INBOX").collect();
    assert!(!inbox.is_empty());
    let (mut rules, mut models, mut defaults) = (0, 0, 0);
    for m in &inbox {
        let id = s(m, "id");
        let row = cats.get(&id).unwrap_or_else(|| panic!("{id} has no message_category row"));
        assert!(!row.decided_by.is_empty(), "{id}: decided_by is empty");
        match row.decided_by.as_str() {
            "rule" => {
                rules += 1;
                let rid = row.rule_id.clone().unwrap_or_default();
                assert!(rid.starts_with("which-tab/") || rid.starts_with("rule:") || rid.starts_with("tab:"), "{id}: rule_id {rid:?} names no entry of the Which tab list");
                assert_ne!(rid, "which-tab/model");
                assert_ne!(rid, "which-tab/default");
            }
            "jev" | "laya" => {
                models += 1;
                let p: f64 = row.probability.clone().expect("a model placement carries its probability").parse().unwrap();
                assert!(p >= smart_inbox::tuning::MODEL_TRUST_NEEDS_REPLY, "{id}: the model placed it at {p}, under every trust level");
            }
            "default" => {
                defaults += 1;
                assert!(row.rule_id.is_none() && row.probability.is_none(), "{id}: a default carries neither a rule nor a probability");
            }
            other => panic!("{id}: unexpected decided_by {other}"),
        }
    }
    assert_eq!(rules + models + defaults, inbox.len());
    assert_eq!(models, 7, "the seven labelled model-only messages are placed by the model");
    assert!(rules > 40 && defaults > 10, "rules {rules}, defaults {defaults}");
}

#[test]
fn a_model_answer_below_its_trust_level_lets_the_list_continue() {
    let fx = fixture();
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let db = TempDb::new("trust");
    let store = seeded(&db, &fx, &model);
    let cats = categories(&store);
    for (id, why) in fx["messages"].as_array().unwrap().iter().filter(|m| !m["model"].is_null() && !m["needs_model"].as_bool().unwrap()).map(|m| (s(m, "id"), s(m, "why"))) {
        let row = &cats[&id];
        assert_eq!((row.tab.as_str(), row.decided_by.as_str()), ("primary", "default"), "{id} ({why}): an unsure model must not place the message");
    }
}

fn plain_facts() -> MessageFacts {
    MessageFacts { message_id: "x".into(), from_address: "a@b-example.com".into(), has_list_header: true, subject: "Flash sale: 40% off".into(), ..Default::default() }
}
fn tabs_default() -> TabSet {
    let db = TempDb::new("tabs");
    let store = store::Store::open(&db.0).unwrap();
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    smart_inbox::load::load_tabs(&conn).unwrap()
}

#[test]
fn the_model_entry_sits_where_the_owner_puts_it() {
    let tabs = tabs_default();
    struct Social;
    impl AskModel for Social {
        fn execute(&self, ask: &smart_inbox::ModelAsk<'_>) -> Option<smart_inbox::ModelVerdict> {
            matches!(ask.page, DecisionPage::WhichTab).then(|| smart_inbox::ModelVerdict { answer: smart_inbox::ModelAnswer::Choice("Social".into()), probability: 0.99, by: DecisionProviderKind::Jev })
        }
    }
    let f = plain_facts();
    let shipped = Placer::new(&tabs, &[], &Social).place(&f);
    assert_eq!((shipped.tab_id.as_str(), &shipped.decided_by), ("promotions", &DecidedBy::Rule { rule_id: "which-tab/promo-list".into() }), "shipped: the list-header rules sit above the model");
    let mut moved = Placer::new(&tabs, &[], &Social);
    assert!(moved.which.move_model_to(0));
    let p = moved.place(&f);
    assert_eq!((p.tab_id.as_str(), &p.decided_by, p.entry_position), ("social", &DecidedBy::Jev, 1), "moved to the top, the model is asked first and its answer stands");
}

#[test]
fn the_engine_off_skips_every_model_entry_and_the_list_falls_through() {
    let tabs = tabs_default();
    let mut f = plain_facts();
    f.has_list_header = false;
    f.subject = "Hello".into();
    let p = Placer::new(&tabs, &[], &NoModel).place(&f);
    assert_eq!((p.tab_id.as_str(), &p.decided_by, p.entry_id.as_str()), ("primary", &DecidedBy::Default, "which-tab/default"));
    assert_eq!(DecisionList::shipped_which_tab().entries.last().unwrap().id, "which-tab/default", "the default closes the list");
}

#[test]
fn headers_are_read_from_raw_mail() {
    let h = smart_inbox::header_facts(b"From: a@b\r\nList-Id: Dev <dev.example.com>\r\nList-Unsubscribe: <mailto:u@x>,\r\n <https://x/u>\r\n\r\nList-Id: not a header\r\n");
    assert_eq!(h.list_id.as_deref(), Some("Dev <dev.example.com>"));
    assert!(h.has_list_unsubscribe && h.has_list_header);
    let none = smart_inbox::header_facts(b"From: a@b\r\nSubject: hi\r\n\r\nbody");
    assert!(!none.has_list_header && none.list_id.is_none());
}
