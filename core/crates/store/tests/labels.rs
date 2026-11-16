//! Typed label operations against the real database rules (migration 0002): path and depth are derived by triggers,
//! a refusal comes back in words, and the descendant query finds a label's whole subtree and nothing else.
mod common;
/// the five built-in category labels migration 0004 seeds (delta 9): these tests count the owner's labels only
const BUILT_IN: [&str; 5] = ["primary", "needs-reply", "updates", "promotions", "social"];
use common::*;
use contracts::{CoreEvent, InboxQuery, StreamKind};
use std::sync::Arc;
use store::{NewLabel, Store, StoreError};

fn label(store: &Store, id: &str, name: &str, parent: Option<&str>) -> store::LabelRecord {
    store
        .label_create(NewLabel { id: Some(id.into()), name: name.into(), parent_id: parent.map(String::from), colour: None, account_ids: vec![], now_ms: 1 })
        .unwrap_or_else(|e| panic!("creating {name}: {e}"))
}

fn words(e: StoreError) -> String {
    match e {
        StoreError::Invalid(w) => w,
        other => panic!("expected a refusal in words, got {other}"),
    }
}

fn paths(store: &Store) -> Vec<(String, u32)> {
    store.labels().unwrap().into_iter().filter(|l| !BUILT_IN.contains(&l.id.as_str())).map(|l| (l.path, l.depth)).collect()
}

#[test]
fn a_label_under_a_parent_gets_its_path_and_depth_from_the_database() {
    let db = TempDb::new("lab-create");
    let store = small_store(&db, 0);
    let clients = label(&store, "c", "Clients", None);
    assert_eq!((clients.path.as_str(), clients.depth), ("Clients", 1));
    let prox = label(&store, "p", "Provanta", Some("c"));
    assert_eq!((prox.path.as_str(), prox.depth, prox.parent_id.as_deref()), ("Clients > Provanta", 2, Some("c")));
    let rfp = label(&store, "r", "RFP", Some("p"));
    assert_eq!(rfp.path, "Clients > Provanta > RFP");
}

#[test]
fn refusals_come_back_in_words_not_as_database_errors() {
    let db = TempDb::new("lab-refuse");
    let store = small_store(&db, 0);
    label(&store, "c", "Clients", None);
    let create = |name: &str, parent: Option<&str>| store.label_create(NewLabel { id: None, name: name.into(), parent_id: parent.map(String::from), now_ms: 1, ..Default::default() });
    assert!(words(create("", None).unwrap_err()).contains("needs a name"));
    assert!(words(create("A > B", None).unwrap_err()).contains("cannot contain \">\""));
    assert!(words(create(" Padded", None).unwrap_err()).contains("start or end with a space"));
    assert!(words(create("Orphan", Some("nope")).unwrap_err()).contains("parent label nope does not exist"));
    assert_eq!(paths(&store), [("Clients".to_string(), 1)], "a refused create stores nothing");
}

#[test]
fn the_deepest_level_is_the_ledgers_and_one_more_is_refused_in_words() {
    let db = TempDb::new("lab-depth");
    let store = small_store(&db, 0);
    let max = constant("LABEL_DEPTH_MAX").expect("the ledger has LABEL_DEPTH_MAX") as usize;
    let mut parent: Option<String> = None;
    for level in 1..=max {
        let made = label(&store, &format!("l{level}"), &format!("Level {level}"), parent.as_deref());
        assert_eq!(made.depth as usize, level);
        parent = Some(made.id);
    }
    let too_deep = store.label_create(NewLabel { id: None, name: "One more".into(), parent_id: parent, now_ms: 1, ..Default::default() }).unwrap_err();
    let w = words(too_deep);
    assert!(w.contains(&format!("at most {max} levels deep")), "{w}");
    assert_eq!(store.labels().unwrap().into_iter().filter(|l| !BUILT_IN.contains(&l.id.as_str())).count(), max);
}

#[test]
fn rename_rewrites_the_paths_of_every_sub_label() {
    let db = TempDb::new("lab-rename");
    let store = small_store(&db, 0);
    label(&store, "c", "Clients", None);
    label(&store, "p", "Provanta", Some("c"));
    label(&store, "r", "RFP", Some("p"));
    label(&store, "x", "Clients 2", None);
    let renamed = store.label_rename("c".into(), "Customers".into(), 2).unwrap();
    assert_eq!(renamed.path, "Customers");
    assert_eq!(
        paths(&store),
        [("Clients 2".to_string(), 1), ("Customers".to_string(), 1), ("Customers > Provanta".to_string(), 2), ("Customers > Provanta > RFP".to_string(), 3)]
    );
    assert!(words(store.label_rename("c".into(), "Bad > Name".into(), 3).unwrap_err()).contains("cannot contain"));
    assert_eq!(store.labels().unwrap().iter().filter(|l| l.path.starts_with("Customers")).count(), 3, "a refused rename changes no path");
    assert!(words(store.label_rename("ghost".into(), "X".into(), 3).unwrap_err()).contains("does not exist"));
}

