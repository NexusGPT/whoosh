//! Delta 20 probes (docs/JOURNEYS.md J10, docs/PRD.md A109, the approved screens ai-replies-r7): AI replies. One test per rule; every test names its mutant.
//! MUTANTS (_captures/replies-contract/mutants.py applies each, runs `cargo test -p contracts --test replies_d20`, expects the named test RED, restores):
//!   M01 `reply_rule_passes` treats an empty rule like the label flow (matches nothing)           -> an_empty_rule_passes_everything_and_a_rule_follows_its_mode
//!   M02 `reply_rule_passes` ignores the rule's match mode (always any)                           -> an_empty_rule_passes_everything_and_a_rule_follows_its_mode
//!   M03 `validate_reply_rule` refuses an empty rule                                              -> a_reply_rule_is_checked_by_the_label_flow_and_may_be_empty
//!   M04 `validate_reply_rule` lets a fact be set twice through                                   -> a_reply_rule_is_checked_by_the_label_flow_and_may_be_empty
//!   M05 `reply_scope_admits(All, ..)` admits nothing                                             -> the_label_ticks_admit_all_or_a_carried_label_and_an_empty_list_admits_nothing
//!   M06 `reply_scope_admits(Labels, ..)` admits a message that carries none of them              -> the_label_ticks_admit_all_or_a_carried_label_and_an_empty_list_admits_nothing
//!   M07 `check_reply_scope` lets an empty list through                                           -> a_scope_names_a_label_that_exists_and_is_never_an_empty_list
//!   M08 `check_reply_ai_reads` refuses the thread-only reading                                   -> the_ai_reads_at_least_one_of_four_and_the_default_is_subject_body_thread
//!   M09 `check_reply_ai_reads` lets all four off through                                         -> the_ai_reads_at_least_one_of_four_and_the_default_is_subject_body_thread
//!   M10 `check_set_reply_card` lets the AI card on with a blank question                         -> the_ai_card_cannot_be_turned_on_without_its_question
//!   M11 `reply_entry_step` ignores the entry switch (a card off still stops)                     -> each_card_is_skipped_when_off_and_decides_when_on
//!   M12 `reply_ai_step` passes a message the model never answered                                -> each_card_is_skipped_when_off_and_decides_when_on
//!   M13 `reply_exit_step` stops a message the exit rule passes (inverted)                        -> each_card_is_skipped_when_off_and_decides_when_on
//!   M14 `count_reply_steps` counts a step without the steps before it                            -> the_counts_follow_the_boards_example_and_each_figure_is_a_subset
//!   M15 `count_reply_steps` counts a message listed twice twice                                  -> the_counts_follow_the_boards_example_and_each_figure_is_a_subset
//!   M16 `count_reply_steps` ignores the window (counts a message from 8 days ago)                -> the_counts_follow_the_boards_example_and_each_figure_is_a_subset
//!   M17 `reply_pass_is_legal` lets a draft exist after a stopped step                            -> a_pass_is_legal_only_when_a_stop_leaves_the_later_steps_unreached
//!   M18 `reply_source_key` drops the account (two accounts' "INBOX" share a key)                 -> a_source_key_round_trips_and_names_its_account
//!   M19 `check_local_folder_path` accepts a relative path                                        -> a_folder_of_this_mac_is_an_absolute_or_home_path_without_dots
//!   M20 `reply_account_sources` counts a disabled folder's items                                 -> an_account_card_derives_its_figures_from_its_tiles_and_the_pane_counts_every_enabled_source
//!   M21 `reply_sources_read` leaves the calendars out of `enabled_total`                          -> an_account_card_derives_its_figures_from_its_tiles_and_the_pane_counts_every_enabled_source
//!   M22 `sync_percent` lets a ring go past full                                                  -> a_sync_percent_never_passes_a_full_ring
//!   M23 `apply_retrieval_change(QuestionsMax)` accepts 0                                         -> each_retrieval_setting_changes_alone_and_a_wrong_value_is_refused
//!   M24 `apply_retrieval_change(Scope)` also resets the keep count                               -> each_retrieval_setting_changes_alone_and_a_wrong_value_is_refused
//!   M25 `apply_retrieval_change(AiFilterQuestion { Some("") })` is accepted                      -> an_ai_filter_is_off_by_an_explicit_none_never_by_a_blank_question
//!   M26 `check_reply_tone` lets a name another tone has through                                  -> a_tone_has_a_name_a_text_and_a_name_no_other_tone_has
//!   M27 `check_tone_choice` accepts an unknown tone                                              -> a_tone_choice_is_the_voice_or_an_existing_tone
//!   M28 `check_reply_prompt_text` lets an unknown variable through                               -> a_prompt_places_known_variables_closed_and_never_forgets_the_message
//!   M29 `check_reply_prompt_text` does not require {{message}}                                   -> a_prompt_places_known_variables_closed_and_never_forgets_the_message
//!   M30 `strip_prompt_notes` keeps the note lines                                                -> the_model_never_reads_the_note_lines_and_every_variable_is_replaced
//!   M31 `render_reply_prompt` leaves a variable with no value as written                         -> the_model_never_reads_the_note_lines_and_every_variable_is_replaced
//!   M32 `next_prompt_version` reuses the lowest free number                                      -> a_version_number_is_never_reused
//!   M33 `reply_draft_can_move` lets a sent draft move                                            -> the_draft_moves_are_closed_and_a_sealed_draft_changes_no_more
//!   M34 `check_reply_draft_open` opens a discarded draft                                         -> the_draft_moves_are_closed_and_a_sealed_draft_changes_no_more
//!   M35 `check_change_request` ignores the change switch                                         -> the_two_on_demand_switches_gate_their_field
//!   M36 `check_on_demand_instruction` ignores the instruction switch                             -> the_two_on_demand_switches_gate_their_field
//!   M37 `reply_draft_after_change` does not count the revision                                   -> a_change_rewrites_the_text_counts_one_revision_and_keeps_the_state
//!   M38 `reply_draft_after_accept` leaves the state Proposed                                     -> accepting_takes_the_draft_into_the_composer_once
//!   M39 `reply_draft_replaced` discards a sent draft                                             -> a_new_draft_replaces_only_the_live_one
//!   M40 `reply_use_kind_key(OnDemand)` spells "auto"                                             -> every_closed_word_has_one_spelling_that_round_trips
//!   M41 `conditions_json` spells the fact kind differently from the store's `rule.conditions_json` -> the_state_json_carries_the_keys_the_contract_names
//!   M42 `reply_filter_json` drops the rest-of-thread tick                                        -> the_state_json_carries_the_keys_the_contract_names
//!   M43 `reply_counts_json` spells `drafts` as `after_exit`                                      -> the_state_json_carries_the_keys_the_contract_names
//!   M44 a `Command` variant of delta 20 is removed from the guard table                          -> every_new_command_has_its_guard_and_its_code
//!   M45 `default_reply_prompt_text` differs from the migration's seed                            -> the_defaults_equal_the_migrations_seed
//!   M47 `check_set_ai_reads` counts the rest of the thread (a label AI that reads only the thread passes)  -> the_ai_reads_at_least_one_of_four_and_the_default_is_subject_body_thread
//!   M46 `reply_counts_days` answers 30 while REPLY_COUNTS_DAYS is 7                               -> the_counts_follow_the_boards_example_and_each_figure_is_a_subset
use contracts::*;
use std::collections::HashSet;

