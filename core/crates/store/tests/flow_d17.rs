//! Delta 17 through the store (label_meta.rs over migration 0009): a fact condition round-trips and an unknown fact is refused in
//! words; a label reads its three switches, its AI inputs and its fallback; the six flow writes keep the contract's refusal codes;
//! chain numbers are never reused; a filing keeps its card and chain and the counts read them with the rejections.
//! Mutants (single edits, unobserved: uncompiled round):
//! - S1 encode_conditions Fact: `wanted` -> `true` -> a_fact_condition_round_trips...
//! - S2 label_from: `exit_enabled: r.get(13)` -> `r.get(12)` -> the_switches_and_the_fallback...
//! - S3 label_set_card: drop the `check_set_card` refusal -> the_switches_and_the_fallback... (the trigger then answers, without the code)
//! - S4 flow_filings: drop the `sent_to IN ('primary','junk')` rows -> a_filing_keeps_its_card...
mod common;
use common::*;
use contracts::*;

fn spec(name: &str) -> LabelSpec {
    LabelSpec { name: name.into(), parent_id: None, colour: None, description: "emails waiting for my answer".into(), scope: LabelScope::All,
        rules: RuleSet { conditions: vec![RuleCondition::SubjectContains { words: "contract".into() }], match_mode: MatchMode::Any }, pinned: false }
}
fn code(r: Result<impl std::fmt::Debug, CoreError>) -> String {
    match r.expect_err("must be refused") { CoreError::Invalid { field, .. } => field, CoreError::NotFound { kind, .. } => format!("not_found:{kind}"), other => format!("{other:?}") }
}

#[test]
fn a_fact_condition_round_trips_and_an_unknown_fact_is_refused_in_words() {
    let cs = vec![RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }, RuleCondition::Fact { fact: MessageFact::Starred, wanted: false }, RuleCondition::HasAttachment];
    let json = store::encode_conditions(&cs);
    assert_eq!(json, r#"[{"kind":"fact","fact":"unanswered","wanted":true},{"kind":"fact","fact":"starred","wanted":false},{"kind":"has_attachment"}]"#);
    let c = rusqlite::Connection::open_in_memory().unwrap();
    assert_eq!(store::decode_conditions(&c, &json).unwrap(), cs);
    let bad = store::decode_conditions(&c, r#"[{"kind":"fact","fact":"sparkly","wanted":true}]"#).expect_err("refused").to_string();
    assert!(bad.contains("sparkly"), "{bad}");
    assert!(store::decode_conditions(&c, r#"[{"kind":"fact","fact":"unread"}]"#).is_err(), "a fact with no yes/no is refused");
}

#[test]
fn the_switches_and_the_fallback_are_read_and_written_with_the_contracts_codes() {
    let db = TempDb::new("d17-switches");
    let s = small_store(&db, 0);
    let l = s.label_create_full(spec("Needs reply"), 1).unwrap();
    assert_eq!((l.entry_enabled, l.ai_enabled, l.exit_enabled, l.ai_reads, l.exit_fallback.clone()), (true, true, false, default_ai_reads(), None), "migration 0009's defaults");
    assert_eq!(code(s.label_set_card(l.id.clone(), FlowCard::Exit, true, 2)), EXIT_NEEDS_FALLBACK_CODE);
    assert_eq!(code(s.label_set_exit_fallback(l.id.clone(), Some(l.id.clone()), 2)), EXIT_FALLBACK_REFUSAL_CODE, "never itself");
    s.label_set_exit_fallback(l.id.clone(), Some("primary".into()), 3).unwrap();
    s.label_set_card(l.id.clone(), FlowCard::Exit, true, 4).unwrap();
    assert_eq!(code(s.label_set_exit_fallback(l.id.clone(), None, 5)), EXIT_NEEDS_FALLBACK_CODE, "no clearing while the filter is on");
    assert_eq!(code(s.label_set_ai_reads(l.id.clone(), AiReads { subject: false, body: false, rest_of_thread: false, attachments: false }, 6)), AI_READS_NOTHING_CODE);
    s.label_set_ai_reads(l.id.clone(), AiReads { subject: true, body: false, rest_of_thread: false, attachments: true }, 7).unwrap();
    s.label_set_card(l.id.clone(), FlowCard::Ai, false, 8).unwrap();
    let back = s.labels_full().unwrap().into_iter().find(|x| x.id == l.id).unwrap();
    assert_eq!((back.exit_enabled, back.ai_enabled, back.ai_reads, back.exit_fallback.as_deref()), (true, false, AiReads { subject: true, body: false, rest_of_thread: false, attachments: true }, Some("primary")));
    assert_eq!(code(s.label_set_card("no-such".into(), FlowCard::Entry, false, 9)), "not_found:label");
}

#[test]
fn chains_are_added_numbered_replaced_and_removed_and_a_number_is_never_reused() {
    let db = TempDb::new("d17-chains");
    let s = small_store(&db, 0);
    let l = s.label_create_full(spec("Needs reply"), 1).unwrap();
    let unanswered = RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }], match_mode: MatchMode::All };
    assert_eq!(s.label_chains(&l.id, ChainPlacement::Entry).unwrap().iter().map(|c| c.chain_id).collect::<Vec<_>>(), vec![0], "the sheet's rule set is entry chain 0");
    assert_eq!(s.label_chain_add(l.id.clone(), ChainPlacement::Exit, unanswered.clone(), 2).unwrap(), 0);
    assert_eq!(s.label_chain_add(l.id.clone(), ChainPlacement::Exit, unanswered.clone(), 3).unwrap(), 1);
    s.label_chain_remove(l.id.clone(), ChainPlacement::Exit, 0).unwrap();
    assert_eq!(s.label_chain_add(l.id.clone(), ChainPlacement::Exit, unanswered.clone(), 4).unwrap(), 2, "one past the highest, never the freed 0");
    assert_eq!(code(s.label_chain_add(l.id.clone(), ChainPlacement::Exit, RuleSet { conditions: vec![], match_mode: MatchMode::All }, 5)), CHAIN_REFUSAL_CODE);
    let starred = RuleSet { conditions: vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: true }], match_mode: MatchMode::Any };
    s.label_chain_update(l.id.clone(), ChainPlacement::Exit, 1, starred.clone(), 6).unwrap();
    assert_eq!(s.label_chains(&l.id, ChainPlacement::Exit).unwrap(), vec![LabelChain { chain_id: 1, rules: starred }, LabelChain { chain_id: 2, rules: unanswered }]);
    assert_eq!(code(s.label_chain_update(l.id.clone(), ChainPlacement::Exit, 9, RuleSet { conditions: vec![RuleCondition::HasAttachment], match_mode: MatchMode::Any }, 7)), "not_found:chain");
    let rules = s.label_rules_all().unwrap();
    assert!(rules.iter().any(|r| r.placement == ChainPlacement::Exit && r.chain_id == 2), "the decision lane reads both cards");
    assert_eq!(s.label_rules(&l.id).unwrap().conditions, vec![RuleCondition::SubjectContains { words: "contract".into() }], "the sheet still reads chain 0");
}

