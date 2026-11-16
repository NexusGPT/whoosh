//! Delta 9 probes: the pure logic of labels-as-tabs, rule sets, re-run, suggestions and blocks. Each test names the rule it
//! holds; `_captures/contracts/mutate-d9.out` records the mutant that turned it red.
use contracts::*;

fn label(id: &str, name: &str, parent: Option<&str>, order: u32, depth: u8, path: &str) -> MailLabel {
    MailLabel { id: id.into(), name: name.into(), parent_id: parent.map(String::from), colour: None, account_ids: vec![], path: path.into(), depth,
        unread: 0, description: String::new(), pinned: false, scope: LabelScope::All, kind: LabelKind::Owner, order, entry_enabled: true, ai_enabled: true, exit_enabled: false, ai_reads: default_ai_reads(), exit_fallback: None }
}
fn tree() -> Vec<MailLabel> {
    vec![label("work", "Work", None, 1, 1, "Work"), label("clients", "Clients", Some("work"), 0, 2, "Work > Clients"),
         label("sup", "Suppliers", Some("work"), 1, 2, "Work > Suppliers"), label("home", "Home", None, 0, 1, "Home")]
}
fn ids(v: &[MailLabel]) -> Vec<&str> { v.iter().map(|l| l.id.as_str()).collect() }
fn words(r: Result<impl std::fmt::Debug, CoreError>) -> String { match r { Err(CoreError::Invalid { reason_words, .. }) => reason_words, other => panic!("expected a refusal in words, got {other:?}") } }
fn facts(id: &str, from: &str, subject: &str, unread: bool, at: i64) -> MessageFacts {
    MessageFacts { message_id: id.into(), account_id: "acc-1".into(), from_name: "".into(), from_address: from.into(), to: vec![], cc: vec![], subject: subject.into(),
        body: "".into(), has_attachment: false, list_header: "".into(), unread, received_at_ms: at,
        unanswered: false, starred: false, vip_sender: false, cc_me: false, replied_by_me: false }
}
fn rs(mode: MatchMode, c: Vec<RuleCondition>) -> RuleSet { RuleSet { conditions: c, match_mode: mode } }

