//! Delta 17 probes (docs/PRD.md "A94 pick", "A94 pick, detail"): the labelling flow, three cards (Entry rules, AI sorting, Exit
//! filter), chains as alternatives, the eight facts in three states, the fallback, the counts, the "why here" read.
//! Each test names the rule it holds. MUTANTS (the lead runs each at the build word; the test named must go red):
//!   M1  `first_matching_chain` takes the LAST matching chain, not the lowest number      -> chains_are_alternatives_the_lowest_matching_number_decides
//!   M2  `first_matching_chain` requires EVERY chain to match (AND across chains)          -> chains_are_alternatives_the_lowest_matching_number_decides
//!   M3  a chain in "all" mode matches on any one condition                                 -> inside_one_chain_the_mode_decides_and_all_means_and
//!   M4  `fact_holds` reads `unread` for Unanswered                                         -> each_fact_reads_its_own_field_in_three_states
//!   M5  `condition_matches` ignores `wanted` (a "no" fact behaves as "yes")                -> each_fact_reads_its_own_field_in_three_states
//!   M6  `fact_holds(MailingList)` reads a second flag instead of the list header           -> the_mailing_list_fact_is_derived_from_the_list_header
//!   M7  `set_fact(.., Any)` keeps the old condition                                        -> the_grid_writes_one_state_per_fact_and_any_is_no_condition
//!   M8  `set_fact` leaves a second condition on the same fact                              -> the_grid_writes_one_state_per_fact_and_any_is_no_condition
//!   M9  `fact_condition(HasAttachment, Yes)` returns the Fact spelling                     -> pj_yes_keeps_the_old_spelling_and_both_read_as_one_fact
//!   M10 `row_conditions` keeps the fact conditions                                         -> the_rows_under_the_grid_are_the_text_conditions_only
//!   M11 `validate_chain` lets an empty chain through                                       -> a_chain_needs_a_condition_its_words_and_one_value_per_fact
//!   M12 `validate_chain` lets a fact be set twice                                          -> a_chain_needs_a_condition_its_words_and_one_value_per_fact
//!   M13 `check_set_card` lets the exit filter on with no fallback                          -> the_exit_filter_cannot_be_turned_on_without_a_fallback
//!   M14 `check_set_exit_fallback` lets the fallback be cleared while the exit filter is on -> clearing_the_fallback_is_refused_while_the_exit_filter_is_on
//!   M15 `parse_exit_fallback` accepts the label itself                                     -> a_fallback_is_a_label_primary_or_junk_never_itself_or_all
//!   M16 `parse_exit_fallback` accepts "all"                                                -> a_fallback_is_a_label_primary_or_junk_never_itself_or_all
//!   M17 `count_label_flow` counts a fallback filing under the label it ended in            -> the_counts_follow_the_boards_example_and_never_double_count
//!   M18 `count_label_flow` leaves sent_to_fallback out of sorted_by_ai                     -> the_counts_follow_the_boards_example_and_never_double_count
//!   M19 `exit_verdict` rejects everything when the label has no exit chain                 -> the_exit_filter_keeps_by_chain_and_with_no_chain_keeps_everything
//!   M20 `matched_conditions` returns every condition in "any" mode                         -> why_here_names_the_card_the_chain_and_the_conditions_that_held
//!   M21 `why_here` keeps a chain id for the fallback card                                  -> why_here_names_the_card_the_chain_and_the_conditions_that_held
//!   M22 `next_chain_id` reuses the lowest free number                                      -> chain_numbers_are_never_reused_while_a_higher_one_lives
//!   M23 `check_set_ai_reads` lets all three off through                                    -> the_ai_reads_at_least_one_input_and_defaults_to_subject_and_body
//!   M24 `default_ai_reads` turns attachments on                                            -> the_ai_reads_at_least_one_input_and_defaults_to_subject_and_body
//!   M25 `flow_state_is_legal` allows exit on with no fallback                              -> the_stored_row_rule_is_the_same_as_the_command_rule
//!   M26 `check_view_command` skips SetLabelCard                                            -> a_view_has_no_flow_every_new_command_refuses_it
//!   M27 `builtin_labels` serves a built-in with the exit filter on                         -> a_label_that_existed_before_keeps_behaving_as_before
use contracts::*;