fn from(w: &str) -> RuleCondition { RuleCondition::From { words: w.into() } }
fn rs(mode: MatchMode, c: Vec<RuleCondition>) -> RuleSet { RuleSet { conditions: c, match_mode: mode } }
fn msg() -> MessageFacts {
    MessageFacts { message_id: "m1".into(), account_id: "acc-1".into(), from_name: "Laura Meier".into(), from_address: "laura.meier@nordlicht.example".into(),
        to: vec![], cc: vec![], subject: "Quote for Q4".into(), body: "".into(), has_attachment: false, list_header: "".into(), unread: false, received_at_ms: 1,
        unanswered: false, starred: false, vip_sender: false, cc_me: false, replied_by_me: false }
}
fn label(id: &str) -> MailLabel {
    MailLabel { id: id.into(), name: id.into(), parent_id: None, colour: None, account_ids: vec![], path: id.into(), depth: 1, unread: 0, description: String::new(),
        pinned: false, scope: LabelScope::All, kind: LabelKind::Owner, order: 0, entry_enabled: true, ai_enabled: true, exit_enabled: false,
        ai_reads: default_ai_reads(), exit_fallback: None }
}
fn code_of<T: std::fmt::Debug>(r: Result<T, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("expected a refusal, got {other:?}") }
}
fn not_found<T: std::fmt::Debug>(r: Result<T, CoreError>) { assert!(matches!(r, Err(CoreError::NotFound { .. })), "expected NotFound, got {r:?}"); }
fn ids(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

// ---------------------------------------------------------------- the filter (record ReplyFilter, ReplyLabelScope, AiReads)

#[test]
fn an_empty_rule_passes_everything_and_a_rule_follows_its_mode() {
    assert!(reply_rule_passes(rs(MatchMode::All, vec![]), msg()), "no condition = nothing to filter by = passes");
    assert!(reply_rule_passes(rs(MatchMode::Any, vec![]), msg()));
    assert!(!rule_set_matches(rs(MatchMode::All, vec![]), msg()), "the label flow's own reading of an empty set stays: matches nothing");
    let both = rs(MatchMode::All, vec![from("laura"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }]);
    assert!(!reply_rule_passes(both, msg()), "All: the message is not unread, so it fails");
    let any = rs(MatchMode::Any, vec![from("laura"), RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }]);
    assert!(reply_rule_passes(any, msg()), "Any: the sender is enough");
}

#[test]
fn a_reply_rule_is_checked_by_the_label_flow_and_may_be_empty() {
    validate_reply_rule(rs(MatchMode::All, vec![])).expect("an empty rule is legal: it passes everything");
    validate_reply_rule(rs(MatchMode::Any, vec![from("laura"), RuleCondition::Fact { fact: MessageFact::CcMe, wanted: false }])).unwrap();
    let (code, _) = code_of(validate_reply_rule(rs(MatchMode::All, vec![from("  ")])));
    assert_eq!(code, REPLY_BAD_RULE_CODE, "a text row with no words, re-coded from the label flow's code");
    let twice = rs(MatchMode::All, vec![RuleCondition::Fact { fact: MessageFact::Unread, wanted: true }, RuleCondition::Fact { fact: MessageFact::Unread, wanted: false }]);
    assert_eq!(code_of(validate_reply_rule(twice)).0, REPLY_BAD_RULE_CODE, "one value per fact");
}

#[test]
fn the_label_ticks_admit_all_or_a_carried_label_and_an_empty_list_admits_nothing() {
    assert!(reply_scope_admits(ReplyLabelScope::All, vec![]), "All admits a message with no label at all");
    let some = ReplyLabelScope::Labels { label_ids: ids(&["needs-reply", "clients"]) };
    assert!(reply_scope_admits(some.clone(), ids(&["clients", "x"])), "one carried label is enough");
    assert!(!reply_scope_admits(some, ids(&["promotions"])), "a message with none of them is not admitted");
    assert!(!reply_scope_admits(ReplyLabelScope::Labels { label_ids: vec![] }, ids(&["clients"])), "an emptied list (every ticked label was deleted) admits nothing");
}

#[test]
fn a_scope_names_a_label_that_exists_and_is_never_an_empty_list() {
    assert_eq!(code_of(check_reply_scope(ReplyLabelScope::Labels { label_ids: vec![] })).0, REPLY_SCOPE_EMPTY_CODE);
    assert_eq!(code_of(check_reply_scope(ReplyLabelScope::Labels { label_ids: ids(&["a", " "]) })).0, REPLY_SCOPE_EMPTY_CODE, "a blank id");
    check_reply_scope(ReplyLabelScope::All).unwrap();
    let labels = vec![label("clients")];
    check_reply_scope_labels(ReplyLabelScope::Labels { label_ids: ids(&["clients"]) }, labels.clone()).unwrap();
    not_found(check_reply_scope_labels(ReplyLabelScope::Labels { label_ids: ids(&["clients", "ghost"]) }, labels));
}

#[test]
fn the_ai_reads_at_least_one_of_four_and_the_default_is_subject_body_thread() {
    let d = default_reply_ai_reads();
    assert_eq!((d.subject, d.body, d.rest_of_thread, d.attachments), (true, true, true, false), "the board ticks Objet, Corps, Reste du fil, not Pièces jointes");
    check_reply_ai_reads(d).unwrap();
    let thread_only = AiReads { subject: false, body: false, rest_of_thread: true, attachments: false };
    assert!(!default_ai_reads().rest_of_thread, "a label starts without the thread; the reply default adds it");
    assert_eq!(code_of(check_set_ai_reads(thread_only)).0, AI_READS_NOTHING_CODE, "the label AI keeps its rule: the thread alone is no reading");
    check_reply_ai_reads(thread_only).expect("the rest of the thread alone is a reading");
    let nothing = AiReads { rest_of_thread: false, ..thread_only };
    assert_eq!(code_of(check_reply_ai_reads(nothing)).0, REPLY_READS_NOTHING_CODE);
}

#[test]
fn the_ai_card_cannot_be_turned_on_without_its_question() {
    let mut f = default_reply_filter();
    f.ai_description = "  ".into();
    assert_eq!(code_of(check_set_reply_card(f.clone(), FlowCard::Ai, true)).0, REPLY_AI_NEEDS_DESCRIPTION_CODE);
    check_set_reply_card(f.clone(), FlowCard::Ai, false).expect("off is always fine");
    check_set_reply_card(f.clone(), FlowCard::Entry, true).expect("the other cards never need a question");
    check_set_reply_card(f, FlowCard::Exit, true).unwrap();
    assert_eq!(code_of(check_reply_ai_description(true, "".into())).0, REPLY_AI_NEEDS_DESCRIPTION_CODE);
    check_reply_ai_description(false, "".into()).expect("a blank question is fine while the card is off");
    check_reply_ai_description(true, default_reply_ai_description()).unwrap();
}

#[test]
fn each_card_is_skipped_when_off_and_decides_when_on() {
    let mut f = default_reply_filter();
    f.scope = ReplyLabelScope::Labels { label_ids: ids(&["clients"]) };
    f.entry_rule = rs(MatchMode::All, vec![RuleCondition::Fact { fact: MessageFact::MailingList, wanted: false }]);
    f.exit_rule = rs(MatchMode::All, vec![RuleCondition::Fact { fact: MessageFact::RepliedByMe, wanted: false }]);
    // entry
    assert_eq!(reply_entry_step(f.clone(), ids(&["clients"]), msg()), ReplyStepResult::Passed);
    assert_eq!(reply_entry_step(f.clone(), ids(&["promotions"]), msg()), ReplyStepResult::Stopped, "the label is not ticked");
    let list = MessageFacts { list_header: "<l.example.com>".into(), ..msg() };
    assert_eq!(reply_entry_step(f.clone(), ids(&["clients"]), list.clone()), ReplyStepResult::Stopped, "the rule says not a mailing list");
    let off = ReplyFilter { entry_enabled: false, ..f.clone() };
    assert_eq!(reply_entry_step(off, ids(&["promotions"]), list), ReplyStepResult::Skipped, "a card that is off lets everything through");
    // ai
    assert_eq!(reply_ai_step(f.clone(), Some(true)), ReplyStepResult::Passed);
    assert_eq!(reply_ai_step(f.clone(), Some(false)), ReplyStepResult::Stopped);
    assert_eq!(reply_ai_step(f.clone(), None), ReplyStepResult::NotReached, "no answer from the model: asked again, neither passed nor stopped");
    assert_eq!(reply_ai_step(ReplyFilter { ai_enabled: false, ..f.clone() }, Some(false)), ReplyStepResult::Skipped);
    // exit
    assert_eq!(reply_exit_step(f.clone(), msg()), ReplyStepResult::Passed);
    assert_eq!(reply_exit_step(f.clone(), MessageFacts { replied_by_me: true, ..msg() }), ReplyStepResult::Stopped, "Déjà répondu par moi: non");
    assert_eq!(reply_exit_step(ReplyFilter { exit_enabled: false, ..f }, MessageFacts { replied_by_me: true, ..msg() }), ReplyStepResult::Skipped);
}