#[test]
fn move_goes_under_another_parent_or_to_the_root_and_takes_the_sub_labels_along() {
    let db = TempDb::new("lab-move");
    let store = small_store(&db, 0);
    label(&store, "c", "Clients", None);
    label(&store, "p", "Provanta", Some("c"));
    label(&store, "r", "RFP", Some("p"));
    label(&store, "w", "Work", None);
    let moved = store.label_move("p".into(), Some("w".into()), 2).unwrap();
    assert_eq!((moved.path.as_str(), moved.depth), ("Work > Provanta", 2));
    assert_eq!(store.labels().unwrap().into_iter().find(|l| l.id == "r").unwrap().path, "Work > Provanta > RFP");
    let root = store.label_move("p".into(), None, 3).unwrap();
    assert_eq!((root.path.as_str(), root.depth, root.parent_id), ("Provanta", 1, None));
    let r = store.labels().unwrap().into_iter().find(|l| l.id == "r").unwrap();
    assert_eq!((r.path.as_str(), r.depth), ("Provanta > RFP", 2));
}

#[test]
fn a_move_under_itself_or_past_the_deepest_level_is_refused_in_words_and_changes_nothing() {
    let db = TempDb::new("lab-move-bad");
    let store = small_store(&db, 0);
    let max = constant("LABEL_DEPTH_MAX").unwrap() as usize;
    label(&store, "c", "Clients", None);
    label(&store, "p", "Provanta", Some("c"));
    let before = paths(&store);
    assert!(words(store.label_move("c".into(), Some("p".into()), 2).unwrap_err()).contains("under itself or under one of its own sub-labels"));
    assert!(words(store.label_move("c".into(), Some("c".into()), 2).unwrap_err()).contains("under itself"));
    // A chain of max levels elsewhere: moving Clients (2 levels) under its last level would pass the limit.
    let mut parent: Option<String> = None;
    for level in 1..=max {
        parent = Some(label(&store, &format!("d{level}"), &format!("Deep {level}"), parent.as_deref()).id);
    }
    let with_deep = paths(&store);
    let w = words(store.label_move("c".into(), parent, 2).unwrap_err());
    assert!(w.contains(&format!("at most {max} levels deep")), "{w}");
    assert_eq!(paths(&store), with_deep, "a refused move changes no path");
    assert!(with_deep.starts_with(&before[..before.len().min(2)]));
}

#[test]
fn ensure_path_creates_the_missing_ancestors_and_leaves_nothing_behind_when_a_level_is_refused() {
    let db = TempDb::new("lab-ensure");
    let store = small_store(&db, 0);
    let made = store.label_ensure_path("Clients > Provanta > RFP".into(), vec![], 1).unwrap();
    assert_eq!(made.created.len(), 3, "all three levels were new");
    assert_eq!(made.label.path, "Clients > Provanta > RFP");
    let again = store.label_ensure_path("clients > provanta".into(), vec![], 2).unwrap();
    assert!(again.created.is_empty(), "the path match ignores case");
    assert_eq!(again.label.id, made.created[1]);
    // Level 4 and 5 fit, level 6 does not: the whole call is undone, levels 4 and 5 included.
    let before = paths(&store);
    let max = constant("LABEL_DEPTH_MAX").unwrap() as usize;
    let long = (1..=max + 1).map(|i| format!("Deep {i}")).collect::<Vec<_>>().join(" > ");
    assert!(words(store.label_ensure_path(long.clone(), vec![], 3).unwrap_err()).contains("levels deep"));
    assert_eq!(paths(&store), before, "a refused path creates none of its levels");
    // The case that needs the savepoint: a use case that keeps going after the refusal commits whatever the call left behind.
    let kept_going = store
        .write(move |ctx| {
            let refused = ctx.label_ensure_path(&long, &[], 4);
            assert!(refused.is_err());
            Ok(())
        })
        .is_ok();
    assert!(kept_going);
    assert_eq!(paths(&store), before, "levels 1 to 5 of the refused path must not survive a commit that follows the refusal");
    assert!(words(store.label_ensure_path("A >  > B".into(), vec![], 3).unwrap_err()).contains("every level"));
}

