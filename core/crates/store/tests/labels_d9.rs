//! Delta 9 through the store's additive functions: the full label record, the sheet's create and update (the clear path
//! included), drag, pin, the label's rule, how a message was filed, blocks and suggestions. Each test names the rule it holds.
mod common;
use common::*;
use contracts::*;

fn spec(name: &str) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: Some("#2F6FED".into()), description: "quotes from suppliers".into(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![RuleCondition::Domain { domain: "linea.example".into() }, RuleCondition::SubjectContains { words: "quote \"Q1\"".into() }], match_mode: MatchMode::All }, pinned: false }
}
fn names(v: &[MailLabel]) -> Vec<String> { v.iter().map(|l| l.name.clone()).collect() }
fn refusal(r: Result<impl std::fmt::Debug, store::StoreError>) -> String { r.expect_err("must be refused").to_string() }

#[test]
fn the_built_ins_come_first_and_the_pinned_ones_are_the_tabs() {
    let db = TempDb::new("d9-builtin");
    let s = small_store(&db, 0);
    let all = s.labels_full().unwrap();
    assert_eq!(names(&all[..5]), ["Primary", "Needs Reply", "Updates", "Promotions", "Social"]);
    assert_eq!(names(&s.pinned_labels().unwrap()), ["Primary", "Needs Reply", "Updates", "Promotions"]);
    assert!(all[..5].iter().all(|l| l.kind == LabelKind::BuiltIn));
}