// ---------------------------------------------------------------- the steps and their counts (record ReplyPass, ReplyStepCounts)

const DAY: i64 = 86_400_000;
fn pass(id: &str, at: i64, e: ReplyStepResult, a: ReplyStepResult, x: ReplyStepResult, draft: bool) -> ReplyPass {
    ReplyPass { message_id: id.into(), received_at_ms: at, entry: e, ai: a, exit: x, draft_id: draft.then(|| format!("d-{id}")) }
}

#[test]
fn the_counts_follow_the_boards_example_and_each_figure_is_a_subset() {
    use ReplyStepResult::*;
    let now = 100 * DAY;
    let mut v: Vec<ReplyPass> = vec![];
    // the board: 128 received, 31 pass the entry card, 12 the AI, 9 the exit card, 9 drafts
    for i in 0..97 { v.push(pass(&format!("e{i}"), now - DAY, Stopped, NotReached, NotReached, false)); }
    for i in 0..19 { v.push(pass(&format!("a{i}"), now - 2 * DAY, Passed, Stopped, NotReached, false)); }
    for i in 0..3 { v.push(pass(&format!("x{i}"), now - 3 * DAY, Passed, Passed, Stopped, false)); }
    for i in 0..9 { v.push(pass(&format!("d{i}"), now - 4 * DAY, Passed, Passed, Passed, true)); }
    let c = count_reply_steps(v.clone(), now, REPLY_COUNTS_DAYS);
    assert_eq!((c.days, c.received, c.after_entry, c.after_ai, c.after_exit, c.drafts), (7, 128, 31, 12, 9, 9));
    assert_eq!(reply_counts_days(), 7, "the Swift side asks for the same number of days");
    // a message from 8 days ago and one from the future are not in the window
    let mut noisy = v.clone();
    noisy.push(pass("old", now - 8 * DAY, Passed, Passed, Passed, true));
    noisy.push(pass("future", now + DAY, Passed, Passed, Passed, true));
    assert_eq!(count_reply_steps(noisy, now, 7), c, "the window is (now - 7 days, now]");
    // a message listed twice counts once, by its last row
    let mut twice = v.clone();
    twice.push(pass("e0", now - DAY, Stopped, NotReached, NotReached, false));
    twice.push(pass("d0", now - 4 * DAY, Passed, Passed, Passed, true));
    assert_eq!(count_reply_steps(twice, now, 7), c, "one row per message");
    // a step is counted only when every step before it let the message through (an illegal row cannot inflate the later figures)
    let bad = vec![pass("bad", now - DAY, Stopped, Passed, Passed, true)];
    let b = count_reply_steps(bad, now, 7);
    assert_eq!((b.received, b.after_entry, b.after_ai, b.after_exit, b.drafts), (1, 0, 0, 0, 0));
    // skipped counts as through
    let s = count_reply_steps(vec![pass("s", now - DAY, Skipped, Skipped, Skipped, true)], now, 7);
    assert_eq!((s.after_entry, s.after_ai, s.after_exit, s.drafts), (1, 1, 1, 1));
    assert!(c.received >= c.after_entry && c.after_entry >= c.after_ai && c.after_ai >= c.after_exit && c.after_exit >= c.drafts, "each figure is a subset of the one before");
}

#[test]
fn a_pass_is_legal_only_when_a_stop_leaves_the_later_steps_unreached() {
    use ReplyStepResult::*;
    assert!(reply_pass_is_legal(pass("m", 1, Passed, Passed, Passed, true)));
    assert!(reply_pass_is_legal(pass("m", 1, Skipped, Skipped, Skipped, true)));
    assert!(reply_pass_is_legal(pass("m", 1, Passed, Stopped, NotReached, false)));
    assert!(reply_pass_is_legal(pass("m", 1, Passed, NotReached, NotReached, false)), "the model has not answered yet");
    assert!(!reply_pass_is_legal(pass("m", 1, NotReached, NotReached, NotReached, false)), "the first step is always reached");
    assert!(!reply_pass_is_legal(pass("m", 1, Stopped, Passed, NotReached, false)), "after a stop nothing is reached");
    assert!(!reply_pass_is_legal(pass("m", 1, Passed, Stopped, Passed, false)));
    assert!(!reply_pass_is_legal(pass("m", 1, Passed, NotReached, Passed, false)));
    assert!(!reply_pass_is_legal(pass("m", 1, Passed, Passed, Stopped, true)), "a draft after a stop");
    assert!(!reply_pass_is_legal(pass("m", 1, Passed, Passed, NotReached, true)), "a draft before the exit card answered");
}

// ---------------------------------------------------------------- sources (records ReplySource, ReplySourceRef, ReplySyncProgress)

#[test]
fn a_source_key_round_trips_and_names_its_account() {
    let a = ReplySourceRef::MailFolder { account_id: "acc-1".into(), folder: "INBOX".into() };
    let b = ReplySourceRef::MailFolder { account_id: "acc-2".into(), folder: "INBOX".into() };
    assert_ne!(reply_source_key(a.clone()), reply_source_key(b.clone()), "two accounts' INBOX are two sources");
    for r in [a, b, ReplySourceRef::LocalFolder { path: "~/Documents/Contrats".into() }, ReplySourceRef::Calendar { calendar_id: "cal-1".into() }] {
        assert_eq!(reply_source_from_key(reply_source_key(r.clone())), Some(r.clone()), "{r:?}");
    }
    assert_eq!(reply_source_from_key("mail\u{1f}acc-1".into()), None, "a mail key needs its folder");
    assert_eq!(reply_source_from_key("fax\u{1f}x".into()), None);
    assert_eq!(reply_source_from_key("".into()), None);
}

#[test]
fn a_folder_of_this_mac_is_an_absolute_or_home_path_without_dots() {
    assert_eq!(check_local_folder_path(" ~/Documents/Contrats/ ".into()).unwrap(), "~/Documents/Contrats", "trimmed, no trailing slash");
    assert_eq!(check_local_folder_path("/Users/me/Docs".into()).unwrap(), "/Users/me/Docs");
    assert_eq!(check_local_folder_path("/".into()).unwrap(), "/");
    for bad in ["", "   ", "Documents/Contrats", "./x", "~me/x", "/Users/me/../other", "/a\0b"] {
        assert_eq!(code_of(check_local_folder_path(bad.into())).0, REPLY_BAD_FOLDER_CODE, "{bad:?}");
    }
    assert_eq!(code_of(check_source_ref(ReplySourceRef::MailFolder { account_id: "".into(), folder: "INBOX".into() })).0, REPLY_BAD_FOLDER_CODE);
    assert_eq!(code_of(check_source_ref(ReplySourceRef::Calendar { calendar_id: " ".into() })).0, REPLY_BAD_FOLDER_CODE);
    check_source_ref(ReplySourceRef::LocalFolder { path: "~/x".into() }).unwrap();
}

