//! fn-split-tabs: split tabs as data. Edit and re-sort commit together; decided_by on every placement.
mod support;
use contracts::{DecisionProviderKind, SplitTab, SplitTabRule};
use smart_inbox::{EditSplitTabs, EditVips, TabEdit, VipEdit};
use std::collections::BTreeSet;
use support::*;

fn setup(tag: &str) -> (TempDb, serde_json::Value, store::Store) {
    let fx = fixture();
    let db = TempDb::new(tag);
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let store = seeded(&db, &fx, &model);
    (db, fx, store)
}

fn tab_names(store: &store::Store) -> Vec<(String, String, u32, bool)> {
    let conn = smart_inbox::load::open_reader(store.path()).unwrap();
    smart_inbox::load::load_tabs(&conn).unwrap().tabs.into_iter().map(|t| (t.id, t.name, t.order, t.enabled)).collect()
}

fn supplier_tab(fx: &serde_json::Value) -> SplitTab {
    SplitTab { id: "suppliers".into(), name: "Suppliers".into(), order: 6, rule: SplitTabRule::Domain { domain: s(fx, "supplier_domain") }, enabled: true }
}

#[test]
fn the_six_defaults_exist_at_first_launch_in_ledger_order() {
    let db = TempDb::new("defaults");
    let store = store::Store::open(&db.0).unwrap();
    let names: Vec<String> = tab_names(&store).into_iter().map(|t| t.1).collect();
    assert_eq!(names, store::default_tab_names());
    assert_eq!(names.len(), 6);
}

#[test]
fn renaming_a_tab_changes_no_placement() {
    let (_db, _fx, store) = setup("rename");
    let before = categories(&store);
    let uc = EditSplitTabs { store: &store };
    for (id, name) in [("updates", "Receipts"), ("primary", "Main"), ("vips", "People")] {
        let report = uc.execute(TabEdit::Rename { id: id.into(), name: name.into() }, 5).unwrap();
        assert!(report.moved.is_empty(), "renaming {id} moved {:?}", report.moved);
    }
    assert_eq!(categories(&store), before, "message_category is identical after three renames");
    let names: Vec<String> = tab_names(&store).into_iter().map(|t| t.1).collect();
    assert!(names.contains(&"Receipts".to_string()) && names.contains(&"Main".to_string()), "the names did change: {names:?}");
}

#[test]
fn renaming_before_the_mail_arrives_changes_no_placement() {
    let fx = fixture();
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let plain_db = TempDb::new("rename-base");
    let baseline = categories(&seeded(&plain_db, &fx, &model));
    let db = TempDb::new("rename-first");
    let renamed = seeded_after(&db, &fx, &model, |store| {
        let uc = EditSplitTabs { store };
        for (id, name) in [("updates", "Receipts"), ("promotions", "Offers"), ("social", "Friends"), ("vips", "People"), ("primary", "Main"), ("needs-reply", "Answer")] {
            uc.execute(TabEdit::Rename { id: id.into(), name: name.into() }, 1).unwrap();
        }
    });
    let after = categories(&renamed);
    assert_eq!(after.len(), baseline.len());
    let moved: Vec<&String> = after.iter().filter(|(id, r)| baseline[*id] != **r).map(|(id, _)| id).collect();
    assert!(moved.is_empty(), "with every default tab renamed, mail still lands in the same tabs; moved {moved:?}");
}