fn rs(mode: MatchMode, c: Vec<RuleCondition>) -> RuleSet { RuleSet { conditions: c, match_mode: mode } }
fn chain(id: u32, mode: MatchMode, c: Vec<RuleCondition>) -> LabelChain { LabelChain { chain_id: id, rules: rs(mode, c) } }
fn from(w: &str) -> RuleCondition { RuleCondition::From { words: w.into() } }
fn subj(w: &str) -> RuleCondition { RuleCondition::SubjectContains { words: w.into() } }
fn msg() -> MessageFacts {
    MessageFacts { message_id: "m1".into(), account_id: "acc-1".into(), from_name: "Laura Meier".into(), from_address: "laura.meier@nordlicht.example".into(),
        to: vec![], cc: vec![], subject: "Quote for Q4".into(), body: "".into(), has_attachment: false, list_header: "".into(), unread: false, received_at_ms: 1,
        unanswered: false, starred: false, vip_sender: false, cc_me: false, replied_by_me: false }
}
fn label(id: &str, exit_enabled: bool, fallback: Option<&str>) -> MailLabel {
    MailLabel { id: id.into(), name: id.into(), parent_id: None, colour: None, account_ids: vec![], path: id.into(), depth: 1, unread: 0, description: String::new(),
        pinned: false, scope: LabelScope::All, kind: LabelKind::Owner, order: 0, entry_enabled: true, ai_enabled: true, exit_enabled,
        ai_reads: default_ai_reads(), exit_fallback: fallback.map(String::from) }
}
fn code_of(r: Result<impl std::fmt::Debug, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("expected a refusal, got {other:?}") }
}

#[test]
fn chains_are_alternatives_the_lowest_matching_number_decides() {
    let chains = vec![chain(1, MatchMode::Any, vec![subj("quote")]), chain(0, MatchMode::Any, vec![from("laura")])];
    assert_eq!(first_matching_chain(chains.clone(), msg()), Some(0), "both match: the lowest number decides, whatever the list order");
    let only_second = MessageFacts { from_address: "x@y.com".into(), from_name: "X".into(), ..msg() };
    assert_eq!(first_matching_chain(chains.clone(), only_second), Some(1), "one chain matching is enough: alternatives, not AND");
    let neither = MessageFacts { from_address: "x@y.com".into(), from_name: "X".into(), subject: "hello".into(), ..msg() };
    assert_eq!(first_matching_chain(chains, neither), None);
    assert_eq!(first_matching_chain(vec![], msg()), None, "no chain matches nothing");
}

#[test]
fn inside_one_chain_the_mode_decides_and_all_means_and() {
    let both = chain(0, MatchMode::All, vec![from("laura"), subj("invoice")]);
    assert_eq!(first_matching_chain(vec![both.clone()], msg()), None, "All: the subject has no 'invoice', so the chain does not match");
    let any = chain(0, MatchMode::Any, vec![from("laura"), subj("invoice")]);
    assert_eq!(first_matching_chain(vec![any], msg()), Some(0), "Any: one condition is enough");
    assert_eq!(first_matching_chain(vec![chain(0, MatchMode::All, vec![from("laura"), subj("quote")])], msg()), Some(0));
    assert_eq!(first_matching_chain(vec![chain(0, MatchMode::All, vec![])], msg()), None, "an empty chain matches nothing, in both modes");
}

fn with(fact: MessageFact, on: bool) -> MessageFacts {
    let mut f = msg();
    match fact {
        MessageFact::Unread => f.unread = on, MessageFact::Unanswered => f.unanswered = on, MessageFact::HasAttachment => f.has_attachment = on,
        MessageFact::Starred => f.starred = on, MessageFact::VipSender => f.vip_sender = on, MessageFact::CcMe => f.cc_me = on,
        MessageFact::MailingList => f.list_header = if on { "<list.example.com>".into() } else { "".into() }, MessageFact::RepliedByMe => f.replied_by_me = on,
    }
    f
}