fn src(account: &str, folder: &str, on: bool, n: u64) -> ReplySource {
    ReplySource { source: ReplySourceRef::MailFolder { account_id: account.into(), folder: folder.into() }, enabled: on, item_count: n, progress: ReplySyncProgress::UpToDate }
}

#[test]
fn an_account_card_derives_its_figures_from_its_tiles_and_the_pane_counts_every_enabled_source() {
    // the board's card: 4 of 6 folders enabled, 9 204 + 4 812 + 2 960 + 1 120 = 18 096 messages in the enabled ones
    let folders = vec![src("acc-1", "INBOX", true, 9_204), src("acc-1", "Sent", true, 4_812), src("acc-1", "Clients", true, 2_960), src("acc-1", "Clients/Lorvan Hotels", true, 1_120),
        src("acc-1", "Clients/Oranta", false, 1_840), src("acc-1", "Newsletters", false, 6_310), src("acc-2", "INBOX", true, 5)];
    let card = reply_account_sources("acc-1".into(), folders.clone());
    assert_eq!((card.total_folders, card.enabled_folders, card.enabled_items), (6, 4, 18_096), "another account's folder is not on this card, a disabled folder's items are not counted");
    assert_eq!(card.folders.len(), 6);
    let other = reply_account_sources("acc-2".into(), folders);
    assert_eq!((other.total_folders, other.enabled_folders, other.enabled_items), (1, 1, 5));
    let local = vec![ReplySource { source: ReplySourceRef::LocalFolder { path: "~/Documents/Contrats".into() }, enabled: true, item_count: 214, progress: ReplySyncProgress::UpToDate },
        ReplySource { source: ReplySourceRef::LocalFolder { path: "~/Documents/Propositions".into() }, enabled: false, item_count: 86, progress: ReplySyncProgress::NotStarted }];
    let cals = vec![ReplyCalendar { calendar_id: "c1".into(), name: "Travail".into(), account_label: "assem@nexus-example.com".into(), colour: "#007AFF".into(), enabled: true },
        ReplyCalendar { calendar_id: "c2".into(), name: "Personnel".into(), account_label: "assem@icloud.com".into(), colour: "#34C759".into(), enabled: false }];
    let pane = reply_sources_read(vec![card, other], local, cals, RetrievalScope::ThisAccount);
    assert_eq!(pane.enabled_total, 4 + 1 + 1 + 1, "every enabled source of the three kinds: mail folders, folders of this Mac, calendars");
}

#[test]
fn a_sync_percent_never_passes_a_full_ring() {
    assert_eq!((sync_percent(0, 0), sync_percent(0, 10), sync_percent(5, 10), sync_percent(10, 10), sync_percent(15, 10), sync_percent(62, 100)), (0, 0, 50, 100, 100, 62));
}

// ---------------------------------------------------------------- retrieval (record RetrievalSettings)

#[test]
fn each_retrieval_setting_changes_alone_and_a_wrong_value_is_refused() {
    let d = default_retrieval_settings();
    assert_eq!((d.questions_max, d.excerpts_per_question, d.keep_n, d.excerpt_size, d.clash_rule, d.scope), (3, 5, 6, ExcerptSize::Paragraph, ClashRule::KeepBothDated, RetrievalScope::ThisAccount), "the board's lines");
    let apply = |c| apply_retrieval_change(d.clone(), c).unwrap();
    assert_eq!(apply(RetrievalChange::QuestionsMax { n: 4 }), RetrievalSettings { questions_max: 4, ..d.clone() });
    assert_eq!(apply(RetrievalChange::ExcerptsPerQuestion { n: 8 }), RetrievalSettings { excerpts_per_question: 8, ..d.clone() });
    assert_eq!(apply(RetrievalChange::KeepN { n: 10 }), RetrievalSettings { keep_n: 10, ..d.clone() });
    assert_eq!(apply(RetrievalChange::ExcerptSize { size: ExcerptSize::Section }), RetrievalSettings { excerpt_size: ExcerptSize::Section, ..d.clone() });
    assert_eq!(apply(RetrievalChange::Clash { rule: ClashRule::KeepNewest }), RetrievalSettings { clash_rule: ClashRule::KeepNewest, ..d.clone() });
    assert_eq!(apply(RetrievalChange::Scope { scope: RetrievalScope::AllAccounts }), RetrievalSettings { scope: RetrievalScope::AllAccounts, ..d.clone() }, "the scope changes alone: nothing else moves");
    let none = ExcerptFields { date: false, sender: false, folder: false, subject: false };
    assert_eq!(apply(RetrievalChange::AttachedFields { fields: none }).attached_fields, none, "attaching nothing is a value");
    let p = "From {{message}} write 2 questions.".to_string();
    assert_eq!(apply(RetrievalChange::Prompt { text: p.clone() }).retrieval_prompt, p);
    for (c, why) in [(RetrievalChange::QuestionsMax { n: 0 }, "zero questions"), (RetrievalChange::QuestionsMax { n: REPLY_QUESTIONS_CEILING + 1 }, "too many questions"),
        (RetrievalChange::ExcerptsPerQuestion { n: 0 }, "zero excerpts"), (RetrievalChange::KeepN { n: REPLY_KEEP_CEILING + 1 }, "keep too many"),
        (RetrievalChange::Prompt { text: "  ".into() }, "an empty prompt"), (RetrievalChange::Prompt { text: "Use {{tone}}".into() }, "a variable the retrieval prompt does not have"),
        (RetrievalChange::Prompt { text: "Use {{message".into() }, "an unclosed variable")] {
        assert_eq!(code_of(apply_retrieval_change(d.clone(), c)).0, REPLY_BAD_SETTING_CODE, "{why}");
    }
}

#[test]
fn an_ai_filter_is_off_by_an_explicit_none_never_by_a_blank_question() {
    let d = default_retrieval_settings();
    assert!(d.ai_filter_question.is_some());
    let off = apply_retrieval_change(d.clone(), RetrievalChange::AiFilterQuestion { question: None }).unwrap();
    assert_eq!(off.ai_filter_question, None, "None switches the filter off: an explicit value, not 'unchanged'");
    let on = apply_retrieval_change(off, RetrievalChange::AiFilterQuestion { question: Some("  Does it help?  ".into()) }).unwrap();
    assert_eq!(on.ai_filter_question.as_deref(), Some("Does it help?"), "trimmed");
    assert_eq!(code_of(apply_retrieval_change(d, RetrievalChange::AiFilterQuestion { question: Some("  ".into()) })).0, REPLY_BAD_SETTING_CODE);
}

// ---------------------------------------------------------------- voice and tones (records ReplyVoice, ReplyTone)

fn tone(id: &str, name: &str) -> ReplyTone { ReplyTone { id: id.into(), name: name.into(), text: "Short.".into() } }