#[test]
fn assign_and_unassign_report_only_the_messages_that_changed() {
    let db = TempDb::new("lab-assign");
    let store = small_store(&db, 6);
    label(&store, "c", "Clients", None);
    let ids: Vec<String> = ["m000", "m001", "m002"].iter().map(|s| s.to_string()).collect();
    assert_eq!(store.label_assign("c".into(), ids.clone(), 5).unwrap(), ids);
    assert!(store.label_assign("c".into(), ids.clone(), 6).unwrap().is_empty(), "the second assign changes nothing");
    assert_eq!(store.label_unassign("c".into(), vec!["m001".into(), "m005".into()]).unwrap(), ["m001"]);
    assert_eq!(store.label_unassign("c".into(), vec!["m001".into()]).unwrap(), Vec::<String>::new());
    let under: Vec<String> = store.messages_under_label("c", 50).unwrap().into_iter().map(|r| r.message_id).collect();
    assert_eq!(under, ["m002", "m000"], "newest first");
}

#[test]
fn an_assignment_publishes_the_inbox_page_of_the_tab_it_changes_and_the_counts_stay_equal_to_a_recount() {
    let db = TempDb::new("lab-events");
    let store = small_store(&db, 12);
    store.counts().unwrap();
    label(&store, "c", "Clients", None);
    let c = Arc::new(Collector::default());
    store.subscribe(StreamKind::Inbox, c.clone());
    store.label_assign("c".into(), vec!["m000".into()], 5).unwrap();
    let events = c.take();
    let tabs: Vec<&str> = events.iter().filter_map(|e| if let CoreEvent::InboxChanged { tab_id, .. } = e { Some(tab_id.as_str()) } else { None }).collect();
    // delta 16: the All view lists every inbox row, so it is refreshed beside the touched tab
    assert_eq!(tabs, ["primary", "all"], "the tab of the labelled message, and the All view");
    let rows = store.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: Some("c".into()), mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    assert_eq!(rows.iter().map(|r| r.message_id.as_str()).collect::<Vec<_>>(), ["m000"]);
    assert_eq!(rows[0].label_names, ["Clients"]);
    assert_eq!(store.counts().unwrap(), store.recount().unwrap());
}

#[test]
fn messages_under_a_label_include_every_sub_label_and_no_look_alike() {
    let db = TempDb::new("lab-under");
    let store = small_store(&db, 12);
    label(&store, "c", "Clients", None);
    label(&store, "p", "Provanta", Some("c"));
    label(&store, "r", "RFP", Some("p"));
    label(&store, "x", "Clients 2", None);
    label(&store, "y", "Clients%", None);
    label(&store, "z", "Clients_", None);
    store.label_assign("c".into(), vec!["m000".into()], 1).unwrap();
    store.label_assign("p".into(), vec!["m001".into()], 1).unwrap();
    store.label_assign("r".into(), vec!["m002".into(), "m001".into()], 1).unwrap();
    store.label_assign("x".into(), vec!["m003".into()], 1).unwrap();
    store.label_assign("y".into(), vec!["m004".into()], 1).unwrap();
    store.label_assign("z".into(), vec!["m005".into()], 1).unwrap();
    let ids = |label: &str, limit: usize| store.messages_under_label(label, limit).unwrap().into_iter().map(|r| r.message_id).collect::<Vec<_>>();
    assert_eq!(ids("c", 50), ["m002", "m001", "m000"], "Clients, Provanta and RFP; m001 once although it carries two of them");
    assert_eq!(ids("p", 50), ["m002", "m001"]);
    assert_eq!(ids("r", 50), ["m002", "m001"]);
    assert_eq!(ids("c", 2), ["m002", "m001"], "the limit keeps the newest");
    assert_eq!(ids("x", 50), ["m003"]);
    assert_eq!(ids("ghost", 50), Vec::<String>::new());
}

#[test]
fn deleting_a_label_promotes_its_sub_labels_and_drops_its_assignments() {
    let db = TempDb::new("lab-delete");
    let store = small_store(&db, 4);
    label(&store, "c", "Clients", None);
    label(&store, "p", "Provanta", Some("c"));
    label(&store, "r", "RFP", Some("p"));
    store.label_assign("p".into(), vec!["m000".into()], 1).unwrap();
    assert!(store.label_delete("p".into()).unwrap());
    assert!(!store.label_delete("p".into()).unwrap(), "a second delete finds nothing");
    assert_eq!(paths(&store), [("Clients".to_string(), 1), ("RFP".to_string(), 1)], "RFP becomes a root and is re-pathed");
    assert!(store.messages_under_label("c", 50).unwrap().is_empty(), "the assignment went with its label");
    assert_eq!(store.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap()[0].label_names, Vec::<String>::new());
}