#[test]
fn each_fact_reads_its_own_field_in_three_states() {
    assert_eq!(message_facts_in_grid_order().len(), 8, "the grid has eight facts");
    for fact in message_facts_in_grid_order() {
        let (yes, no) = (fact_condition(fact, FactState::Yes).unwrap(), fact_condition(fact, FactState::No).unwrap());
        let (t, f) = (with(fact, true), with(fact, false));
        assert!(rule_set_matches(rs(MatchMode::All, vec![yes.clone()]), t.clone()), "{fact:?} yes matches a message where it holds");
        assert!(!rule_set_matches(rs(MatchMode::All, vec![yes]), f.clone()), "{fact:?} yes refuses a message where it does not");
        assert!(rule_set_matches(rs(MatchMode::All, vec![no.clone()]), f), "{fact:?} no matches a message where it does not hold");
        assert!(!rule_set_matches(rs(MatchMode::All, vec![no]), t), "{fact:?} no refuses a message where it holds");
        assert_eq!(fact_condition(fact, FactState::Any), None, "{fact:?} any is no condition");
        // one field per fact: setting the fact on never makes another fact true (a copy-paste field would)
        for other in message_facts_in_grid_order().into_iter().filter(|o| *o != fact) {
            assert!(!fact_holds(other, with(fact, true)), "{fact:?} set on must not turn {other:?} on");
        }
    }
}

#[test]
fn the_mailing_list_fact_is_derived_from_the_list_header() {
    assert!(fact_holds(MessageFact::MailingList, MessageFacts { list_header: "<news.example.com>".into(), ..msg() }));
    assert!(!fact_holds(MessageFact::MailingList, MessageFacts { list_header: "   ".into(), ..msg() }), "a blank header is not a list");
    assert!(!fact_holds(MessageFact::MailingList, msg()));
}

#[test]
fn the_grid_writes_one_state_per_fact_and_any_is_no_condition() {
    let start = vec![from("laura"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }, subj("quote")];
    let to_no = set_fact(start.clone(), MessageFact::Unread, FactState::No);
    assert_eq!(to_no, vec![from("laura"), subj("quote"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: false }], "the old condition goes, the others keep their place, the new one is last");
    let to_any = set_fact(start, MessageFact::Unread, FactState::Any);
    assert_eq!(to_any, vec![from("laura"), subj("quote")], "any removes the row");
    let cells = fact_states(vec![RuleCondition::Fact { fact: MessageFact::Starred, wanted: false }, RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }]);
    assert_eq!(cells.iter().map(|c| c.fact).collect::<Vec<_>>(), message_facts_in_grid_order(), "the grid is read in the board's order, all eight cells");
    assert_eq!(cells.iter().find(|c| c.fact == MessageFact::Starred).unwrap().state, FactState::No);
    assert_eq!(cells.iter().find(|c| c.fact == MessageFact::Unread).unwrap().state, FactState::Yes);
    assert_eq!(cells.iter().filter(|c| c.state == FactState::Any).count(), 6, "a fact with no condition reads any");
}

#[test]
fn pj_yes_keeps_the_old_spelling_and_both_read_as_one_fact() {
    assert_eq!(fact_condition(MessageFact::HasAttachment, FactState::Yes), Some(RuleCondition::HasAttachment));
    assert_eq!(fact_condition(MessageFact::HasAttachment, FactState::No), Some(RuleCondition::Fact { fact: MessageFact::HasAttachment, wanted: false }));
    let legacy = condition_fact(RuleCondition::HasAttachment).unwrap();
    assert_eq!((legacy.fact, legacy.wanted), (MessageFact::HasAttachment, true));
    let cell = |c: Vec<RuleCondition>| fact_states(c).into_iter().find(|s| s.fact == MessageFact::HasAttachment).unwrap().state;
    assert_eq!(cell(vec![RuleCondition::HasAttachment]), FactState::Yes, "a rule written before delta 17 fills the grid's cell");
    assert_eq!(cell(vec![RuleCondition::Fact { fact: MessageFact::HasAttachment, wanted: true }]), FactState::Yes);
    assert!(condition_fact(from("x")).is_none());
}