#[test]
fn the_four_categories_are_built_in_labels_with_the_former_tab_ids_and_social_is_not_pinned_and_primary_is_a_view() {
    let b = builtin_labels();
    assert_eq!(b.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Needs Reply", "Updates", "Promotions", "Social"], "delta 16: Primary is a view, not a label");
    assert_eq!(ids(&b), ["needs-reply", "updates", "promotions", "social"], "a built-in label keeps the id of the tab it replaces");
    assert_eq!(b.iter().filter(|l| l.pinned).map(|l| l.name.as_str()).collect::<Vec<_>>(), ["Needs Reply", "Updates", "Promotions"]);
    assert!(b.iter().all(|l| l.kind == LabelKind::BuiltIn && l.parent_id.is_none() && l.depth == 1 && l.scope == LabelScope::All));
}

#[test]
fn rename_and_delete_are_refused_on_a_built_in_in_words_and_allowed_on_an_owner_label() {
    let p = builtin_labels().remove(0);   // Needs Reply
    assert!(words(check_label_rename(p.clone())).contains("built-in"));
    assert!(words(check_label_delete(p)).contains("Unpin"));
    assert!(check_label_rename(tree().remove(0)).is_ok() && check_label_delete(tree().remove(0)).is_ok());
}

#[test]
fn the_tree_order_puts_parents_first_and_siblings_by_order_and_the_tabs_follow_it() {
    let t = ordered_tree(tree());
    assert_eq!(ids(&t), ["home", "work", "clients", "sup"]);
    // the saved order wins over the alphabet: B before A, and Z before Y under B
    let odd = vec![label("a", "A", None, 1, 1, "A"), label("y", "Y", Some("b"), 1, 2, "B > Y"), label("b", "B", None, 0, 1, "B"), label("z", "Z", Some("b"), 0, 2, "B > Z")];
    assert_eq!(ids(&ordered_tree(odd)), ["b", "z", "y", "a"]);
    let mut pinned = tree();
    for l in pinned.iter_mut() { l.pinned = l.id == "sup" || l.id == "home"; }
    assert_eq!(ids(&pinned_in_tree_order(pinned)), ["home", "sup"], "tab order = tree order, not insertion order");
}

#[test]
fn a_drag_resequences_both_parents_rewrites_paths_and_refuses_a_loop_or_too_deep() {
    let moved = reorder_label(tree(), "sup".into(), Some("home".into()), 0).unwrap();
    assert_eq!(ids(&moved), ["home", "sup", "work", "clients"]);
    let sup = moved.iter().find(|l| l.id == "sup").unwrap();
    assert_eq!((sup.path.as_str(), sup.depth, sup.order), ("Home > Suppliers", 2, 0));
    assert_eq!(moved.iter().find(|l| l.id == "clients").unwrap().order, 0, "the old parent's siblings close the gap");
    // an index past the end means last
    let last = reorder_label(tree(), "home".into(), Some("work".into()), 99).unwrap();
    assert_eq!(last.iter().find(|l| l.id == "home").unwrap().order, 2);
    // into its own subtree / onto itself
    assert!(words(reorder_label(tree(), "work".into(), Some("clients".into()), 0)).contains("inside itself"));
    assert!(words(reorder_label(tree(), "work".into(), Some("work".into()), 0)).contains("inside itself"));
    // depth: a chain of five cannot go one deeper
    let mut chain = vec![label("l1", "a", None, 0, 1, "a")];
    for d in 2..=5u8 { chain.push(label(&format!("l{d}"), &format!("n{d}"), Some(&format!("l{}", d - 1)), 0, d, "x")); }
    chain.push(label("top", "Top", None, 1, 1, "Top"));
    assert!(words(reorder_label(chain.clone(), "top".into(), Some("l5".into()), 0)).contains("5 levels"));
    assert!(words(reorder_label(chain, "l1".into(), Some("top".into()), 0)).contains("5 levels"), "moving the whole chain under Top makes it six");
}

#[test]
fn a_drag_refuses_a_duplicate_sibling_name_a_built_in_leaving_the_top_and_a_scope_that_does_not_fit() {
    let mut t = tree();
    t.push(label("dup", "suppliers", None, 5, 1, "suppliers"));
    assert!(words(reorder_label(t, "dup".into(), Some("work".into()), 0)).contains("already a label named"));
    let mut b = tree();
    b.extend(builtin_labels());
    assert!(words(reorder_label(b, "needs-reply".into(), Some("work".into()), 0)).contains("top level"));
    let mut s = tree();
    s[0].scope = LabelScope::Account { account_id: "acc-1".into() };
    assert!(words(reorder_label(s, "home".into(), Some("work".into()), 0)).contains("same mailbox"));
}

fn spec(name: &str) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: Some("#2F6FED".into()), description: "".into(), scope: LabelScope::All, rules: rs(MatchMode::Any, vec![]), pinned: false }
}

#[test]
fn the_sheet_is_checked_before_anything_is_written() {
    assert!(validate_label_spec(spec("Quotes"), None).is_ok());
    assert!(words(validate_label_spec(spec("  "), None)).contains("needs a name"));
    assert!(words(validate_label_spec(spec("A > B"), None)).contains("\">\""));
    let mut c = spec("Q"); c.colour = Some("blue".into());
    assert!(words(validate_label_spec(c, None)).contains("#RRGGBB"));
    let mut r = spec("Q"); r.rules = rs(MatchMode::All, vec![RuleCondition::SubjectContains { words: " ".into() }]);
    assert!(words(validate_label_spec(r, None)).contains("This rule has no value"));
    let mut d = spec("Q"); d.rules = rs(MatchMode::All, vec![RuleCondition::Domain { domain: "a@b.com".into() }]);
    assert!(validate_label_spec(d, None).is_err());
    let deep = label("p5", "five", None, 0, 5, "a > b > c > d > five");
    assert!(words(validate_label_spec(spec("Six"), Some(deep))).contains("5 levels"));
    let mut under = label("acct", "Acct", None, 0, 1, "Acct"); under.scope = LabelScope::Account { account_id: "acc-2".into() };
    assert!(words(validate_label_spec(spec("Shared"), Some(under.clone()))).contains("same mailbox"));
    let mut ok = spec("Same"); ok.scope = LabelScope::Account { account_id: "acc-2".into() };
    assert!(validate_label_spec(ok, Some(under)).is_ok());
}