#[test]
fn a_label_is_created_with_its_meaning_scope_and_rules_and_read_back_whole() {
    let db = TempDb::new("d9-create");
    let s = small_store(&db, 0);
    let mut sp = spec("Quotes");
    sp.scope = LabelScope::Account { account_id: "acc-a".into() };
    let made = s.label_create_full(sp.clone(), 10).unwrap();
    assert_eq!((made.description.as_str(), made.colour.as_deref(), made.scope.clone(), made.kind, made.pinned), ("quotes from suppliers", Some("#2F6FED"), LabelScope::Account { account_id: "acc-a".into() }, LabelKind::Owner, false));
    assert_eq!(s.label_rules(&made.id).unwrap(), sp.rules, "the rule round-trips, quotes and all");
    // the kind names are the ones smart-inbox/src/load.rs reads ("subject", "body", ...): compare the stored text byte for byte
    let id = made.id.clone();
    let stored: String = s.write(move |ctx| Ok(ctx.tx.query_row("SELECT conditions_json FROM rule WHERE action_label_id = ?1", [id], |r| r.get(0))?)).unwrap();
    assert_eq!(stored, r#"[{"kind":"domain","domain":"linea.example"},{"kind":"subject","words":"quote \"Q1\""}]"#);
    assert_eq!(s.labels_full().unwrap().last().unwrap().id, made.id, "a new top-level label goes last");
    let mut bad = spec("Bad"); bad.colour = Some("red".into());
    assert!(refusal(s.label_create_full(bad, 11)).contains("#RRGGBB"));
    let mut under = spec("Kid"); under.parent_id = Some(made.id.clone());
    assert!(refusal(s.label_create_full(under, 12)).contains("same mailbox"), "a shared label cannot sit under a one-mailbox label");
}

#[test]
fn an_update_replaces_every_field_so_clearing_colour_meaning_and_rules_works() {
    let db = TempDb::new("d9-clear");
    let s = small_store(&db, 0);
    let made = s.label_create_full(spec("Quotes"), 10).unwrap();
    let mut cleared = spec("Quotes");
    cleared.colour = None; cleared.description = String::new(); cleared.rules = RuleSet { conditions: vec![], match_mode: MatchMode::Any }; cleared.pinned = true;
    let after = s.label_update_full(made.id.clone(), cleared, 20).unwrap();
    assert_eq!((after.colour, after.description.as_str(), after.pinned), (None, "", true), "the clear path is the one that breaks");
    assert!(s.label_rules(&made.id).unwrap().conditions.is_empty(), "no rules is a value, not 'unchanged'");
    let mut renamed = spec("Quotes 2");
    renamed.rules = RuleSet { conditions: vec![RuleCondition::HasAttachment], match_mode: MatchMode::Any };
    let r = s.label_update_full(made.id.clone(), renamed, 30).unwrap();
    assert_eq!(r.name, "Quotes 2");
    assert_eq!(s.label_rules(&made.id).unwrap().conditions, vec![RuleCondition::HasAttachment]);
}

#[test]
fn a_built_in_refuses_rename_move_and_delete_in_words_but_keeps_its_meaning_editable() {
    let db = TempDb::new("d9-builtin-guard");
    let s = small_store(&db, 0);
    // delta 16: the first row is the Primary VIEW's legacy row, which the view door refuses on its own; the guard is a built-in's
    let p = s.labels_full().unwrap().into_iter().find(|l| l.kind == LabelKind::BuiltIn && inbox_view_from_id(l.id.clone()).is_none()).expect("a built-in label");
    let sp = |name: &str| LabelSpec { name: name.into(), parent_id: None, colour: None, description: "mail I must answer today".into(), scope: LabelScope::All, rules: RuleSet { conditions: vec![], match_mode: MatchMode::Any }, pinned: true };
    assert!(refusal(s.label_update_full(p.id.clone(), sp("Main"), 5)).contains("cannot be renamed"));
    assert!(refusal(s.label_delete_tree(p.id.clone())).contains("cannot be deleted"));
    let own = s.label_create_full(spec("Mine"), 6).unwrap();
    assert!(refusal(s.label_reorder(p.id.clone(), Some(own.id), 0)).contains("top level"));
    let kept = s.label_update_full(p.id.clone(), sp(&p.name), 7).unwrap();
    assert_eq!(kept.description, "mail I must answer today");
}

#[test]
fn a_drag_changes_the_tab_order_and_the_last_pin_cannot_go() {
    let db = TempDb::new("d9-drag");
    let s = small_store(&db, 0);
    let mut sp = spec("Provanta"); sp.pinned = true; sp.rules = RuleSet { conditions: vec![], match_mode: MatchMode::Any };
    let prox = s.label_create_full(sp, 10).unwrap();
    assert_eq!(names(&s.pinned_labels().unwrap()), ["Primary", "Needs Reply", "Updates", "Promotions", "Provanta"]);
    s.label_reorder(prox.id.clone(), None, 1).unwrap();
    assert_eq!(names(&s.pinned_labels().unwrap()), ["Primary", "Provanta", "Needs Reply", "Updates", "Promotions"], "the saved order is the tab order");
    let mut kid = spec("RFP"); kid.parent_id = Some(prox.id.clone()); kid.pinned = true; kid.rules = RuleSet { conditions: vec![], match_mode: MatchMode::Any };
    let rfp = s.label_create_full(kid, 11).unwrap();
    assert_eq!(rfp.path, "Provanta > RFP");
    assert!(refusal(s.label_reorder(prox.id.clone(), Some(rfp.id.clone()), 0)).contains("inside itself"));
    assert_eq!(s.labels_full().unwrap().iter().find(|l| l.id == prox.id).unwrap().order, 1, "a refused drag changes nothing");
    s.label_reorder(rfp.id.clone(), None, 0).unwrap();
    assert_eq!(s.labels_full().unwrap().iter().find(|l| l.id == rfp.id).unwrap().path, "RFP");
    for l in s.pinned_labels().unwrap().into_iter().rev().skip(1) { s.label_pin(l.id, false, 20).unwrap(); }
    let last = s.pinned_labels().unwrap();
    assert_eq!(last.len(), 1);
    assert!(refusal(s.label_pin(last[0].id.clone(), false, 21)).contains("at least one label has to stay pinned"));
}

#[test]
fn deleting_a_label_removes_its_sub_labels_and_its_rule_and_not_the_messages() {
    let db = TempDb::new("d9-delete");
    let s = small_store(&db, 2);
    let a = s.label_create_full(spec("A"), 1).unwrap();
    let mut kid = spec("B"); kid.parent_id = Some(a.id.clone());
    let b = s.label_create_full(kid, 2).unwrap();
    s.label_file(b.id.clone(), s.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap()[0].message_id.clone(), DecidedBy::Owner, None, 3).unwrap();
    assert_eq!(s.label_delete_tree(a.id.clone()).unwrap(), 2);
    assert!(s.label_rules(&b.id).unwrap().conditions.is_empty() && s.label_rules(&a.id).unwrap().conditions.is_empty());
    assert!(s.labels_full().unwrap().iter().all(|l| l.id != a.id && l.id != b.id));
    assert!(!s.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap().is_empty(), "the messages stay");
}

#[test]
fn a_filing_says_who_decided_it_and_a_second_filing_replaces_the_decision() {
    let db = TempDb::new("d9-filing");
    let s = small_store(&db, 1);
    let m = s.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap()[0].message_id.clone();
    s.label_file("updates".into(), m.clone(), DecidedBy::Rule { rule_id: "rule-x".into() }, None, 5).unwrap();
    assert_eq!(s.label_filing("updates", &m).unwrap(), Some((DecidedBy::Rule { rule_id: "rule-x".into() }, None)));
    s.label_file("updates".into(), m.clone(), DecidedBy::Jev, Some(0.8), 6).unwrap();
    assert_eq!(s.label_filing("updates", &m).unwrap(), Some((DecidedBy::Jev, Some(0.8))), "the rule id does not outlive the rule's decision");
    assert_eq!(s.label_filing("social", &m).unwrap(), None);
}

#[test]
fn blocks_are_idempotent_and_a_suggestion_writes_what_the_contract_says_once() {
    let db = TempDb::new("d9-suggest");
    let s = small_store(&db, 0);
    assert!(s.block_add(BlockTarget::Sender { address: "Spam@X.com".into() }, 10).unwrap());
    assert!(!s.block_add(BlockTarget::Sender { address: "spam@x.com".into() }, 99).unwrap(), "the second block changes nothing");
    assert_eq!(s.blocks().unwrap(), vec![Block { target: BlockTarget::Sender { address: "spam@x.com".into() }, since_ms: 10 }]);
    let sug = |id: &str, kind, subject: &str, label: Option<&str>| Suggestion { id: id.into(), kind, subject: subject.into(), label_id: label.map(String::from), offered_at_ms: 20, accepted: false };
    s.suggestion_put(sug("s1", SuggestionKind::AlwaysJunkDomain, "bad.example", None)).unwrap();
    assert_eq!(s.suggestions().unwrap().len(), 1);
    assert_eq!(s.suggestion_accept("s1".into(), 30).unwrap(), SuggestionEffect::Block { target: BlockTarget::Domain { domain: "bad.example".into() } });
    assert!(s.blocks().unwrap().iter().any(|b| b.target == BlockTarget::Domain { domain: "bad.example".into() } && b.since_ms == 30));
    assert!(refusal(s.suggestion_accept("s1".into(), 31)).contains("already accepted"));
    assert!(s.suggestions().unwrap().is_empty(), "an accepted suggestion is no longer on offer");
    // always-label appends a From condition to the label's rule
    let lab = s.label_create_full(spec("Suppliers"), 40).unwrap();
    s.suggestion_put(sug("s2", SuggestionKind::AlwaysLabel, "Orders@Linea.example", Some(&lab.id))).unwrap();
    s.label_update_full(lab.id.clone(), { let mut x = spec("Suppliers"); x.rules = RuleSet { conditions: vec![RuleCondition::HasAttachment], match_mode: MatchMode::Any }; x }, 41).unwrap();
    s.suggestion_accept("s2".into(), 42).unwrap();
    assert_eq!(s.label_rules(&lab.id).unwrap().conditions, vec![RuleCondition::HasAttachment, RuleCondition::From { words: "orders@linea.example".into() }]);
    s.suggestion_put(sug("s3", SuggestionKind::MuteThread, "t-1", None)).unwrap();
    assert!(s.suggestion_dismiss("s3".into()).unwrap() && !s.suggestion_dismiss("s3".into()).unwrap());
}

#[test]
fn an_attachments_vector_key_survives_a_second_save() {
    let db = TempDb::new("d9-vkey");
    let s = small_store(&db, 1);
    let m = s.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap()[0].message_id.clone();
    s.write(move |ctx| { ctx.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes) VALUES ('att-1', ?1, 'a.jpg', 'image/jpeg', 1)", [m])?; Ok(()) }).unwrap();
    let c = AttachmentContent { attachment_id: "att-1".into(), text: None, ocr_text: None, labels: vec![], transcript: None, embedded: true, vector_key: Some("m/att-1".into()), updated_at_ms: 1 };
    s.put_attachment_content(c.clone()).unwrap();
    assert_eq!(s.attachment_content("att-1").unwrap().unwrap().vector_key.as_deref(), Some("m/att-1"));
    s.put_attachment_content(AttachmentContent { updated_at_ms: 2, ..c.clone() }).unwrap();
    s.put_attachment_content(AttachmentContent { vector_key: None, embedded: false, updated_at_ms: 3, ..c }).unwrap();
    assert_eq!(s.attachment_content("att-1").unwrap().unwrap().vector_key, None, "no key is a value too");
}
