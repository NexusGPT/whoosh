//! Delta 16 probes (docs/PRD.md "A91 shape"): Primary and All are views, not labels; Unread is the filter on any tab.
//! Each test names the rule it holds. MUTANTS (the lead runs each at the build word, the test named must go red):
//!   M1 `view_contains` All stops excluding Junk              -> all_excludes_junk_and_trash_and_everything_not_in_the_inbox
//!   M2 `view_contains` All stops excluding Trash             -> all_excludes_junk_and_trash_and_everything_not_in_the_inbox
//!   M3 Primary reads `category_id == "primary"` only         -> primary_is_the_inbox_mail_filed_into_no_other_category
//!   M4 Primary admits a message filed in Updates             -> primary_is_the_inbox_mail_filed_into_no_other_category
//!   M5 `inbox_tabs` puts All before Primary                  -> the_tab_list_is_primary_all_then_the_pinned_labels_in_tree_order
//!   M6 `inbox_tabs` keeps a legacy "primary" label row       -> the_tab_list_is_primary_all_then_the_pinned_labels_in_tree_order
//!   M7 `check_view_command` skips PinLabel                   -> a_view_cannot_be_applied_removed_renamed_moved_pinned_unpinned_deleted_or_re_run
//!   M8 `check_view_command` skips RemoveLabel                -> a_view_cannot_be_applied_removed_renamed_moved_pinned_unpinned_deleted_or_re_run
//!   M9 `check_view_command` lets a label be created as "All" -> a_top_level_label_cannot_be_named_like_a_view_but_a_nested_one_can
//!   M10 `count_view` counts members of the wrong view       -> a_views_counts_are_its_members_unread_and_waiting_and_all_is_never_below_primary
//!   M11 the refusal drops its code                           -> every_refusal_carries_the_code_and_a_sentence_naming_the_view
//!   M12 `check_tab_order` lets a view after a label by       -> a_split_tab_order_keeps_the_views_first_in_their_order_and_every_other_order_is_the_stores_to_judge
//!   M13 `check_tab_order` lets the views out of their order  -> a_split_tab_order_keeps_the_views_first_in_their_order_and_every_other_order_is_the_stores_to_judge
use contracts::*;

fn facts(category: &str) -> ViewFacts { ViewFacts { category_id: category.into(), in_inbox: true, junk: false, trashed: false } }
fn msg(category: &str, unread: bool, waiting: bool) -> ViewMessage { ViewMessage { facts: facts(category), unread, awaiting_reply: waiting } }

fn label(id: &str, name: &str, order: u32, pinned: bool, kind: LabelKind) -> MailLabel {
    MailLabel { id: id.into(), name: name.into(), parent_id: None, colour: None, account_ids: vec![], path: name.into(), depth: 1,
        unread: 0, description: String::new(), pinned, scope: LabelScope::All, kind, order, entry_enabled: true, ai_enabled: true, exit_enabled: false, ai_reads: default_ai_reads(), exit_fallback: None }
}
fn spec(name: &str, parent: Option<&str>) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: parent.map(String::from), colour: None, description: String::new(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: false }
}
fn refused(r: Result<(), CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("expected a refusal, got {other:?}") }
}

#[test]
fn the_views_are_primary_and_all_with_stable_ids_and_a_label_id_is_no_view() {
    assert_eq!(inbox_views(), vec![InboxView::Primary, InboxView::All]);
    assert_eq!((inbox_view_id(InboxView::Primary).as_str(), inbox_view_id(InboxView::All).as_str()), ("primary", "all"));
    assert_eq!(inbox_view_from_id("primary".into()), Some(InboxView::Primary));
    assert_eq!(inbox_view_from_id("all".into()), Some(InboxView::All));
    assert_eq!(inbox_view_from_id("needs-reply".into()), None);
    assert_eq!(inbox_view_from_id("clients".into()), None);
}