#[test]
fn a_filing_keeps_its_card_and_chain_and_the_counts_read_filings_and_rejections() {
    let db = TempDb::new("d17-counts");
    let s = small_store(&db, 4);
    let l = s.label_create_full(spec("Needs reply"), 1).unwrap();
    // small_store names its messages m000, m001, ...
    let (lid, a, b, c) = (l.id.clone(), "m000".to_string(), "m001".to_string(), "m002".to_string());
    let (la, lb, lc) = (lid.clone(), lid.clone(), lid.clone());
    let (a2, b2, c2) = (a.clone(), b.clone(), c.clone());
    s.write(move |ctx| {
        ctx.label_file_flow(&la, &a2, &DecidedBy::Rule { rule_id: "rule:x".into() }, None, None, WhyCard::Entry, Some(0), 2)?;
        ctx.label_file_flow(&lb, &b2, &DecidedBy::Laya, Some("labels/model"), Some(0.9), WhyCard::Exit, Some(1), 2)?;
        ctx.flow_rejection_put(&FlowRejection { message_id: c2.clone(), ai_label_id: lc.clone(), sent_to: "primary".into(), chain_id: None, decided_at_ms: 2 })
    }).unwrap();
    let counts = count_label_flow(lid.clone(), s.label_flow_filings().unwrap());
    assert_eq!((counts.caught_by_entry, counts.sorted_by_ai, counts.kept_by_exit, counts.sent_to_fallback), (1, 2, 1, 1));
    assert_eq!(s.flow_rejection(&c).unwrap().map(|r| r.sent_to), Some("primary".to_string()));
    let why = s.label_why(&a, &lid).unwrap().expect("filed");
    assert_eq!((why.card, why.chain_id, why.decided_by), (WhyCard::Entry, Some(0), DecidedBy::Rule { rule_id: "rule:x".into() }));
    let why = s.label_why(&c, "primary").unwrap().expect("sent to Primary");
    assert_eq!(why.card, WhyCard::Fallback);
    let _ = b;
}