#[test]
fn a_tone_has_a_name_a_text_and_a_name_no_other_tone_has() {
    let existing = vec![tone("t1", "Formel"), tone("t2", "Bref")];
    check_reply_tone(tone("", "Chaleureux"), existing.clone()).unwrap();
    check_reply_tone(tone("t1", "Formel"), existing.clone()).expect("saving a tone under its own name is not a clash");
    assert_eq!(code_of(check_reply_tone(tone("", " formel "), existing.clone())).0, REPLY_BAD_TONE_CODE, "another tone has that name, any case");
    assert_eq!(code_of(check_reply_tone(tone("", " "), existing.clone())).0, REPLY_BAD_TONE_CODE);
    assert_eq!(code_of(check_reply_tone(ReplyTone { text: " ".into(), ..tone("", "X") }, existing.clone())).0, REPLY_BAD_TONE_CODE);
    assert_eq!(code_of(check_reply_tone(tone(REPLY_TONE_VOICE_ID, "Mine"), existing)).0, REPLY_BAD_TONE_CODE, "the voice's id is reserved");
    let v = default_reply_voice();
    assert_eq!((v.tone_text.as_str(), v.sample_size, v.analysed_at_ms, v.same_person_examples), ("", 100, None, 3), "never analysed");
    assert_eq!(check_voice_sample_size(100).unwrap(), 100);
    assert_eq!(code_of(check_voice_sample_size(0)).0, REPLY_BAD_SAMPLE_CODE);
    assert_eq!(code_of(check_voice_sample_size(REPLY_SAMPLE_CEILING + 1)).0, REPLY_BAD_SAMPLE_CODE);
    assert_eq!(check_same_person_examples(0).unwrap(), 0, "none is a value");
    assert_eq!(code_of(check_same_person_examples(REPLY_EXAMPLES_CEILING + 1)).0, REPLY_BAD_SAMPLE_CODE);
}

#[test]
fn a_tone_choice_is_the_voice_or_an_existing_tone() {
    let tones = vec![tone("t1", "Formel")];
    check_tone_choice(REPLY_TONE_VOICE_ID.into(), tones.clone()).unwrap();
    check_tone_choice("t1".into(), tones.clone()).unwrap();
    not_found(check_tone_choice("ghost".into(), tones));
}

// ---------------------------------------------------------------- prompt and notes (records ReplyPrompt, FixedNote)

#[test]
fn a_prompt_places_known_variables_closed_and_never_forgets_the_message() {
    let ids: Vec<String> = reply_prompt_variables().into_iter().map(|v| v.id).collect();
    assert_eq!(ids, ids_of(&["message", "thread", "instruction", "excerpts", "calendar", "tone", "notes"]), "the seven variables of the board, in its order");
    assert!(reply_prompt_variables().iter().all(|v| v.name_key == format!("reply.var.{}.name", v.id) && v.meaning_key == format!("reply.var.{}.meaning", v.id)));
    check_reply_prompt_text(default_reply_prompt_text()).unwrap();
    check_reply_prompt_text("Reply to {{message}}.".into()).unwrap();
    for (text, why) in [("", "empty"), ("// only a note", "only note lines"), ("Reply to {{message}} as {{style}}", "an unknown variable"), ("Reply to {{message}} {{tone", "an unclosed variable"),
        ("Reply to {{message}} }}", "a stray closer"), ("Just write something kind.", "no {{message}}"), ("Use {{tone}} and {{notes}}", "the message is never placed")] {
        assert_eq!(code_of(check_reply_prompt_text(text.into())).0, REPLY_BAD_PROMPT_CODE, "{why}");
    }
    // a variable placed only inside a note line does not count as placed
    assert_eq!(code_of(check_reply_prompt_text("Be kind.\n// reply to {{message}}".into())).0, REPLY_BAD_PROMPT_CODE);
    assert_eq!(prompt_variables_used("a {{tone}} b {{message}} c {{tone}} {{ notes }} {{unclosed".into()), ids_of(&["tone", "message", "notes"]), "first use order, each once, an unclosed one is not a variable");
    assert_eq!(prompt_placeholder("thread".into()), "{{thread}}");
}
fn ids_of(v: &[&str]) -> Vec<String> { ids(v) }

#[test]
fn the_model_never_reads_the_note_lines_and_every_variable_is_replaced() {
    let text = "Reply to {{message}} as {{tone}}.\n  // a note for the owner, with {{notes}}\nEnd.";
    assert_eq!(strip_prompt_notes(text.into()), "Reply to {{message}} as {{tone}}.\nEnd.");
    let out = render_reply_prompt(text.into(), vec![PromptValue { id: "message".into(), value: "the mail".into() }]);
    assert_eq!(out, "Reply to the mail as .\nEnd.", "a variable with no value is replaced by nothing; the note line is gone");
    let all: Vec<PromptValue> = reply_prompt_variables().into_iter().map(|v| PromptValue { value: format!("<{}>", v.id), id: v.id }).collect();
    let full = render_reply_prompt(default_reply_prompt_text(), all);
    assert!(!full.contains("{{") && !full.contains("//"), "nothing placeholder-shaped or note-shaped reaches the model: {full}");
    assert!(full.contains("<message>") && full.contains("<notes>"));
}

#[test]
fn a_version_number_is_never_reused() {
    assert_eq!(next_prompt_version(vec![]), 1);
    assert_eq!(next_prompt_version(vec![1, 2, 3]), 4);
    assert_eq!(next_prompt_version(vec![1, 3]), 4, "a hole is not filled");
    check_prompt_version_exists(vec![1, 2], 2).unwrap();
    not_found(check_prompt_version_exists(vec![1, 2], 3));
    assert_eq!(code_of(check_fixed_note(FixedNote { id: "".into(), title: " ".into(), text: "x".into() })).0, REPLY_BAD_NOTE_CODE);
    assert_eq!(code_of(check_fixed_note(FixedNote { id: "".into(), title: "Prix".into(), text: " ".into() })).0, REPLY_BAD_NOTE_CODE);
    check_fixed_note(FixedNote { id: "".into(), title: "Prix".into(), text: "Never quote a price.".into() }).unwrap();
}

// ---------------------------------------------------------------- drafts (record ReplyDraft)

fn draft(state: ReplyDraftState) -> ReplyDraft {
    ReplyDraft { id: "d1".into(), thread_id: "t1".into(), message_id: "m1".into(), text: "Bonjour,".into(), made_by: ReplyMadeBy::Auto, excerpt_ids: ids(&["c1"]), instruction: None,
        state, revision: 0, created_at_ms: 10, updated_at_ms: 10 }
}

#[test]
fn the_draft_moves_are_closed_and_a_sealed_draft_changes_no_more() {
    use ReplyDraftState::*;
    let all = [Proposed, Edited, Sent, Discarded];
    let legal: HashSet<(ReplyDraftState, ReplyDraftState)> = [(Proposed, Edited), (Proposed, Sent), (Proposed, Discarded), (Edited, Sent), (Edited, Discarded)].into_iter().collect();
    for f in all { for t in all { assert_eq!(reply_draft_can_move(f, t), legal.contains(&(f, t)), "{f:?} -> {t:?}"); } }
    for sealed in [Sent, Discarded] {
        let d = draft(sealed);
        assert_eq!(code_of(check_reply_draft_open(d.clone())).0, REPLY_DRAFT_CLOSED_CODE, "{sealed:?}");
        assert_eq!(code_of(reply_draft_after_change(d.clone(), "x".into(), 20)).0, REPLY_DRAFT_CLOSED_CODE);
        assert_eq!(code_of(reply_draft_after_accept(d.clone(), 20)).0, REPLY_DRAFT_CLOSED_CODE);
        assert_eq!(code_of(reply_draft_after_discard(d.clone(), 20)).0, REPLY_DRAFT_CLOSED_CODE);
        assert_eq!(code_of(reply_draft_after_sent(d, 20)).0, REPLY_DRAFT_CLOSED_CODE);
    }
    check_reply_draft_open(draft(Proposed)).unwrap();
    check_reply_draft_open(draft(Edited)).unwrap();
}