#[test]
fn all_excludes_junk_and_trash_and_everything_not_in_the_inbox() {
    for c in ["primary", "needs-reply", "updates", "promotions", "social", "vips", ""] {
        assert!(view_contains(InboxView::All, facts(c)), "All holds every category: {c:?}");
    }
    assert!(!view_contains(InboxView::All, ViewFacts { junk: true, ..facts("primary") }), "Junk is not in All");
    assert!(!view_contains(InboxView::All, ViewFacts { trashed: true, ..facts("updates") }), "Trash is not in All");
    assert!(!view_contains(InboxView::All, ViewFacts { in_inbox: false, ..facts("primary") }), "an archived or snoozed message is not in the inbox");
}

#[test]
fn primary_is_the_inbox_mail_filed_into_no_other_category() {
    assert!(view_contains(InboxView::Primary, facts("primary")));
    assert!(view_contains(InboxView::Primary, facts("")), "not filed yet reads as primary (the decision model's default)");
    for other in ["needs-reply", "updates", "promotions", "social", "vips", "invoices"] {
        assert!(!view_contains(InboxView::Primary, facts(other)), "{other} is another category");
    }
    assert!(!view_contains(InboxView::Primary, ViewFacts { junk: true, ..facts("primary") }));
    assert!(!view_contains(InboxView::Primary, ViewFacts { trashed: true, ..facts("primary") }));
    assert!(!view_contains(InboxView::Primary, ViewFacts { in_inbox: false, ..facts("primary") }));
}

#[test]
fn a_views_counts_are_its_members_unread_and_waiting_and_all_is_never_below_primary() {
    let corpus = vec![
        msg("primary", true, true), msg("primary", false, false), msg("", true, false),
        msg("updates", true, false), msg("needs-reply", true, true), msg("social", false, false),
        ViewMessage { facts: ViewFacts { junk: true, ..facts("primary") }, unread: true, awaiting_reply: true },
        ViewMessage { facts: ViewFacts { trashed: true, ..facts("updates") }, unread: true, awaiting_reply: false },
    ];
    let p = count_view(InboxView::Primary, corpus.clone());
    let a = count_view(InboxView::All, corpus);
    assert_eq!((p.tab_id.as_str(), p.unread, p.waiting), ("primary", 2, 1));
    assert_eq!((a.tab_id.as_str(), a.unread, a.waiting), ("all", 4, 2), "Junk and Trash unread never count in All");
    assert!(a.unread >= p.unread);
}

#[test]
fn the_tab_list_is_primary_all_then_the_pinned_labels_in_tree_order() {
    // a store seeded before delta 16 still holds a built-in "primary" label row, pinned: it must not show twice
    let labels = vec![
        label("primary", "Primary", 0, true, LabelKind::BuiltIn),
        label("promotions", "Promotions", 2, true, LabelKind::BuiltIn),
        label("needs-reply", "Needs Reply", 1, true, LabelKind::BuiltIn),
        label("social", "Social", 3, false, LabelKind::BuiltIn),
        label("clients", "Clients", 4, true, LabelKind::Owner),
        label("home", "Home", 5, false, LabelKind::Owner),
    ];
    let tabs = inbox_tabs(labels.clone());
    assert_eq!(tabs.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["primary", "all", "needs-reply", "promotions", "clients"]);
    assert_eq!(tabs.iter().map(|t| t.kind).collect::<Vec<_>>(), [InboxTabKind::View, InboxTabKind::View, InboxTabKind::Label, InboxTabKind::Label, InboxTabKind::Label]);
    assert_eq!(tabs[0].name, "Primary");
    assert_eq!(tabs[1].name, "All");
    // with no pinned label the strip is still Primary and All
    assert_eq!(inbox_tabs(vec![]).len(), 2);
    // the label list drops the legacy view row and nothing else
    let kept = labels_without_views(labels);
    assert_eq!(kept.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(), ["promotions", "needs-reply", "social", "clients", "home"]);
}