#[test]
fn the_rows_under_the_grid_are_the_text_conditions_only() {
    let all = vec![from("laura"), RuleCondition::HasAttachment, RuleCondition::Fact { fact: MessageFact::CcMe, wanted: true }, subj("quote"), RuleCondition::Domain { domain: "x.com".into() }];
    assert_eq!(row_conditions(all), vec![from("laura"), subj("quote"), RuleCondition::Domain { domain: "x.com".into() }]);
}

#[test]
fn a_chain_needs_a_condition_its_words_and_one_value_per_fact() {
    assert_eq!(code_of(validate_chain(rs(MatchMode::All, vec![]))).0, CHAIN_REFUSAL_CODE);
    assert_eq!(code_of(validate_chain(rs(MatchMode::All, vec![from("  ")]))).0, CHAIN_REFUSAL_CODE, "a text row with no words");
    let twice = rs(MatchMode::All, vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }, RuleCondition::Fact { fact: MessageFact::Unread, wanted: false }]);
    let (code, words) = code_of(validate_chain(twice));
    assert_eq!(code, CHAIN_REFUSAL_CODE);
    assert!(words.contains("unread") && words.contains("twice"), "the refusal names the fact: {words}");
    let twice_yes = rs(MatchMode::All, vec![RuleCondition::HasAttachment, RuleCondition::Fact { fact: MessageFact::HasAttachment, wanted: true }]);
    assert!(validate_chain(twice_yes).is_err(), "the two spellings of attachment-yes are one fact: set twice");
    assert!(validate_chain(rs(MatchMode::Any, vec![from("laura"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }])).is_ok());
}

#[test]
fn the_exit_filter_cannot_be_turned_on_without_a_fallback() {
    let (code, words) = code_of(check_set_card(label("a", false, None), FlowCard::Exit, true));
    assert_eq!(code, EXIT_NEEDS_FALLBACK_CODE);
    assert!(words.starts_with("Say where the other messages go"), "{words}");
    assert!(code_of(check_set_card(label("a", false, Some("  ")), FlowCard::Exit, true)).0 == EXIT_NEEDS_FALLBACK_CODE, "a blank fallback is none");
    assert!(check_set_card(label("a", false, Some("primary")), FlowCard::Exit, true).is_ok());
    assert!(check_set_card(label("a", false, None), FlowCard::Exit, false).is_ok(), "switching it off never needs one");
    assert!(check_set_card(label("a", false, None), FlowCard::Entry, true).is_ok() && check_set_card(label("a", false, None), FlowCard::Ai, false).is_ok(), "the other cards switch freely");
}

#[test]
fn clearing_the_fallback_is_refused_while_the_exit_filter_is_on() {
    let all = vec![label("a", true, Some("primary")), label("b", false, None)];
    assert_eq!(code_of(check_set_exit_fallback(all[0].clone(), None, all.clone())).0, EXIT_NEEDS_FALLBACK_CODE);
    assert!(check_set_exit_fallback(label("a", false, Some("primary")), None, all.clone()).is_ok(), "off: it can be cleared");
    assert!(check_set_exit_fallback(all[0].clone(), Some("b".into()), all).is_ok(), "changing it to another label is fine");
}