#[test]
fn an_empty_rule_set_matches_nothing_in_either_mode() {
    for m in [MatchMode::Any, MatchMode::All] { assert!(!rule_set_matches(rs(m, vec![]), facts("1", "a@b.com", "hi", true, 1))); }
}

#[test]
fn any_and_all_combine_conditions_and_a_domain_matches_its_subdomains() {
    let f = facts("1", "orders@mail.linea-supply-example.com", "Quote Q1", true, 1);
    let from = RuleCondition::From { words: "ORDERS".into() };
    let subj = RuleCondition::SubjectContains { words: "invoice".into() };
    assert!(rule_set_matches(rs(MatchMode::Any, vec![from.clone(), subj.clone()]), f.clone()));
    assert!(!rule_set_matches(rs(MatchMode::All, vec![from, subj]), f.clone()), "all needs every condition");
    assert!(rule_set_matches(rs(MatchMode::All, vec![RuleCondition::Domain { domain: "linea-supply-example.com".into() }]), f.clone()));
    assert!(!rule_set_matches(rs(MatchMode::Any, vec![RuleCondition::Domain { domain: "supply-example.com".into() }]), f), "a domain is a whole name, not a suffix of letters");
}

#[test]
fn the_live_count_reports_count_unread_and_newest() {
    let corpus = vec![facts("1", "a@x.com", "s", true, 10), facts("2", "a@x.com", "s", false, 30), facts("3", "b@y.com", "s", true, 20)];
    let c = count_rule_matches(rs(MatchMode::Any, vec![RuleCondition::Domain { domain: "x.com".into() }]), corpus.clone());
    assert_eq!(c, RuleSetCount { count: 2, unread: 1, newest_at_ms: Some(30) });
    assert_eq!(count_rule_matches(rs(MatchMode::Any, vec![]), corpus), RuleSetCount { count: 0, unread: 0, newest_at_ms: None });
}

#[test]
fn a_rerun_files_new_matches_unfiles_old_ones_and_with_no_rules_changes_nothing() {
    let corpus = vec![facts("1", "a@x.com", "s", true, 1), facts("2", "b@y.com", "s", true, 2), facts("3", "a@x.com", "s", true, 3)];
    let rules = rs(MatchMode::Any, vec![RuleCondition::Domain { domain: "x.com".into() }]);
    let p = plan_rerun(rules.clone(), false, corpus.clone(), vec!["1".into(), "2".into()]);
    assert_eq!((p.file, p.unfile, p.keep, p.model_pending), (vec!["3".to_string()], vec!["2".to_string()], vec!["1".to_string()], false));
    // delta 12: with a meaning the decision model has not spoken yet, a filed message no rule matches stays
    let m = plan_rerun(rules, true, corpus.clone(), vec!["1".into(), "2".into()]);
    assert_eq!((m.file, m.unfile, m.keep, m.model_pending), (vec!["3".to_string()], Vec::<String>::new(), vec!["1".to_string(), "2".to_string()], true));
    let none = plan_rerun(rs(MatchMode::Any, vec![]), false, corpus, vec!["1".into(), "2".into()]);
    assert_eq!((none.file.len(), none.unfile.len(), none.keep, none.model_pending), (0, 0, vec!["1".to_string(), "2".to_string()], false), "no rule, no judgement: nothing leaves");
}

fn sug(kind: SuggestionKind, subject: &str, label: Option<&str>, accepted: bool) -> Suggestion {
    Suggestion { id: "s1".into(), kind, subject: subject.into(), label_id: label.map(String::from), offered_at_ms: 1, accepted }
}