#[test]
fn adding_a_domain_tab_moves_exactly_that_domains_mail() {
    let (_db, fx, store) = setup("add-domain");
    let before = categories(&store);
    let supplier: BTreeSet<String> = strs(&fx, "supplier_message_ids").into_iter().collect();
    let report = EditSplitTabs { store: &store }.execute(TabEdit::Add(supplier_tab(&fx)), 5).unwrap();
    let after = categories(&store);
    let changed: BTreeSet<String> = after.iter().filter(|(id, r)| before.get(*id) != Some(r)).map(|(id, _)| id.clone()).collect();
    assert_eq!(changed, supplier, "exactly the fixture messages of {} change", s(&fx, "supplier_domain"));
    for id in &supplier {
        assert_eq!(after[id].tab, "suppliers");
        assert_eq!((after[id].decided_by.as_str(), after[id].rule_id.as_deref()), ("rule", Some("tab:suppliers")), "{id}: decided_by names the tab entry");
    }
    let moved: BTreeSet<String> = report.moved.iter().map(|m| m.message_id.clone()).collect();
    assert_eq!(moved, supplier, "the report lists exactly those messages");
    let counts = store.counts().unwrap();
    let t = counts.tabs.iter().find(|t| t.tab_id == "suppliers").expect("the new tab is in the counts");
    assert_eq!(t.unread as usize, supplier.len(), "the five supplier messages are unread");
}

#[test]
fn a_domain_tab_that_exists_before_the_mail_arrives_takes_exactly_that_domains_mail() {
    let fx = fixture();
    let model = FixtureModel::new(&fx, DecisionProviderKind::Jev);
    let db = TempDb::new("domain-first");
    let store = seeded_after(&db, &fx, &model, |store| {
        EditSplitTabs { store }.execute(TabEdit::Add(supplier_tab(&fx)), 1).unwrap();
    });
    let supplier: BTreeSet<String> = strs(&fx, "supplier_message_ids").into_iter().collect();
    let in_tab: BTreeSet<String> = categories(&store).into_iter().filter(|(_, r)| r.tab == "suppliers").map(|(id, _)| id).collect();
    assert_eq!(in_tab, supplier, "on arrival too, the tab takes {} and nothing else", s(&fx, "supplier_domain"));
}

#[test]
fn removing_a_tab_returns_its_messages_to_primary() {
    let (_db, fx, store) = setup("remove");
    let uc = EditSplitTabs { store: &store };
    uc.execute(TabEdit::Add(supplier_tab(&fx)), 5).unwrap();
    let supplier: BTreeSet<String> = strs(&fx, "supplier_message_ids").into_iter().collect();
    let report = uc.execute(TabEdit::Remove { id: "suppliers".into() }, 6).unwrap();
    let after = categories(&store);
    for id in &supplier {
        assert_eq!((after[id].tab.as_str(), after[id].decided_by.as_str()), ("primary", "default"), "{id} is back in Primary, decided by the default");
    }
    assert_eq!(report.moved.iter().map(|m| m.message_id.clone()).collect::<BTreeSet<_>>(), supplier);
    assert!(store.counts().unwrap().tabs.iter().all(|t| t.tab_id != "suppliers"));
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
    // Removing a default tab (VIPs) returns its mail to Primary as well.
    let vip_ids: Vec<String> = after.iter().filter(|(_, r)| r.tab == "vips").map(|(id, _)| id.clone()).collect();
    assert!(!vip_ids.is_empty());
    uc.execute(TabEdit::Remove { id: "vips".into() }, 7).unwrap();
    let after = categories(&store);
    for id in &vip_ids {
        assert_eq!(after[id].tab, "primary", "{id}");
    }
}