#[test]
fn a_fallback_is_a_label_primary_or_junk_never_itself_or_all() {
    let all = vec![label("a", false, None), label("b", false, None)];
    let parse = |f: &str| parse_exit_fallback("a".into(), f.into(), all.clone());
    assert_eq!(parse("primary").unwrap(), ExitFallback::Primary);
    assert_eq!(parse("junk").unwrap(), ExitFallback::Junk);
    assert_eq!(parse("b").unwrap(), ExitFallback::Label { label_id: "b".into() });
    assert_eq!(code_of(parse("a")).0, EXIT_FALLBACK_REFUSAL_CODE, "itself");
    assert_eq!(code_of(parse("all")).0, EXIT_FALLBACK_REFUSAL_CODE, "the All view is not a place");
    assert_eq!(code_of(parse("ghost")).0, EXIT_FALLBACK_REFUSAL_CODE, "an id that is no label");
}

#[test]
fn the_counts_follow_the_boards_example_and_never_double_count() {
    // the mac-main example: À répondre. Entry catches 1, the AI picked 31, the exit filter kept 23 and sent 8 to Primary.
    let mut filings: Vec<FlowFiling> = vec![FlowFiling { message_id: "e1".into(), ended_in: Some("needs-reply".into()), card: WhyCard::Entry, ai_pick: None }];
    filings.extend((0..23).map(|i| FlowFiling { message_id: format!("k{i}"), ended_in: Some("needs-reply".into()), card: WhyCard::Exit, ai_pick: Some("needs-reply".into()) }));
    filings.extend((0..8).map(|i| FlowFiling { message_id: format!("p{i}"), ended_in: None, card: WhyCard::Fallback, ai_pick: Some("needs-reply".into()) }));
    // noise that must not leak in: another label's filings, a hand filing, a fallback that landed IN needs-reply from another label
    filings.push(FlowFiling { message_id: "o1".into(), ended_in: Some("updates".into()), card: WhyCard::Ai, ai_pick: Some("updates".into()) });
    filings.push(FlowFiling { message_id: "h1".into(), ended_in: Some("needs-reply".into()), card: WhyCard::Hand, ai_pick: None });
    filings.push(FlowFiling { message_id: "f1".into(), ended_in: Some("needs-reply".into()), card: WhyCard::Fallback, ai_pick: Some("updates".into()) });
    let c = count_label_flow("needs-reply".into(), filings.clone());
    assert_eq!((c.caught_by_entry, c.sorted_by_ai, c.kept_by_exit, c.sent_to_fallback), (1, 31, 23, 8));
    let u = count_label_flow("updates".into(), filings);
    assert_eq!((u.caught_by_entry, u.sorted_by_ai, u.kept_by_exit, u.sent_to_fallback), (0, 2, 0, 1), "updates: its own AI pick, plus the one its exit filter would have sent away");
}

#[test]
fn the_exit_filter_keeps_by_chain_and_with_no_chain_keeps_everything() {
    let unanswered = chain(0, MatchMode::All, vec![RuleCondition::Fact { fact: MessageFact::Unanswered, wanted: true }]);
    assert_eq!(exit_verdict(vec![unanswered.clone()], with(MessageFact::Unanswered, true)), ExitVerdict::Kept { chain_id: Some(0) });
    assert_eq!(exit_verdict(vec![unanswered], with(MessageFact::Unanswered, false)), ExitVerdict::Rejected);
    assert_eq!(exit_verdict(vec![], msg()), ExitVerdict::Kept { chain_id: None }, "an exit filter with no chain filters nothing");
}

#[test]
fn why_here_names_the_card_the_chain_and_the_conditions_that_held() {
    let c = chain(2, MatchMode::Any, vec![from("laura"), subj("invoice"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }]);
    let f = with(MessageFact::Unread, true);
    let w = why_here(WhyCard::Entry, Some(c.clone()), f.clone(), DecidedBy::Rule { rule_id: "r-2".into() });
    assert_eq!((w.card, w.chain_id), (WhyCard::Entry, Some(2)));
    assert_eq!(w.matched, vec![from("laura"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }], "any mode: only the conditions that held, not the subject that did not");
    assert_eq!(w.decided_by, DecidedBy::Rule { rule_id: "r-2".into() });
    let fb = why_here(WhyCard::Fallback, Some(c.clone()), f.clone(), DecidedBy::Jev);
    assert_eq!((fb.chain_id, fb.matched.is_empty()), (None, true), "the fallback names no chain");
    let ai = why_here(WhyCard::Ai, None, f.clone(), DecidedBy::Laya);
    assert_eq!((ai.chain_id, ai.decided_by), (None, DecidedBy::Laya));
    let kept = why_here(WhyCard::Exit, Some(chain(0, MatchMode::All, vec![from("laura"), subj("quote")])), f, DecidedBy::Jev);
    assert_eq!((kept.chain_id, kept.matched.len()), (Some(0), 2), "all mode: every condition held");
}