#[test]
fn accepting_a_suggestion_says_what_to_write_once() {
    assert_eq!(suggestion_effect(sug(SuggestionKind::BlockSender, "Orders@Linea.example", None, false)).unwrap(), SuggestionEffect::Block { target: BlockTarget::Sender { address: "orders@linea.example".into() } });
    assert_eq!(suggestion_effect(sug(SuggestionKind::AlwaysJunkDomain, "@Linea.example", None, false)).unwrap(), SuggestionEffect::Block { target: BlockTarget::Domain { domain: "linea.example".into() } });
    assert_eq!(suggestion_effect(sug(SuggestionKind::AlwaysLabel, "a@b.com", Some("lbl-3"), false)).unwrap(), SuggestionEffect::LabelRule { address: "a@b.com".into(), label_id: "lbl-3".into() });
    assert!(words(suggestion_effect(sug(SuggestionKind::AlwaysLabel, "a@b.com", None, false))).contains("label"));
    assert!(words(suggestion_effect(sug(SuggestionKind::BlockSender, "a@b.com", None, true))).contains("already accepted"));
    assert!(suggestion_effect(sug(SuggestionKind::MuteThread, "  ", None, false)).is_err());
    assert_eq!(suggestion_words(SuggestionKind::BlockSender, "a@b.com".into()), "Block a@b.com");
}

#[test]
fn a_block_covers_the_sender_or_the_domain_and_its_subdomains_only() {
    let blocks = vec![Block { target: BlockTarget::Sender { address: "Spam@x.com".into() }, since_ms: 1 }, Block { target: BlockTarget::Domain { domain: "bad.example".into() }, since_ms: 2 }];
    assert!(is_blocked(blocks.clone(), "spam@x.com".into()) && is_blocked(blocks.clone(), "a@bad.example".into()) && is_blocked(blocks.clone(), "a@news.bad.example".into()));
    assert!(!is_blocked(blocks.clone(), "other@x.com".into()) && !is_blocked(blocks, "a@notbad.example".into()));
}

#[test]
fn every_mailbox_has_its_own_key() {
    let all = [Mailbox::Outbox, Mailbox::Vips, Mailbox::Pinned, Mailbox::Snoozed, Mailbox::Drafts, Mailbox::Sent, Mailbox::Junk, Mailbox::Trash, Mailbox::Archive];
    let keys: std::collections::BTreeSet<String> = all.iter().map(|m| mailbox_key(*m)).collect();
    assert_eq!(keys.len(), all.len());
    assert!(!keys.contains(MAILBOX_ALL_KEY));
}

#[test]
fn reply_all_addresses_the_sender_and_every_recipient_but_the_owner_once() {
    let own = vec!["me@work.example".to_string(), "ME@home.example".to_string()];
    let r = reply_all_recipients("Ann@x.com".into(), vec!["me@work.example".into(), "bob@x.com".into(), "ann@x.com".into()], vec!["Bob@X.com".into(), "cy@x.com".into(), "me@home.example".into()], own.clone());
    assert_eq!((r.to, r.cc), (vec!["Ann@x.com".to_string(), "bob@x.com".to_string()], vec!["cy@x.com".to_string()]), "the owner's addresses drop out in any case, nobody twice, a cc already in to stays in to");
    let mine = reply_all_recipients("me@work.example".into(), vec!["bob@x.com".into()], vec![], own);
    assert_eq!(mine.to, vec!["bob@x.com".to_string()], "answering the owner's own mail goes to its recipients");
}

#[test]
fn a_subject_takes_its_prefix_once_and_the_bodies_quote_and_list_attachments() {
    assert_eq!(prefixed_subject("Quote".into(), "Re".into()), "Re: Quote");
    assert_eq!(prefixed_subject("RE: Quote".into(), "Re".into()), "RE: Quote");
    assert_eq!(prefixed_subject("Re: Quote".into(), "Fwd".into()), "Fwd: Re: Quote");
    assert_eq!(quoted_reply_body("Ann".into(), "Mon 09:12".into(), "Hi\n\nBye".into()), "\n\nOn Mon 09:12, Ann wrote:\n> Hi\n>\n> Bye");
    let f = forward_body("Ann".into(), "ann@x.com".into(), "Quote".into(), "Mon 09:12".into(), vec!["q1.pdf".into(), "photo.jpg".into()], "Hi".into());
    assert!(f.contains("From: Ann <ann@x.com>") && f.contains("Attachments: q1.pdf, photo.jpg") && f.ends_with("\nHi"));
    assert!(!forward_body("A".into(), "a@b.c".into(), "S".into(), "t".into(), vec![], "x".into()).contains("Attachments"));
}