#[test]
fn the_two_on_demand_switches_gate_their_field() {
    let on = default_on_demand_options();
    let no_instruction = OnDemandOptions { instruction_before: false, ..on };
    let no_change = OnDemandOptions { change_after: false, ..on };
    assert_eq!(check_on_demand_instruction(on, Some("  say yes  ".into())).unwrap(), Some("say yes".into()), "trimmed");
    assert_eq!(check_on_demand_instruction(on, Some("   ".into())).unwrap(), None, "blank = none");
    assert_eq!(check_on_demand_instruction(no_instruction, None).unwrap(), None, "no instruction is always fine");
    assert_eq!(code_of(check_on_demand_instruction(no_instruction, Some("say yes".into()))).0, REPLY_INSTRUCTION_OFF_CODE);
    assert_eq!(check_change_request(on, draft(ReplyDraftState::Proposed), "  shorter ".into()).unwrap(), "shorter");
    assert_eq!(code_of(check_change_request(no_change, draft(ReplyDraftState::Proposed), "shorter".into())).0, REPLY_CHANGE_OFF_CODE);
    assert_eq!(code_of(check_change_request(on, draft(ReplyDraftState::Proposed), "  ".into())).0, REPLY_BAD_TEXT_CODE);
    assert_eq!(code_of(check_change_request(on, draft(ReplyDraftState::Sent), "shorter".into())).0, REPLY_DRAFT_CLOSED_CODE, "a closed draft is refused first");
}

#[test]
fn a_change_rewrites_the_text_counts_one_revision_and_keeps_the_state() {
    let d = draft(ReplyDraftState::Edited);
    let c = reply_draft_after_change(d.clone(), "Bonjour, voici.".into(), 99).unwrap();
    assert_eq!((c.text.as_str(), c.revision, c.state, c.updated_at_ms, c.created_at_ms, c.excerpt_ids.clone()), ("Bonjour, voici.", 1, ReplyDraftState::Edited, 99, 10, d.excerpt_ids.clone()));
    assert_eq!(reply_draft_after_change(c, "Encore.".into(), 100).unwrap().revision, 2);
    assert_eq!(code_of(reply_draft_after_change(d, "  ".into(), 5)).0, REPLY_BAD_TEXT_CODE, "the model returned nothing");
}

#[test]
fn accepting_takes_the_draft_into_the_composer_once() {
    let d = draft(ReplyDraftState::Proposed);
    let a = reply_draft_after_accept(d, 50).unwrap();
    assert_eq!((a.state, a.updated_at_ms), (ReplyDraftState::Edited, 50));
    assert_eq!(code_of(reply_draft_after_accept(a.clone(), 60)).0, REPLY_DRAFT_CLOSED_CODE, "already in the composer");
    assert_eq!(reply_draft_after_discard(a.clone(), 70).unwrap().state, ReplyDraftState::Discarded);
    assert_eq!(reply_draft_after_sent(a, 80).unwrap().state, ReplyDraftState::Sent);
    assert_eq!(reply_draft_after_sent(draft(ReplyDraftState::Proposed), 80).unwrap().state, ReplyDraftState::Sent, "sent from the reply box without opening the composer");
}

#[test]
fn a_new_draft_replaces_only_the_live_one() {
    assert_eq!(reply_draft_replaced(None, 5), None);
    for live in [ReplyDraftState::Proposed, ReplyDraftState::Edited] {
        let r = reply_draft_replaced(Some(draft(live)), 5).expect("the live draft is replaced");
        assert_eq!((r.state, r.updated_at_ms), (ReplyDraftState::Discarded, 5), "{live:?}");
    }
    for sealed in [ReplyDraftState::Sent, ReplyDraftState::Discarded] { assert_eq!(reply_draft_replaced(Some(draft(sealed)), 5), None, "{sealed:?} stays as it is"); }
}

// ---------------------------------------------------------------- the state JSON (every read)

fn json(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap_or_else(|e| panic!("not JSON ({e}): {s}")) }
fn keys(v: &serde_json::Value) -> Vec<String> { let mut k: Vec<String> = v.as_object().expect("an object").keys().cloned().collect(); k.sort(); k }
fn k(v: &[&str]) -> Vec<String> { let mut o = ids(v); o.sort(); o }

#[test]
fn every_closed_word_has_one_spelling_that_round_trips() {
    macro_rules! round { ($to:ident, $from:ident, [$($v:expr => $k:literal),+]) => { $( assert_eq!($to($v), $k); assert_eq!($from($k.to_string()), Some($v)); )+ assert_eq!($from("nope".to_string()), None); } }
    round!(reply_use_kind_key, reply_use_kind_from_key, [ReplyUseKind::Auto => "auto", ReplyUseKind::OnDemand => "on_demand"]);
    round!(reply_step_result_key, reply_step_result_from_key, [ReplyStepResult::Passed => "passed", ReplyStepResult::Skipped => "skipped", ReplyStepResult::Stopped => "stopped", ReplyStepResult::NotReached => "not_reached"]);
    round!(reply_made_by_key, reply_made_by_from_key, [ReplyMadeBy::Auto => "auto", ReplyMadeBy::OnDemand => "on_demand"]);
    round!(reply_draft_state_key, reply_draft_state_from_key, [ReplyDraftState::Proposed => "proposed", ReplyDraftState::Edited => "edited", ReplyDraftState::Sent => "sent", ReplyDraftState::Discarded => "discarded"]);
    round!(excerpt_size_key, excerpt_size_from_key, [ExcerptSize::Sentence => "sentence", ExcerptSize::Paragraph => "paragraph", ExcerptSize::Section => "section"]);
    round!(clash_rule_key, clash_rule_from_key, [ClashRule::KeepBothDated => "keep_both_dated", ClashRule::KeepNewest => "keep_newest", ClashRule::KeepOldest => "keep_oldest"]);
    round!(retrieval_scope_key, retrieval_scope_from_key, [RetrievalScope::ThisAccount => "this_account", RetrievalScope::AllAccounts => "all_accounts"]);
    round!(on_demand_option_key, on_demand_option_from_key, [OnDemandOption::InstructionBefore => "instruction_before", OnDemandOption::ChangeAfter => "change_after"]);
    round!(match_mode_key, match_mode_from_key, [MatchMode::Any => "any", MatchMode::All => "all"]);
}