#[test]
fn a_split_inbox_keeps_at_least_one_tab_and_a_refused_edit_changes_nothing() {
    let (_db, _fx, store) = setup("last-tab");
    let before = (categories(&store), tab_names(&store));
    let uc = EditSplitTabs { store: &store };
    for id in ["needs-reply", "updates", "promotions", "social", "vips"] {
        uc.execute(TabEdit::SetEnabled { id: id.into(), enabled: false }, 5).unwrap();
    }
    let err = uc.execute(TabEdit::Remove { id: "primary".into() }, 6).unwrap_err();
    assert!(err.to_string().contains("at least one tab"), "{err}");
    let err = uc.execute(TabEdit::SetEnabled { id: "primary".into(), enabled: false }, 6).unwrap_err();
    assert!(err.to_string().contains("at least one tab"), "{err}");
    assert!(uc.execute(TabEdit::Rename { id: "nope".into(), name: "x".into() }, 6).is_err());
    assert!(uc.execute(TabEdit::Add(SplitTab { id: "primary".into(), name: "Dup".into(), order: 9, rule: SplitTabRule::Sender { address: "a@b".into() }, enabled: true }), 6).is_err());
    // Only the five disabled tabs differ from the start; the refused edits left no trace.
    let now = tab_names(&store);
    assert_eq!(now.iter().filter(|t| t.3).count(), 1);
    assert_eq!(now.len(), before.1.len());
    assert!(categories(&store).values().all(|r| r.tab == "primary"), "with one tab on, every message sits in it");
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

#[test]
fn disabling_and_enabling_a_tab_moves_mail_out_and_back() {
    let (_db, fx, store) = setup("toggle");
    let uc = EditSplitTabs { store: &store };
    uc.execute(TabEdit::Add(supplier_tab(&fx)), 5).unwrap();
    uc.execute(TabEdit::SetEnabled { id: "suppliers".into(), enabled: false }, 6).unwrap();
    let off = categories(&store);
    assert!(strs(&fx, "supplier_message_ids").iter().all(|id| off[id].tab == "primary"), "a disabled tab holds no mail");
    uc.execute(TabEdit::SetEnabled { id: "suppliers".into(), enabled: true }, 7).unwrap();
    let on = categories(&store);
    assert!(strs(&fx, "supplier_message_ids").iter().all(|id| on[id].tab == "suppliers"), "its rule takes the mail back when it is on again");
}

#[test]
fn reordering_tabs_moves_no_mail() {
    let (_db, _fx, store) = setup("reorder");
    let before = categories(&store);
    let report = EditSplitTabs { store: &store }.execute(TabEdit::Reorder { ids: vec!["vips".into(), "updates".into(), "primary".into()] }, 5).unwrap();
    assert!(report.moved.is_empty());
    assert_eq!(categories(&store), before);
    let order: Vec<String> = tab_names(&store).into_iter().map(|t| t.0).collect();
    assert_eq!(&order[..3], ["vips", "updates", "primary"]);
}

#[test]
fn a_tab_by_sender_account_label_or_decision_category() {
    let (_db, fx, store) = setup("kinds");
    let uc = EditSplitTabs { store: &store };
    // sender
    let sender = "person3@private3-example.net";
    let ids_from = |addr: &str| -> BTreeSet<String> { fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "from") == addr && s(m, "folder") == "INBOX").map(|m| s(m, "id")).collect() };
    let want = ids_from(sender);
    assert_eq!(want.len(), 1);
    let r = uc.execute(TabEdit::Add(SplitTab { id: "one-sender".into(), name: "One".into(), order: 6, rule: SplitTabRule::Sender { address: sender.into() }, enabled: true }), 5).unwrap();
    assert_eq!(r.moved.iter().map(|m| m.message_id.clone()).collect::<BTreeSet<_>>(), want);
    // label (a label the sync assigned)
    store.write(|ctx| {
        ctx.tx.execute("INSERT INTO label (id, name, colour, updated_at_ms) VALUES ('l-inv', 'Invoices', NULL, 0)", [])?;
        ctx.tx.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms) VALUES ('l-inv', 'm100', 0)", [])?;
        Ok(())
    }).unwrap();
    let r = uc.execute(TabEdit::Add(SplitTab { id: "inv".into(), name: "Invoices".into(), order: 7, rule: SplitTabRule::Label { label_id: "l-inv".into() }, enabled: true }), 6).unwrap();
    assert_eq!(r.moved.iter().map(|m| m.message_id.as_str()).collect::<Vec<_>>(), ["m100"]);
    // account: every message of that account (inbox) that is not the owner's own move
    let gm: BTreeSet<String> = fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "account") == "acc-gm" && s(m, "folder") == "INBOX").map(|m| s(m, "id")).collect();
    let r = uc.execute(TabEdit::Add(SplitTab { id: "gm".into(), name: "Gmail".into(), order: 8, rule: SplitTabRule::Account { account_id: "acc-gm".into() }, enabled: true }), 7).unwrap();
    let moved: BTreeSet<String> = r.moved.iter().map(|m| m.message_id.clone()).filter(|id| id != "m100").collect();
    assert!(gm.is_superset(&moved) && moved.len() >= gm.len() - 1, "the account tab takes that account's mail: moved {} of {}", moved.len(), gm.len());
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