#[test]
fn chain_numbers_are_never_reused_while_a_higher_one_lives() {
    assert_eq!(next_chain_id(vec![]), 0);
    assert_eq!(next_chain_id(vec![0]), 1);
    assert_eq!(next_chain_id(vec![0, 2]), 3, "chain 1 was removed: its number is not handed out again while 2 lives");
}

#[test]
fn the_ai_reads_at_least_one_input_and_defaults_to_subject_and_body() {
    assert_eq!(default_ai_reads(), AiReads { subject: true, body: true, rest_of_thread: false, attachments: false });
    assert_eq!(code_of(check_set_ai_reads(AiReads { subject: false, body: false, rest_of_thread: false, attachments: false })).0, AI_READS_NOTHING_CODE);
    for r in [AiReads { subject: true, body: false, rest_of_thread: false, attachments: false }, AiReads { subject: false, body: false, rest_of_thread: false, attachments: true }, default_ai_reads()] {
        assert!(check_set_ai_reads(r).is_ok(), "{r:?}");
    }
}

#[test]
fn the_stored_row_rule_is_the_same_as_the_command_rule() {
    assert!(flow_state_is_legal(false, None) && flow_state_is_legal(true, Some("primary".into())) && flow_state_is_legal(false, Some("junk".into())));
    assert!(!flow_state_is_legal(true, None) && !flow_state_is_legal(true, Some("  ".into())), "exit on with no fallback is the one illegal row");
}

#[test]
fn a_view_has_no_flow_every_new_command_refuses_it() {
    let cmds = vec![
        Command::SetLabelCard { label_id: "all".into(), card: FlowCard::Entry, enabled: true },
        Command::SetLabelAiReads { label_id: "primary".into(), reads: default_ai_reads() },
        Command::SetLabelExitFallback { label_id: "all".into(), fallback: Some("junk".into()) },
        Command::AddLabelChain { label_id: "primary".into(), placement: ChainPlacement::Entry, chain: rs(MatchMode::All, vec![from("a")]) },
        Command::UpdateLabelChain { label_id: "all".into(), placement: ChainPlacement::Exit, chain_id: 0, chain: rs(MatchMode::All, vec![from("a")]) },
        Command::RemoveLabelChain { label_id: "primary".into(), placement: ChainPlacement::Exit, chain_id: 0 },
    ];
    for c in cmds { assert_eq!(code_of(check_view_command(c.clone())).0, VIEW_REFUSAL_CODE, "{c:?}"); }
    let real = Command::SetLabelCard { label_id: "needs-reply".into(), card: FlowCard::Ai, enabled: false };
    assert!(check_view_command(real).is_ok(), "a real label passes the view door");
}

#[test]
fn a_label_that_existed_before_keeps_behaving_as_before() {
    for l in builtin_labels() {
        assert!(l.entry_enabled && l.ai_enabled && !l.exit_enabled, "{}: entry on, AI on, exit off", l.id);
        assert_eq!(l.ai_reads, default_ai_reads());
        assert_eq!(l.exit_fallback, None);
    }
}

#[test]
fn the_fact_keys_round_trip_and_an_unknown_key_is_none() {
    for f in message_facts_in_grid_order() { assert_eq!(message_fact_from_key(message_fact_key(f)), Some(f)); }
    assert_eq!(message_fact_from_key("starred ".into()), None);
    assert_eq!(message_fact_from_key("read".into()), None, "'read' is not a fact: the board says Non lu (unread)");
}