#[test]
fn a_view_cannot_be_applied_removed_renamed_moved_pinned_unpinned_deleted_or_re_run() {
    let ids = vec!["m1".to_string()];
    let doors: Vec<(&str, Command)> = vec![
        ("apply by name", Command::Label { message_ids: ids.clone(), label_name: "Primary".into(), create_if_missing: true }),
        ("apply by path", Command::Label { message_ids: ids.clone(), label_name: "All > Sub".into(), create_if_missing: true }),
        ("apply by move", Command::MoveToLabel { message_ids: ids.clone(), label_id: "all".into() }),
        ("remove", Command::RemoveLabel { message_ids: ids.clone(), label_id: "primary".into() }),
        ("rename", Command::RenameLabel { label_id: "primary".into(), name: "Main".into() }),
        ("move", Command::MoveLabel { label_id: "all".into(), parent_id: None }),
        ("move under", Command::MoveLabel { label_id: "clients".into(), parent_id: Some("all".into()) }),
        ("reorder", Command::ReorderLabel { label_id: "primary".into(), new_parent_id: None, new_index: 3 }),
        ("reorder under", Command::ReorderLabel { label_id: "clients".into(), new_parent_id: Some("primary".into()), new_index: 0 }),
        ("pin", Command::PinLabel { label_id: "all".into(), pinned: true }),
        ("unpin", Command::PinLabel { label_id: "primary".into(), pinned: false }),
        ("delete", Command::DeleteLabel { label_id: "primary".into() }),
        ("re-run", Command::RerunLabel { label_id: "all".into() }),
        ("update", Command::UpdateLabel { label_id: "primary".into(), spec: spec("Primary", None) }),
        ("create under", Command::CreateLabel { name: "Sub".into(), parent_id: Some("all".into()) }),
        ("rename tab", Command::RenameSplitTab { tab_id: "primary".into(), name: "Main".into() }),
        ("hide tab", Command::SetSplitTabEnabled { id: "all".into(), on: false }),
        ("show tab", Command::SetSplitTabEnabled { id: "primary".into(), on: true }),
        ("remove tab", Command::RemoveSplitTab { id: "all".into() }),
        ("reorder tabs", Command::ReorderSplitTabs { ids: vec!["updates".into(), "all".into()] }),
        ("tab on a view", Command::AddSplitTab { name: "X".into(), rule: SplitTabRule::Label { label_id: "primary".into() } }),
    ];
    for (what, c) in doors {
        let (code, words) = refused(check_view_command(c));
        assert_eq!(code, VIEW_REFUSAL_CODE, "{what}");
        assert!(words.contains("view"), "{what}: {words}");
    }
    // the same actions on a real label and the commands that are not about labels all pass
    for c in [
        Command::Label { message_ids: ids.clone(), label_name: "Clients > All".into(), create_if_missing: true },
        Command::RemoveLabel { message_ids: ids.clone(), label_id: "clients".into() },
        Command::PinLabel { label_id: "clients".into(), pinned: true },
        Command::PinLabel { label_id: "needs-reply".into(), pinned: false },
        Command::RenameLabel { label_id: "clients".into(), name: "Customers".into() },
        Command::MoveLabel { label_id: "clients".into(), parent_id: None },
        Command::DeleteLabel { label_id: "clients".into() },
        Command::MarkRead { message_ids: ids.clone(), read: true },
        Command::Archive { message_ids: ids },
    ] {
        assert!(check_view_command(c.clone()).is_ok(), "{c:?} is not about a view");
    }
}

#[test]
fn a_split_tab_order_keeps_the_views_first_in_their_order_and_every_other_order_is_the_stores_to_judge() {
    let order = |l: &[&str]| Command::ReorderSplitTabs { ids: l.iter().map(|x| x.to_string()).collect() };
    // the order lists every tab and Primary is one: the views first, the others in any order, is let by
    let fine: Vec<Vec<&str>> = vec![vec!["primary", "updates", "needs-reply"], vec!["primary", "all", "updates"], vec!["primary", "all"], vec!["all", "updates"], vec!["updates", "needs-reply"], vec![]];
    for l in &fine {
        assert!(check_view_command(order(l)).is_ok(), "{l:?}: the views are first (or there is none): not a move");
    }
    // a tab twice or missing is no move of a view: the store answers it with its own sentence
    assert!(check_view_command(order(&["primary", "primary", "updates"])).is_ok());
    // a view moved is refused in the views' words
    let moved: Vec<(Vec<&str>, &str)> = vec![(vec!["updates", "primary"], "Primary"), (vec!["primary", "updates", "all"], "All"), (vec!["all", "primary"], "Primary"), (vec!["updates", "all"], "All")];
    for (l, who) in &moved {
        let (code, words) = refused(check_view_command(order(l)));
        assert_eq!(code, VIEW_REFUSAL_CODE, "{l:?}");
        assert_eq!(words, format!("{who} is a view of the inbox, not a label, so it cannot be reordered: the views stay first."), "{l:?}");
    }
}
// broken copy: inbox_views.rs `check_tab_order`: `if !leading || p < last` becomes `if p < last`: the `["updates","primary"]` case is let by and the test fails there.
// broken copy: inbox_views.rs `check_tab_order`: `if !leading || p < last` becomes `if !leading`: the `["all","primary"]` case is let by and the test fails there.