#[test]
fn a_decision_category_tab_collects_what_the_model_names() {
    let (_db, fx, store) = setup("category");
    EditSplitTabs { store: &store }
        .execute(TabEdit::Add(SplitTab { id: "invoices".into(), name: "Invoices".into(), order: 6, rule: SplitTabRule::DecisionCategory { category: "Invoices".into() }, enabled: true }), 5)
        .unwrap();
    let port = RecordingPort { kind: DecisionProviderKind::Jev, calls: Default::default(), answer_choice: Some(("Invoices".into(), 0.95)) };
    let model = smart_inbox::ViaDecisionPort(&port);
    let mut inc = incoming(&fx["messages"].as_array().unwrap()[0]);
    inc.new.message.id = "new-1".into();
    inc.new.message.thread_id = "t-new-1".into();
    inc.new.message.from_address = "billing@vendor-example.com".into();
    inc.new.message.received_at_ms = fx["now_ms"].as_i64().unwrap();
    inc.headers = Default::default();
    inc.gmail_labels = vec![];
    inc.new.message.account_id = "acc-work".into();
    let p = smart_inbox::PlaceNewMessages { store: &store, model: &model }.execute(vec![inc], 9).unwrap();
    assert_eq!((p[0].tab_id.as_str(), format!("{:?}", p[0].decided_by).as_str()), ("invoices", "Jev"));
    let sent = port.calls.lock().unwrap();
    let options = sent.iter().find_map(|(_, qs)| qs.iter().find_map(|q| if let contracts::DecisionQuestion::Choice { options, .. } = q { Some(options.clone()) } else { None })).unwrap();
    assert!(options.contains(&"Invoices".to_string()) && options.contains(&"Updates".to_string()), "the question offers the owner's category: {options:?}");
}

#[test]
fn the_owners_own_move_survives_every_re_sort() {
    let (_db, fx, store) = setup("owner");
    let id = strs(&fx, "supplier_message_ids")[0].clone();
    let id2 = id.clone();
    store.write(move |ctx| {
        ctx.will_change(std::slice::from_ref(&id2))?;
        ctx.tx.execute("UPDATE message_category SET tab_id = 'social', decided_by = 'owner', rule_id = NULL, probability = NULL WHERE message_id = ?1", [&id2])?;
        Ok(())
    }).unwrap();
    EditSplitTabs { store: &store }.execute(TabEdit::Add(supplier_tab(&fx)), 5).unwrap();
    let row = &categories(&store)[&id];
    assert_eq!((row.tab.as_str(), row.decided_by.as_str()), ("social", "owner"), "a message the owner moved by hand stays where they put it");
}

#[test]
fn an_owner_rule_goes_first_and_names_itself() {
    let (_db, fx, store) = setup("rules");
    store.write(|ctx| {
        ctx.tx.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms) VALUES ('r1', 0, '[{\"kind\":\"subject\",\"words\":\"receipt\"},{\"kind\":\"domain\",\"domain\":\"svc0-example.com\"}]', 'social', 1, 0)", [])?;
        Ok(())
    }).unwrap();
    let want: Vec<String> = fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "subject").to_lowercase().contains("receipt") && s(m, "from").ends_with("svc0-example.com")).map(|m| s(m, "id")).collect();
    assert_eq!(want.len(), 1);
    let r = store.write(|ctx| smart_inbox::resort(ctx, &smart_inbox::Scope::Custom, 5)).unwrap();
    assert_eq!(r.moved.iter().map(|m| m.message_id.clone()).collect::<Vec<_>>(), want);
    let row = &categories(&store)[&want[0]];
    assert_eq!((row.tab.as_str(), row.decided_by.as_str(), row.rule_id.as_deref()), ("social", "rule", Some("rule:r1")));
}