#[test]
fn the_state_json_carries_the_keys_the_contract_names() {
    // conditions: the SAME objects the store's rule.conditions_json holds (store/src/label_meta.rs `encode_conditions`), byte for byte
    let all = vec![from("laura"), RuleCondition::To { words: "a".into() }, RuleCondition::Cc { words: "b".into() }, RuleCondition::Domain { domain: "x.com".into() }, RuleCondition::SubjectContains { words: "s".into() },
        RuleCondition::BodyContains { words: "b".into() }, RuleCondition::ListHeader { words: "l".into() }, RuleCondition::Account { account_id: "acc".into() }, RuleCondition::HasAttachment,
        RuleCondition::Fact { fact: MessageFact::CcMe, wanted: false }];
    assert_eq!(conditions_json(all), "[{\"kind\":\"from\",\"words\":\"laura\"},{\"kind\":\"to\",\"words\":\"a\"},{\"kind\":\"cc\",\"words\":\"b\"},{\"kind\":\"domain\",\"domain\":\"x.com\"},{\"kind\":\"subject\",\"words\":\"s\"},{\"kind\":\"body\",\"words\":\"b\"},{\"kind\":\"list_header\",\"words\":\"l\"},{\"kind\":\"account\",\"account_id\":\"acc\"},{\"kind\":\"has_attachment\"},{\"kind\":\"fact\",\"fact\":\"cc_me\",\"wanted\":false}]");
    // a string with a quote, a backslash and a newline survives
    let n = json(&conditions_json(vec![from("a \"b\" \\ c\nd")]));
    assert_eq!(n[0]["words"], "a \"b\" \\ c\nd");

    let u = json(&reply_use_json(ReplyUse { kind: ReplyUseKind::OnDemand, enabled: true, tone_id: "voice".into(), match_thread: true, prompt_id: REPLY_PROMPT_ID.into() }));
    assert_eq!(keys(&u), k(&["kind", "enabled", "tone_id", "match_thread", "prompt_id"]));
    assert_eq!(u["kind"], "on_demand");

    let mut f = default_reply_filter();
    f.scope = ReplyLabelScope::Labels { label_ids: ids(&["clients"]) };
    f.entry_rule = rs(MatchMode::All, vec![RuleCondition::Fact { fact: MessageFact::MailingList, wanted: false }, from("laura")]);
    let fj = json(&reply_filter_json(f));
    assert_eq!(keys(&fj), k(&["scope", "entry", "ai", "exit"]));
    assert_eq!(keys(&fj["scope"]), k(&["all", "label_ids"]));
    assert_eq!(fj["scope"]["all"], false);
    assert_eq!(keys(&fj["entry"]), k(&["enabled", "rule"]));
    assert_eq!(keys(&fj["entry"]["rule"]), k(&["match", "conditions", "facts", "rows"]));
    assert_eq!(fj["entry"]["rule"]["facts"]["mailing_list"], "no", "the grid cell");
    assert_eq!(fj["entry"]["rule"]["facts"]["unread"], "any");
    assert_eq!(fj["entry"]["rule"]["facts"].as_object().unwrap().len(), 8, "all eight cells");
    assert_eq!(fj["entry"]["rule"]["rows"].as_array().unwrap().len(), 1, "the rows under the grid are the text conditions only");
    assert_eq!(keys(&fj["ai"]), k(&["enabled", "subject", "body", "rest_of_thread", "attachments", "description"]));
    assert_eq!(fj["ai"]["rest_of_thread"], true);
    assert_eq!(keys(&fj["exit"]), k(&["enabled", "rule"]));

    assert_eq!(keys(&json(&on_demand_json(default_on_demand_options()))), k(&["instruction_before", "change_after"]));
    let c = json(&reply_counts_json(ReplyStepCounts { days: 7, received: 128, after_entry: 31, after_ai: 12, after_exit: 9, drafts: 9 }));
    assert_eq!(keys(&c), k(&["days", "received", "after_entry", "after_ai", "after_exit", "drafts"]));
    assert_eq!(c["received"], 128);

    let r = json(&retrieval_json(default_retrieval_settings()));
    assert_eq!(keys(&r), k(&["retrieval_prompt", "questions_max", "excerpts_per_question", "excerpt_size", "attached_fields", "keep_n", "clash_rule", "ai_filter_question", "scope"]));
    assert_eq!(keys(&r["attached_fields"]), k(&["date", "sender", "folder", "subject"]));
    let off = json(&retrieval_json(RetrievalSettings { ai_filter_question: None, ..default_retrieval_settings() }));
    assert!(off["ai_filter_question"].is_null(), "no filter is JSON null, not an empty string");

    let v = json(&reply_voice_json(default_reply_voice(), ReplyVoiceAnalysis::Running { read: 40, of: 100 }, vec![tone("t1", "Formel")]));
    assert_eq!(keys(&v), k(&["voice", "analysis", "tones"]));
    assert_eq!(keys(&v["voice"]), k(&["tone_text", "sample_size", "analysed_at_ms", "same_person_examples"]));
    assert!(v["voice"]["analysed_at_ms"].is_null());
    assert_eq!((v["analysis"]["state"].clone(), v["analysis"]["read"].clone()), (json("\"running\""), json("40")));
    assert_eq!(keys(&v["tones"][0]), k(&["id", "name", "text"]));

    let pv = json(&reply_prompt_versions_json(vec![ReplyPrompt { prompt_id: "replies".into(), version: 1, text: "a".into(), created_at_ms: 1, current: false }, ReplyPrompt { prompt_id: "replies".into(), version: 3, text: "c".into(), created_at_ms: 3, current: true }]));
    assert_eq!((pv[0]["version"].clone(), pv[1]["version"].clone()), (json("3"), json("1")), "newest first");
    assert_eq!(keys(&pv[0]), k(&["prompt_id", "version", "text", "created_at_ms", "current"]));
    let vars = json(&reply_prompt_variables_json(reply_prompt_variables()));
    assert_eq!(keys(&vars[0]), k(&["id", "name_key", "meaning_key", "placeholder"]));
    assert_eq!(vars[1]["placeholder"], "{{thread}}");
    assert_eq!(keys(&json(&fixed_notes_json(vec![FixedNote { id: "n1".into(), title: "t".into(), text: "x".into() }]))[0]), k(&["id", "title", "text"]));

    let dj = json(&reply_draft_json(ReplyDraft { instruction: Some("say yes".into()), ..draft(ReplyDraftState::Proposed) }));
    assert_eq!(keys(&dj), k(&["id", "thread_id", "message_id", "text", "made_by", "excerpt_ids", "instruction", "state", "revision", "created_at_ms", "updated_at_ms"]));
    assert_eq!((dj["state"].clone(), dj["instruction"].clone()), (json("\"proposed\""), json("\"say yes\"")));
    assert!(json(&reply_draft_json(draft(ReplyDraftState::Proposed)))["instruction"].is_null(), "no instruction is null");

    let account = reply_account_sources("acc-1".into(), vec![src("acc-1", "INBOX", true, 9_204), ReplySource { progress: ReplySyncProgress::Syncing { percent: 62 }, ..src("acc-1", "Clients", true, 2_960) }]);
    let pane = json(&reply_sources_json(reply_sources_read(vec![account], vec![], vec![ReplyCalendar { calendar_id: "c1".into(), name: "Travail".into(), account_label: "a".into(), colour: "#007AFF".into(), enabled: true }], RetrievalScope::AllAccounts)));
    assert_eq!(keys(&pane), k(&["accounts", "local_folders", "calendars", "scope", "enabled_total"]));
    assert_eq!(keys(&pane["accounts"][0]), k(&["account_id", "enabled_folders", "total_folders", "enabled_items", "folders"]));
    assert_eq!(keys(&pane["accounts"][0]["folders"][0]), k(&["source", "key", "enabled", "owner_enabled", "item_count", "progress"]));
    assert_eq!((pane["accounts"][0]["folders"][1]["progress"]["state"].clone(), pane["accounts"][0]["folders"][1]["progress"]["percent"].clone()), (json("\"syncing\""), json("62")));
    assert_eq!(keys(&pane["calendars"][0]), k(&["calendar_id", "name", "account_label", "colour", "enabled"]));
    assert_eq!(pane["enabled_total"], 3);

    let step = |r| ReplyStepReport { result: r, matched: vec![from("laura")], model_note: Some("asks a question".into()) };
    let t = json(&reply_try_json(ReplyTryReport { run_id: "run-1".into(), message_id: "m1".into(), kind: ReplyUseKind::Auto,
        retrieval: ReplyRetrievalReport { questions: ids(&["What did we quote?"]), found: 5, kept: vec![ReplyExcerpt { chunk_id: "c1".into(), source: ReplySourceRef::LocalFolder { path: "~/x".into() }, date_ms: Some(5), sender: None, folder: Some("Clients".into()), subject: None, text: "Quote 4 200".into() }], dropped_by_ai_filter: 1 },
        entry: step(ReplyStepResult::Passed), ai: step(ReplyStepResult::Passed), exit: step(ReplyStepResult::Stopped), draft_text: None }));
    assert_eq!(keys(&t), k(&["run_id", "message_id", "kind", "retrieval", "entry", "ai", "exit", "draft_text"]));
    assert_eq!(keys(&t["retrieval"]), k(&["questions", "found", "kept", "dropped_by_ai_filter"]));
    assert_eq!(keys(&t["retrieval"]["kept"][0]), k(&["chunk_id", "source", "date_ms", "sender", "folder", "subject", "text"]));
    assert_eq!(keys(&t["ai"]), k(&["result", "matched", "model_note"]));
    assert!(t["draft_text"].is_null());
}