#[test]
fn a_top_level_label_cannot_be_named_like_a_view_but_a_nested_one_can() {
    for name in ["Primary", "primary", "ALL", " All "] {
        let (code, words) = refused(check_view_command(Command::CreateLabel { name: name.into(), parent_id: None }));
        assert_eq!(code, VIEW_REFUSAL_CODE, "{name:?}");
        assert!(words.contains("name of a view"), "{words}");
        assert!(check_view_command(Command::CreateLabelFull { spec: spec(name, None) }).is_err(), "{name:?} full");
        assert!(validate_label_spec(spec(name, None), None).is_err(), "{name:?} validate");
    }
    assert!(check_view_command(Command::CreateLabel { name: "All".into(), parent_id: Some("clients".into()) }).is_ok());
    let parent = label("clients", "Clients", 0, false, LabelKind::Owner);
    assert!(validate_label_spec(spec("All", Some("clients")), Some(parent)).is_ok(), "Clients > All is an ordinary label");
    assert!(validate_label_spec(spec("Allies", None), None).is_ok(), "only the exact name is reserved");
}

#[test]
fn every_refusal_carries_the_code_and_a_sentence_naming_the_view() {
    let (code, words) = refused(check_not_a_view("primary".into(), ViewRefusedAction::Pin));
    assert_eq!(code, "view_not_a_label");
    assert_eq!(words, "Primary is a view of the inbox, not a label, so it cannot be pinned: the views are always the first tabs.");
    let (_, words) = refused(check_not_a_view("all".into(), ViewRefusedAction::Rename));
    assert_eq!(words, "All is a view of the inbox, not a label, so it cannot be renamed.");
    assert!(check_not_a_view("clients".into(), ViewRefusedAction::Rename).is_ok());
    // the older rename and delete doors read the same sentence on a legacy view row
    let legacy = label("primary", "Primary", 0, true, LabelKind::BuiltIn);
    assert_eq!(refused(check_label_rename(legacy.clone())).0, VIEW_REFUSAL_CODE);
    assert_eq!(refused(check_label_delete(legacy)).0, VIEW_REFUSAL_CODE);
    assert_eq!(refused(reorder_label(vec![label("a", "A", 0, false, LabelKind::Owner)], "all".into(), None, 0).map(|_| ())).0, VIEW_REFUSAL_CODE);
}

#[test]
fn unread_stays_the_delta_10_filter_and_applies_to_a_view_like_to_a_tab() {
    // the chip's source is InboxFilter::Unread: unchanged, the query for a view is the query for a tab plus the filter
    let q = InboxQuery { tab_id: inbox_view_id(InboxView::All), account_id: None, label_id: None, mailbox: None, account_ids: vec![],
        filters: vec![InboxFilter::Unread], smart_folder_id: None };
    assert_eq!(q.tab_id, "all");
    let unread = ListFilterFacts { unread: true, starred: false, important: false, awaiting_reply: false };
    let read = ListFilterFacts { unread: false, ..unread };
    assert!(passes_filters(unread, q.filters.clone()));
    assert!(!passes_filters(read, q.filters.clone()));
    // Unread is not a tab and not a view
    assert!(inbox_tabs(vec![]).iter().all(|t| t.id != "unread"));
    assert_eq!(inbox_view_from_id("unread".into()), None);
}