#[test]
fn marking_and_unmarking_a_vip_moves_that_senders_mail() {
    let (_db, fx, store) = setup("vip-edit");
    let uc = EditVips { store: &store };
    let from = "person0@private0-example.net";
    let ids: Vec<String> = fx["messages"].as_array().unwrap().iter().filter(|m| s(m, "from") == from).map(|m| s(m, "id")).collect();
    assert_eq!(categories(&store)[&ids[0]].tab, "primary");
    let moved = uc.execute(VipEdit::MarkAddress(from.into()), 5).unwrap();
    assert_eq!(moved, ids);
    let row = &categories(&store)[&ids[0]];
    assert_eq!((row.tab.as_str(), row.rule_id.as_deref()), ("vips", Some("which-tab/vip")));
    assert_eq!(uc.execute(VipEdit::UnmarkAddress(from.into()), 6).unwrap(), ids);
    let row = &categories(&store)[&ids[0]];
    assert_eq!((row.tab.as_str(), row.decided_by.as_str()), ("primary", "default"));
    // A domain
    assert!(!uc.execute(VipEdit::MarkDomain("@private1-example.net".into()), 7).unwrap().is_empty());
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

fn label_tab(id: &str, label: &str, order: u32) -> SplitTab {
    SplitTab { id: id.into(), name: id.into(), order, rule: SplitTabRule::Label { label_id: label.into() }, enabled: true }
}

#[test]
fn a_label_rule_matches_the_label_and_every_descendant_and_records_the_exact_node() {
    // Labels are a tree (client > Provanta > Orbit > Q4, client > Vodafone); the fixture models the path until the contract's
    // Label row carries parentId/path/depth. A rule names a node and matches a message carrying that node or anything below it.
    let (_db, fx, store) = setup("label-tree");
    let on = |k: &str| s(&fx["label_messages"], k);
    let uc = EditSplitTabs { store: &store };
    // client > Provanta: the exact label, its child and its grandchild (depth 3 and 4); not the sibling branch, not the parent.
    let r = uc.execute(TabEdit::Add(label_tab("provanta", "l-provanta", 6)), 5).unwrap();
    let moved: BTreeSet<String> = r.moved.iter().map(|m| m.message_id.clone()).collect();
    assert_eq!(moved, BTreeSet::from([on("exact"), on("child"), on("grandchild")]));
    let cats = categories(&store);
    assert_eq!(cats[&on("exact")].rule_id.as_deref(), Some("tab:provanta@l-provanta"));
    assert_eq!(cats[&on("child")].rule_id.as_deref(), Some("tab:provanta@l-orbit"), "decided_by records the exact node the message carries");
    assert_eq!(cats[&on("grandchild")].rule_id.as_deref(), Some("tab:provanta@l-q4"));
    assert_eq!(cats[&on("sibling")].tab, "primary");
    assert_eq!(cats[&on("parent")].tab, "primary");
}

#[test]
fn a_label_rule_on_the_root_takes_the_whole_tree_and_a_leaf_rule_only_the_leaf() {
    let (_db, fx, store) = setup("label-root");
    let on = |k: &str| s(&fx["label_messages"], k);
    EditSplitTabs { store: &store }.execute(TabEdit::Add(label_tab("client", "l-client", 6)), 5).unwrap();
    let cats = categories(&store);
    for k in ["exact", "child", "grandchild", "sibling", "parent"] {
        assert_eq!(cats[&on(k)].tab, "client", "{k} sits under client");
    }
    assert_eq!(cats[&on("parent")].rule_id.as_deref(), Some("tab:client@l-client"));
    let (_db2, fx2, store2) = setup("label-leaf");
    let on2 = |k: &str| s(&fx2["label_messages"], k);
    EditSplitTabs { store: &store2 }.execute(TabEdit::Add(label_tab("q4only", "l-q4", 6)), 5).unwrap();
    let cats = categories(&store2);
    assert_eq!(cats[&on2("grandchild")].tab, "q4only");
    for k in ["exact", "child", "sibling", "parent"] {
        assert_eq!(cats[&on2(k)].tab, "primary", "{k} is above or beside the leaf, so it does not match");
    }
    assert_eq!(store2.counts().unwrap(), store2.recount().unwrap());
}