// ---------------------------------------------------------------- the commands (every new `Command` has a guard that refuses a wrong value in words with its code)

#[test]
fn every_new_command_has_its_guard_and_its_code() {
    let d = default_retrieval_settings();
    let blank_ai = ReplyFilter { ai_description: " ".into(), ..default_reply_filter() };
    // (the command, the first refusal its guard gives, its code). A command with no refusal of its own is listed under `no_refusal`.
    let guarded: Vec<(Command, String)> = vec![
        (Command::SetReplyFilterScope { scope: ReplyLabelScope::Labels { label_ids: vec![] } }, code_of(check_reply_scope(ReplyLabelScope::Labels { label_ids: vec![] })).0),
        (Command::SetReplyFilterCard { card: FlowCard::Ai, enabled: true }, code_of(check_set_reply_card(blank_ai, FlowCard::Ai, true)).0),
        (Command::SetReplyFilterEntryRule { rule: rs(MatchMode::All, vec![from(" ")]) }, code_of(validate_reply_rule(rs(MatchMode::All, vec![from(" ")]))).0),
        (Command::SetReplyFilterExitRule { rule: rs(MatchMode::All, vec![from(" ")]) }, code_of(validate_reply_rule(rs(MatchMode::All, vec![from(" ")]))).0),
        (Command::SetReplyFilterAiReads { reads: AiReads { subject: false, body: false, rest_of_thread: false, attachments: false } },
            code_of(check_reply_ai_reads(AiReads { subject: false, body: false, rest_of_thread: false, attachments: false })).0),
        (Command::SetReplyFilterAiDescription { description: " ".into() }, code_of(check_reply_ai_description(true, " ".into())).0),
        (Command::SetRetrievalSetting { change: RetrievalChange::KeepN { n: 0 } }, code_of(apply_retrieval_change(d, RetrievalChange::KeepN { n: 0 })).0),
        (Command::AddReplyLocalFolder { path: "rel/path".into() }, code_of(check_local_folder_path("rel/path".into())).0),
        (Command::RunVoiceAnalysis { sample_size: 0 }, code_of(check_voice_sample_size(0)).0),
        (Command::SetVoiceSamePersonExamples { count: 99 }, code_of(check_same_person_examples(99)).0),
        (Command::SaveReplyTone { tone: tone("", " ") }, code_of(check_reply_tone(tone("", " "), vec![])).0),
        (Command::SaveReplyPromptVersion { prompt_id: "replies".into(), text: "".into() }, code_of(check_reply_prompt_text("".into())).0),
        (Command::SaveFixedNote { note: FixedNote { id: "".into(), title: "".into(), text: "".into() } }, code_of(check_fixed_note(FixedNote { id: "".into(), title: "".into(), text: "".into() })).0),
        (Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: Some("x".into()) }, code_of(check_on_demand_instruction(OnDemandOptions { instruction_before: false, change_after: true }, Some("x".into()))).0),
        (Command::RequestReplyChange { draft_id: "d1".into(), text: "x".into() }, code_of(check_change_request(OnDemandOptions { instruction_before: true, change_after: false }, draft(ReplyDraftState::Proposed), "x".into())).0),
        (Command::AcceptReplyDraft { draft_id: "d1".into() }, code_of(reply_draft_after_accept(draft(ReplyDraftState::Sent), 1)).0),
        (Command::DiscardReplyDraft { draft_id: "d1".into() }, code_of(reply_draft_after_discard(draft(ReplyDraftState::Discarded), 1)).0),
    ];
    let expected: Vec<&str> = vec![REPLY_SCOPE_EMPTY_CODE, REPLY_AI_NEEDS_DESCRIPTION_CODE, REPLY_BAD_RULE_CODE, REPLY_BAD_RULE_CODE, REPLY_READS_NOTHING_CODE, REPLY_AI_NEEDS_DESCRIPTION_CODE,
        REPLY_BAD_SETTING_CODE, REPLY_BAD_FOLDER_CODE, REPLY_BAD_SAMPLE_CODE, REPLY_BAD_SAMPLE_CODE, REPLY_BAD_TONE_CODE, REPLY_BAD_PROMPT_CODE, REPLY_BAD_NOTE_CODE, REPLY_INSTRUCTION_OFF_CODE,
        REPLY_CHANGE_OFF_CODE, REPLY_DRAFT_CLOSED_CODE, REPLY_DRAFT_CLOSED_CODE];
    assert_eq!(guarded.iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>(), expected);
    // the commands that cannot be refused by their value alone (they name an object the core looks up, or switch something): each is constructible and cloneable
    let no_refusal = vec![
        Command::SetReplyUseEnabled { kind: ReplyUseKind::Auto, enabled: true }, Command::SetReplyUseTone { kind: ReplyUseKind::Auto, tone_id: "voice".into() },
        Command::SetReplyUseMatchThread { kind: ReplyUseKind::OnDemand, on: true }, Command::SetReplyUsePrompt { kind: ReplyUseKind::Auto, prompt_id: REPLY_PROMPT_ID.into() },
        Command::SetOnDemandOption { option: OnDemandOption::ChangeAfter, enabled: false },
        Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "c1".into() }, enabled: true }, Command::RemoveReplyLocalFolder { path: "~/x".into() },
        Command::SetVoiceText { text: "".into() }, Command::DeleteReplyTone { tone_id: "t1".into() }, Command::RestoreReplyPromptVersion { prompt_id: REPLY_PROMPT_ID.into(), version: 1 },
        Command::DeleteFixedNote { note_id: "n1".into() }, Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto },
    ];
    for c in guarded.iter().map(|(c, _)| c).chain(no_refusal.iter()) { assert_eq!(&c.clone(), c); }
    assert_eq!(guarded.len() + no_refusal.len(), 29, "the 29 commands of delta 20");
}

#[test]
fn the_events_of_delta_20_carry_what_the_screens_read() {
    let d = draft(ReplyDraftState::Proposed);
    let evs = vec![CoreEvent::ReplyDraftReady { thread_id: "t1".into(), draft_id: "d1".into() }, CoreEvent::ReplyDraftChanged { thread_id: "t1".into(), draft: d.clone() },
        CoreEvent::ReplySourceProgress { source: ReplySourceRef::MailFolder { account_id: "a".into(), folder: "Clients".into() }, item_count: 2_960, progress: ReplySyncProgress::Syncing { percent: 62 } }];
    for e in &evs { assert_eq!(&e.clone(), e); }
    assert!(matches!(&evs[1], CoreEvent::ReplyDraftChanged { draft, .. } if draft == &d));
}

// ---------------------------------------------------------------- the defaults and the migration agree

#[test]
fn the_defaults_equal_the_migrations_seed() {
    let sql = std::fs::read_to_string(format!("{}/../store/migrations/sql/0013_ai_replies.sql", env!("CARGO_MANIFEST_DIR"))).expect("migration 0013");
    let lit = |s: String| s.replace('\'', "''");
    assert!(sql.contains(&lit(default_reply_prompt_text())), "the migration seeds the contract's default prompt, byte for byte");
    assert!(sql.contains(&lit(default_reply_ai_description())), "the migration seeds the contract's default AI question");
    let r = default_retrieval_settings();
    assert!(sql.contains(&lit(r.retrieval_prompt)) && sql.contains(&lit(r.ai_filter_question.unwrap())), "the migration seeds the contract's retrieval prompt and filter question");
    let sample = default_reply_voice();
    assert!(sql.contains(&format!("{}, NULL, {}", sample.sample_size, sample.same_person_examples)), "the voice row's defaults");
    assert_eq!((REPLY_COUNTS_DAYS, default_reply_filter().ai_reads), (7, default_reply_ai_reads()));
}
