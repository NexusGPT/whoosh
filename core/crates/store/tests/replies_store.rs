//! Delta 20 (AI replies settings) in the store, against a real migrated database: each family of `store/src/replies.rs` set, then read back through
//! its typed read; each refusal in the contract's code; a refused write leaves nothing behind.
//!
//! Mutants (single edits; each was run and seen red, the verbatim results are in _captures/replies-settings/mutants-run.log; the runner is
//! _captures/replies-settings/run-mutants.py):
//! - S1 replies.rs `reply_set_use_enabled`: the WHERE names the wrong kind
//! - S2 replies.rs `reply_set_filter_scope`: the old ticks are not cleared
//! - S3 replies.rs `reply_set_filter_rule`: the entry arm writes the exit column
//! - S4 replies.rs `reply_apply_retrieval_change`: the new `keep_n` is not written
//! - S5 replies.rs `reply_set_source_enabled`: a source that does not exist is no longer NotFound
//! - S6 replies.rs `push_prompt_version`: the old version stays current
//! - S7 replies.rs `reply_delete_tone`: the NotFound is dropped
//! - S8 replies.rs `reply_write`: a refusal commits what the closure already wrote (no rollback)
//! - S9 replies.rs `reply_record_pass`: the second write for a message adds a row instead of replacing it
//! - S10 to S13 (delta 21): a dropped calendar keeps its source row; a new calendar starts on; the window's events are not replaced; the indexed count ignores chunks still waiting for a vector
mod common;

use common::{msg, new_msg, TempDb};
use contracts::*;
use store::Store;

const A: &str = "owner@example.test";

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: A.into(), smtp_username: A.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn open() -> (Store, String, TempDb) {
    // the shared TempDb names a folder by the clock: two parallel tests can read the same nanosecond, so each test also takes its own number
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let db = TempDb::new(&format!("replies-{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let store = Store::open(&db.0).unwrap();
    let account = store.add_account(A, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (store, account, db)
}
fn code(e: CoreError) -> String {
    match e { CoreError::Invalid { field, .. } => field, CoreError::NotFound { kind, .. } => format!("not_found:{kind}"), other => panic!("not a refusal: {other}") }
}
fn rule(words: &str) -> RuleSet { RuleSet { conditions: vec![RuleCondition::From { words: words.into() }], match_mode: MatchMode::Any } }
fn count(s: &Store, sql: &'static str) -> i64 { s.write(move |c| Ok(c.tx.query_row(sql, [], |r| r.get::<_, i64>(0))?)).unwrap() }

#[test]
fn a_fresh_store_holds_the_contracts_defaults() {
    let (s, _, _d) = open();
    assert_eq!(s.reply_filter().unwrap(), default_reply_filter());
    assert_eq!(s.reply_on_demand().unwrap(), default_on_demand_options());
    assert_eq!(s.reply_retrieval().unwrap(), default_retrieval_settings());
    assert_eq!(s.reply_voice().unwrap(), default_reply_voice());
    let auto = s.reply_use(ReplyUseKind::Auto).unwrap();
    assert_eq!((auto.enabled, auto.tone_id.as_str(), auto.match_thread, auto.prompt_id.as_str()), (false, "voice", true, "replies"), "the auto use starts off");
    assert!(s.reply_use(ReplyUseKind::OnDemand).unwrap().enabled, "on demand starts on");
    let v = s.reply_prompt_versions("replies").unwrap();
    assert_eq!((v.len(), v[0].version, v[0].current, v[0].text.as_str()), (1, 1, true, default_reply_prompt_text().as_str()));
}

#[test]
fn the_uses_change_one_at_a_time_and_a_tone_or_prompt_that_does_not_exist_is_refused() {
    let (s, _, _d) = open();
    s.reply_set_use_enabled(ReplyUseKind::Auto, true, 5).unwrap();
    assert!(s.reply_use(ReplyUseKind::Auto).unwrap().enabled);
    assert!(s.reply_use(ReplyUseKind::OnDemand).unwrap().enabled, "the other use did not move");
    s.reply_set_use_enabled(ReplyUseKind::OnDemand, false, 6).unwrap();
    assert!(!s.reply_use(ReplyUseKind::OnDemand).unwrap().enabled && s.reply_use(ReplyUseKind::Auto).unwrap().enabled);
    s.reply_set_use_match_thread(ReplyUseKind::Auto, false, 7).unwrap();
    assert!(!s.reply_use(ReplyUseKind::Auto).unwrap().match_thread);
    assert_eq!(code(s.reply_set_use_tone(ReplyUseKind::Auto, "ghost".into(), 8).unwrap_err()), "not_found:tone");
    let formal = s.reply_save_tone(ReplyTone { id: String::new(), name: "Formel".into(), text: "Vouvoyer.".into() }).unwrap();
    s.reply_set_use_tone(ReplyUseKind::Auto, formal.clone(), 9).unwrap();
    assert_eq!(s.reply_use(ReplyUseKind::Auto).unwrap().tone_id, formal);
    assert_eq!(code(s.reply_set_use_prompt(ReplyUseKind::Auto, "ghost".into(), 10).unwrap_err()), "not_found:prompt");
    s.reply_set_use_prompt(ReplyUseKind::Auto, "replies".into(), 11).unwrap();
}
// mutant S1: reply_set_use_enabled's `WHERE kind = ?1` names the other kind (the other use moves).

#[test]
fn the_filter_scope_is_all_or_exactly_the_ticked_labels_and_a_ghost_label_leaves_nothing_behind() {
    let (s, _, _d) = open();
    s.write(|c| Ok(c.tx.execute("INSERT INTO label (id, name, colour, path, depth, updated_at_ms) VALUES ('l1', 'Clients', '#fff', 'Clients', 1, 0), ('l2', 'Team', '#fff', 'Team', 1, 0)", [])?)).unwrap();
    s.reply_set_filter_scope(ReplyLabelScope::Labels { label_ids: vec!["l1".into(), "l2".into()] }, 5).unwrap();
    assert_eq!(s.reply_filter().unwrap().scope, ReplyLabelScope::Labels { label_ids: vec!["l1".into(), "l2".into()] });
    s.reply_set_filter_scope(ReplyLabelScope::Labels { label_ids: vec!["l2".into()] }, 6).unwrap();
    assert_eq!(s.reply_filter().unwrap().scope, ReplyLabelScope::Labels { label_ids: vec!["l2".into()] }, "the old ticks are replaced, not added to");
    assert_eq!(code(s.reply_set_filter_scope(ReplyLabelScope::Labels { label_ids: vec![] }, 7).unwrap_err()), REPLY_SCOPE_EMPTY_CODE);
    // a list with a real label and a ghost is refused WHOLE: the real one is not stored by the half that ran first
    assert_eq!(code(s.reply_set_filter_scope(ReplyLabelScope::Labels { label_ids: vec!["l1".into(), "ghost".into()] }, 8).unwrap_err()), "not_found:label");
    assert_eq!(s.reply_filter().unwrap().scope, ReplyLabelScope::Labels { label_ids: vec!["l2".into()] }, "the refused write changed nothing (rolled back)");
    s.reply_set_filter_scope(ReplyLabelScope::All, 9).unwrap();
    assert_eq!(s.reply_filter().unwrap().scope, ReplyLabelScope::All);
    assert_eq!(count(&s, "SELECT count(*) FROM reply_filter_label"), 0, "All holds no tick rows");
}
// mutant S2: reply_set_filter_scope no longer runs `DELETE FROM reply_filter_label` (l1 stays ticked beside l2).
// mutant S8: reply_write commits what a refused closure already wrote (the ghost half-write survives).

#[test]
fn the_cards_the_rules_the_ai_reads_and_the_question_are_stored_and_refused_in_the_contracts_words() {
    let (s, _, _d) = open();
    s.reply_set_filter_card(FlowCard::Entry, false, 1).unwrap();
    s.reply_set_filter_card(FlowCard::Exit, false, 2).unwrap();
    let f = s.reply_filter().unwrap();
    assert_eq!((f.entry_enabled, f.ai_enabled, f.exit_enabled), (false, true, false));
    s.reply_set_filter_rule(FlowCard::Entry, rule("boss@example.test"), 3).unwrap();
    s.reply_set_filter_rule(FlowCard::Exit, RuleSet { conditions: vec![RuleCondition::HasAttachment], match_mode: MatchMode::All }, 4).unwrap();
    let f = s.reply_filter().unwrap();
    assert_eq!(f.entry_rule, rule("boss@example.test"), "the entry rule lands in the entry columns");
    assert_eq!(f.exit_rule, RuleSet { conditions: vec![RuleCondition::HasAttachment], match_mode: MatchMode::All }, "the exit rule lands in the exit columns");
    s.reply_set_filter_rule(FlowCard::Entry, RuleSet { conditions: vec![], match_mode: MatchMode::All }, 5).unwrap();
    assert!(s.reply_filter().unwrap().entry_rule.conditions.is_empty(), "an empty rule is a value");
    assert_eq!(code(s.reply_set_filter_rule(FlowCard::Entry, rule("  "), 6).unwrap_err()), REPLY_BAD_RULE_CODE);
    let reads = AiReads { subject: true, body: false, rest_of_thread: false, attachments: true };
    s.reply_set_filter_ai_reads(reads, 7).unwrap();
    assert_eq!(s.reply_filter().unwrap().ai_reads, reads);
    let none = AiReads { subject: false, body: false, rest_of_thread: false, attachments: false };
    assert_eq!(code(s.reply_set_filter_ai_reads(none, 8).unwrap_err()), REPLY_READS_NOTHING_CODE);
    assert_eq!(s.reply_filter().unwrap().ai_reads, reads, "a refused tick set changes nothing");
    s.reply_set_filter_ai_description("  Needs an answer  ".into(), 9).unwrap();
    assert_eq!(s.reply_filter().unwrap().ai_description, "Needs an answer", "the question is stored trimmed");
    assert_eq!(code(s.reply_set_filter_ai_description("   ".into(), 10).unwrap_err()), REPLY_AI_NEEDS_DESCRIPTION_CODE, "blank while the AI card is on");
    s.reply_set_filter_card(FlowCard::Ai, false, 11).unwrap();
    s.reply_set_filter_ai_description("".into(), 12).unwrap();
    assert_eq!(code(s.reply_set_filter_card(FlowCard::Ai, true, 13).unwrap_err()), REPLY_AI_NEEDS_DESCRIPTION_CODE, "the AI card on needs its question");
}
// mutant S3: reply_set_filter_rule's Entry arm writes the exit columns.

#[test]
fn the_two_on_demand_switches_are_independent() {
    let (s, _, _d) = open();
    s.reply_set_on_demand(OnDemandOption::InstructionBefore, false, 1).unwrap();
    assert_eq!(s.reply_on_demand().unwrap(), OnDemandOptions { instruction_before: false, change_after: true });
    s.reply_set_on_demand(OnDemandOption::ChangeAfter, false, 2).unwrap();
    assert_eq!(s.reply_on_demand().unwrap(), OnDemandOptions { instruction_before: false, change_after: false });
}

#[test]
fn each_retrieval_setting_changes_alone_and_a_bad_one_is_refused() {
    let (s, _, _d) = open();
    let d = default_retrieval_settings();
    let after = s.reply_apply_retrieval_change(RetrievalChange::KeepN { n: 9 }, 1).unwrap();
    assert_eq!(after.keep_n, 9);
    assert_eq!(s.reply_retrieval().unwrap(), RetrievalSettings { keep_n: 9, ..d.clone() }, "only keep_n moved");
    s.reply_apply_retrieval_change(RetrievalChange::ExcerptSize { size: ExcerptSize::Section }, 2).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::Clash { rule: ClashRule::KeepNewest }, 3).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::Scope { scope: RetrievalScope::AllAccounts }, 4).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::AttachedFields { fields: ExcerptFields { date: false, sender: true, folder: false, subject: true } }, 5).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::AiFilterQuestion { question: None }, 6).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::QuestionsMax { n: 2 }, 7).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::ExcerptsPerQuestion { n: 4 }, 8).unwrap();
    s.reply_apply_retrieval_change(RetrievalChange::Prompt { text: "Ask about {{message}}".into() }, 9).unwrap();
    let r = s.reply_retrieval().unwrap();
    assert_eq!((r.excerpt_size, r.clash_rule, r.scope, r.ai_filter_question.clone(), r.questions_max, r.excerpts_per_question, r.keep_n, r.retrieval_prompt.as_str()),
        (ExcerptSize::Section, ClashRule::KeepNewest, RetrievalScope::AllAccounts, None, 2, 4, 9, "Ask about {{message}}"));
    assert_eq!(r.attached_fields, ExcerptFields { date: false, sender: true, folder: false, subject: true });
    assert_eq!(code(s.reply_apply_retrieval_change(RetrievalChange::QuestionsMax { n: 0 }, 10).unwrap_err()), REPLY_BAD_SETTING_CODE);
    assert_eq!(code(s.reply_apply_retrieval_change(RetrievalChange::Prompt { text: "{{ghost}}".into() }, 11).unwrap_err()), REPLY_BAD_SETTING_CODE);
    assert_eq!(code(s.reply_apply_retrieval_change(RetrievalChange::AiFilterQuestion { question: Some("  ".into()) }, 12).unwrap_err()), REPLY_BAD_SETTING_CODE);
    assert_eq!(s.reply_retrieval().unwrap(), r, "a refused change changes nothing");
}
// mutant S4: reply_apply_retrieval_change binds the OLD keep_n (`s.keep_n` -> the stored one) so the new value is not written.

#[test]
fn sources_a_switch_a_folder_of_this_mac_and_the_syncs_report() {
    let (s, account, _d) = open();
    let inbox = ReplySourceRef::MailFolder { account_id: account.clone(), folder: "INBOX".into() };
    assert_eq!(code(s.reply_set_source_enabled(inbox.clone(), true, 1).unwrap_err()), "not_found:source", "a source nothing registered is NotFound");
    s.reply_source_ensure(inbox.clone(), 2).unwrap();
    let row = |s: &Store, r: &ReplySourceRef| s.reply_sources().unwrap().into_iter().find(|x| &x.source == r).expect("the source is listed");
    assert!(!row(&s, &inbox).enabled, "a registered source starts off");
    s.reply_set_source_enabled(inbox.clone(), true, 3).unwrap();
    assert!(row(&s, &inbox).enabled);
    assert!(s.reply_source_progress(inbox.clone(), 120, ReplySyncProgress::Syncing { percent: 40 }, 4).unwrap());
    let r = row(&s, &inbox);
    assert_eq!((r.item_count, r.progress.clone()), (120, ReplySyncProgress::Syncing { percent: 40 }));
    s.reply_source_progress(inbox.clone(), 300, ReplySyncProgress::Failed { reason_words: "offline".into() }, 5).unwrap();
    assert_eq!(row(&s, &inbox).progress, ReplySyncProgress::Failed { reason_words: "offline".into() });
    s.reply_source_progress(inbox.clone(), 300, ReplySyncProgress::UpToDate, 6).unwrap();
    assert_eq!(row(&s, &inbox).progress, ReplySyncProgress::UpToDate);
    assert!(!s.reply_source_progress(ReplySourceRef::Calendar { calendar_id: "ghost".into() }, 0, ReplySyncProgress::UpToDate, 7).unwrap(), "no such source");
    let cal = ReplySourceRef::Calendar { calendar_id: "cal-1".into() };
    assert_eq!(code(s.reply_set_source_enabled(cal.clone(), true, 8).unwrap_err()), "not_found:calendar");
    s.reply_source_ensure(cal.clone(), 9).unwrap();
    s.reply_set_source_enabled(cal.clone(), true, 10).unwrap();
    assert!(row(&s, &cal).enabled);
    // a folder of this Mac: the same folder twice is one source, switched on; a path that cannot be one is refused; removing an unknown one is NotFound
    let key = s.reply_add_local_folder("/Users/owner/Docs/".into(), 11).unwrap();
    assert_eq!(key, reply_source_key(ReplySourceRef::LocalFolder { path: "/Users/owner/Docs".into() }), "the trailing slash is dropped");
    let local = ReplySourceRef::LocalFolder { path: "/Users/owner/Docs".into() };
    s.reply_set_source_enabled(local.clone(), false, 12).unwrap();
    assert_eq!(s.reply_add_local_folder("/Users/owner/Docs".into(), 13).unwrap(), key);
    assert!(row(&s, &local).enabled, "added again = switched on");
    assert_eq!(s.reply_sources().unwrap().iter().filter(|x| matches!(x.source, ReplySourceRef::LocalFolder { .. })).count(), 1);
    assert_eq!(code(s.reply_add_local_folder("relative/path".into(), 14).unwrap_err()), REPLY_BAD_FOLDER_CODE);
    s.reply_remove_local_folder("/Users/owner/Docs/".into()).unwrap();
    assert_eq!(code(s.reply_remove_local_folder("/Users/owner/Docs".into()).unwrap_err()), "not_found:folder");
}
// mutant S5: reply_set_source_enabled drops its `n == 0` NotFound (a ghost source is accepted).

#[test]
fn the_voice_the_tones_and_the_examples() {
    let (s, _, _d) = open();
    s.reply_set_voice_text("Short.".into(), 1).unwrap();
    s.reply_set_same_person_examples(0, 2).unwrap();
    let v = s.reply_voice().unwrap();
    assert_eq!((v.tone_text.as_str(), v.same_person_examples), ("Short.", 0));
    assert_eq!(code(s.reply_set_same_person_examples(REPLY_EXAMPLES_CEILING + 1, 3).unwrap_err()), REPLY_BAD_SAMPLE_CODE);
    s.reply_record_voice_analysis(80, 900, Some("Warm and brief.".into())).unwrap();
    let v = s.reply_voice().unwrap();
    assert_eq!((v.sample_size, v.analysed_at_ms, v.tone_text.as_str()), (80, Some(900), "Warm and brief."));
    s.reply_record_voice_analysis(80, 950, None).unwrap();
    assert_eq!(s.reply_voice().unwrap().tone_text, "Warm and brief.", "an analysis that wrote no text keeps the text");
    assert_eq!(code(s.reply_record_voice_analysis(0, 960, None).unwrap_err()), REPLY_BAD_SAMPLE_CODE);

    let formal = s.reply_save_tone(ReplyTone { id: String::new(), name: " Formel ".into(), text: "Vouvoyer.".into() }).unwrap();
    let brief = s.reply_save_tone(ReplyTone { id: String::new(), name: "Bref".into(), text: "Trois lignes.".into() }).unwrap();
    assert_ne!(formal, brief);
    assert_eq!(s.reply_tones().unwrap().iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Formel", "Bref"], "in the order saved, names trimmed");
    s.reply_save_tone(ReplyTone { id: formal.clone(), name: "Formel".into(), text: "Vouvoyer toujours.".into() }).unwrap();
    assert_eq!(s.reply_tones().unwrap()[0].text, "Vouvoyer toujours.", "the same id replaces");
    for (id, name, text) in [(String::new(), "formel", "x"), (String::new(), " ", "x"), (String::new(), "Court", " "), ("voice".to_string(), "Voice", "x")] {
        assert_eq!(code(s.reply_save_tone(ReplyTone { id, name: name.into(), text: text.into() }).unwrap_err()), REPLY_BAD_TONE_CODE, "{name:?}");
    }
    s.reply_set_use_tone(ReplyUseKind::Auto, formal.clone(), 5).unwrap();
    s.reply_delete_tone(formal.clone()).unwrap();
    assert_eq!(s.reply_use(ReplyUseKind::Auto).unwrap().tone_id, "voice", "a removed tone sends the use back to the voice");
    assert_eq!(code(s.reply_delete_tone(formal).unwrap_err()), "not_found:tone");
}
// mutant S7: reply_delete_tone drops its `== 0` NotFound.

#[test]
fn prompt_versions_only_grow_one_is_current_and_restore_makes_a_new_version() {
    let (s, _, _d) = open();
    let v2 = s.reply_save_prompt_version("replies".into(), "Answer {{message}} briefly.".into(), 100).unwrap();
    assert_eq!(v2, 2);
    let all = s.reply_prompt_versions("replies").unwrap();
    assert_eq!(all.iter().map(|p| (p.version, p.current)).collect::<Vec<_>>(), [(1, false), (2, true)], "exactly the newest is current");
    let v3 = s.reply_restore_prompt_version("replies".into(), 1, 200).unwrap();
    assert_eq!(v3, 3, "a restore is a new version");
    let all = s.reply_prompt_versions("replies").unwrap();
    assert_eq!(all.iter().filter(|p| p.current).count(), 1);
    assert_eq!(all[2].text, all[0].text, "version 3 carries version 1's text");
    assert_eq!(all.len(), 3, "nothing was removed");
    assert_eq!(code(s.reply_save_prompt_version("replies".into(), "no placeholder at all".into(), 300).unwrap_err()), REPLY_BAD_PROMPT_CODE);
    assert_eq!(code(s.reply_save_prompt_version("replies".into(), "{{message}} {{ghost}}".into(), 300).unwrap_err()), REPLY_BAD_PROMPT_CODE);
    assert_eq!(code(s.reply_save_prompt_version("ghost".into(), "{{message}}".into(), 300).unwrap_err()), "not_found:prompt");
    assert_eq!(code(s.reply_restore_prompt_version("replies".into(), 99, 300).unwrap_err()), "not_found:prompt version");
    assert_eq!(s.reply_prompt_versions("replies").unwrap().len(), 3, "refusals add no version");
}
// mutant S6: push_prompt_version no longer demotes the old current (the unique index then refuses the insert: the save fails).

#[test]
fn fixed_notes_are_created_replaced_listed_in_order_and_deleted() {
    let (s, _, _d) = open();
    let a = s.reply_save_fixed_note(FixedNote { id: String::new(), title: "Prices".into(), text: "Never quote.".into() }).unwrap();
    let b = s.reply_save_fixed_note(FixedNote { id: String::new(), title: "Sign-off".into(), text: "Best, Assem".into() }).unwrap();
    s.reply_save_fixed_note(FixedNote { id: a.clone(), title: "Prices".into(), text: "Never quote a price.".into() }).unwrap();
    let notes = s.reply_fixed_notes().unwrap();
    assert_eq!(notes.iter().map(|n| (n.id.as_str(), n.text.as_str())).collect::<Vec<_>>(), [(a.as_str(), "Never quote a price."), (b.as_str(), "Best, Assem")]);
    assert_eq!(code(s.reply_save_fixed_note(FixedNote { id: String::new(), title: " ".into(), text: "x".into() }).unwrap_err()), REPLY_BAD_NOTE_CODE);
    assert_eq!(code(s.reply_save_fixed_note(FixedNote { id: String::new(), title: "t".into(), text: "".into() }).unwrap_err()), REPLY_BAD_NOTE_CODE);
    s.reply_delete_fixed_note(a.clone()).unwrap();
    assert_eq!(s.reply_fixed_notes().unwrap().len(), 1);
    assert_eq!(code(s.reply_delete_fixed_note(a).unwrap_err()), "not_found:note");
}

#[test]
fn a_pass_is_one_row_per_message_and_only_legal_shapes_are_stored() {
    let (s, account, _d) = open();
    s.insert_messages(vec![new_msg(msg("m1", "t1", &account, "x@ext.test", 1_000), None), new_msg(msg("m2", "t2", &account, "y@ext.test", 2_000), None)]).unwrap();
    let pass = |id: &str, at, entry, ai, exit| ReplyPass { message_id: id.into(), received_at_ms: at, entry, ai, exit, draft_id: None };
    use ReplyStepResult::*;
    s.reply_record_pass(pass("m1", 1_000, Passed, Stopped, NotReached)).unwrap();
    s.reply_record_pass(pass("m1", 1_000, Passed, Passed, Passed)).unwrap();
    s.reply_record_pass(pass("m2", 2_000, Stopped, NotReached, NotReached)).unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM reply_pass"), 2, "looking at m1 again replaced its row");
    let rows = s.reply_passes_since(0).unwrap();
    assert_eq!(rows.iter().find(|p| p.message_id == "m1").unwrap().exit, Passed);
    assert_eq!(s.reply_passes_since(1_500).unwrap().len(), 1, "the window opens after since_ms");
    assert_eq!(code(s.reply_record_pass(pass("m2", 2_000, Stopped, Passed, NotReached)).unwrap_err()), "bad_reply_pass", "a stop leaves the later steps unreached");
}
// mutant S9: reply_record_pass's `ON CONFLICT(message_id) DO UPDATE` becomes `ON CONFLICT(message_id) DO NOTHING` (the second write is lost).

#[test]
fn the_live_draft_of_a_thread_is_the_proposed_or_edited_one() {
    let (s, account, _d) = open();
    s.insert_messages(vec![new_msg(msg("m1", "t1", &account, "x@ext.test", 1_000), None)]).unwrap();
    assert_eq!(s.reply_draft_live("t1").unwrap(), None);
    s.write(|c| Ok(c.tx.execute("INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, excerpt_ids_json, instruction, state, revision, created_at_ms, updated_at_ms) \
        VALUES ('d1', 't1', 'm1', 'Hello', 'on_demand', '[\"c1\",\"c2\"]', 'be brief', 'proposed', 0, 5, 6)", [])?)).unwrap();
    let d = s.reply_draft_live("t1").unwrap().expect("a live draft");
    assert_eq!((d.id.as_str(), d.text.as_str(), d.made_by, d.excerpt_ids.clone(), d.instruction.clone(), d.state), ("d1", "Hello", ReplyMadeBy::OnDemand, vec!["c1".to_string(), "c2".to_string()], Some("be brief".to_string()), ReplyDraftState::Proposed));
    s.write(|c| Ok(c.tx.execute("UPDATE reply_draft SET state = 'discarded' WHERE id = 'd1'", [])?)).unwrap();
    assert_eq!(s.reply_draft_live("t1").unwrap(), None, "a discarded draft is not live");
    assert_eq!(s.reply_draft("d1").unwrap().map(|d| d.state), Some(ReplyDraftState::Discarded), "but it is still readable by id");
}

#[test]
fn the_newest_message_is_what_the_preview_reads() {
    let (s, account, _d) = open();
    assert_eq!(s.reply_newest_message().unwrap(), None);
    s.insert_messages(vec![new_msg(msg("m1", "t1", &account, "old@ext.test", 1_000), None), new_msg(msg("m2", "t2", &account, "new@ext.test", 9_000), None)]).unwrap();
    let (from, subject, _) = s.reply_newest_message().unwrap().unwrap();
    assert_eq!((from.as_str(), subject.as_str()), ("new@ext.test", "Subject m2"));
}

// ---------------------------------------------------------------- delta 21: the shell's calendars and the crawl's progress

fn info(id: &str, name: &str) -> ReplyCalendarInfo { ReplyCalendarInfo { calendar_id: id.into(), name: name.into(), account_label: "iCloud".into(), colour: "#3366FF".into() } }
fn event(cal: &str, id: &str, title: &str, start: i64, end: i64) -> CalendarEvent {
    CalendarEvent { event_id: id.into(), calendar_id: cal.into(), title: title.into(), start_ms: start, end_ms: end, attendees: vec!["a@x.test".into(), "b \"q\"@x.test".into()], location: Some("Room 1".into()) }
}

#[test]
fn calendars_the_shell_pushes_start_off_keep_their_switch_replace_their_window_and_go_with_their_events() {
    let (s, _, _d) = open();
    // a calendar registered before the shell listed it (no reply_calendar row) is dropped with its source row when the shell does not list it
    s.reply_source_ensure(ReplySourceRef::Calendar { calendar_id: "legacy".into() }, 0).unwrap();
    // a new calendar starts OFF: its events are not kept
    let r = s.reply_sync_calendars(vec![info("a", "Work"), info("b", "Home")], 0, 10_000, vec![event("a", "e1", "Standup", 100, 200)], 1).unwrap();
    assert_eq!((r.events.len(), r.ignored_events), (0, 1));
    assert_eq!(s.reply_calendars().unwrap().iter().map(|c| (c.calendar_id.as_str(), c.name.as_str(), c.account_label.as_str(), c.colour.as_str(), c.enabled)).collect::<Vec<_>>(),
        [("a", "Work", "iCloud", "#3366FF", false), ("b", "Home", "iCloud", "#3366FF", false)], "the name, label and colour have a home now, and the switch is off");
    assert!(s.reply_calendar_events().unwrap().is_empty());
    assert_eq!(code(s.reply_set_source_enabled(ReplySourceRef::Calendar { calendar_id: "legacy".into() }, true, 1).unwrap_err()), "not_found:calendar", "the unlisted legacy calendar went");
    // the switch is the owner's (the source), and it survives a second sync that renames the calendar
    s.reply_set_source_enabled(ReplySourceRef::Calendar { calendar_id: "a".into() }, true, 2).unwrap();
    let r = s.reply_sync_calendars(vec![info("a", "Work (new name)"), info("b", "Home")], 0, 10_000,
        vec![event("a", "e1", "Standup", 100, 200), event("a", "e1", "Standup moved", 300, 400), event("a", "late", "Outside", 50_000, 60_000), event("b", "e2", "Off calendar", 100, 200)], 3).unwrap();
    assert_eq!((r.events.len(), r.ignored_events), (1, 2), "a repeated id replaces the first; outside the window and a calendar that is off are not kept");
    let a = s.reply_calendars().unwrap().into_iter().find(|c| c.calendar_id == "a").unwrap();
    assert_eq!((a.name.as_str(), a.enabled), ("Work (new name)", true));
    let ev = s.reply_calendar_events().unwrap();
    assert_eq!((ev.len(), ev[0].title.as_str(), ev[0].attendees.clone(), ev[0].location.clone()), (1, "Standup moved", vec!["a@x.test".to_string(), "b \"q\"@x.test".to_string()], Some("Room 1".to_string())));
    // events outside a later window stay; events inside it are replaced by what the shell sends (here: none)
    s.reply_sync_calendars(vec![info("a", "Work"), info("b", "Home")], 20_000, 30_000, vec![event("a", "mid", "Later", 25_000, 26_000)], 4).unwrap();
    assert_eq!(s.reply_calendar_events().unwrap().len(), 2);
    s.reply_sync_calendars(vec![info("a", "Work"), info("b", "Home")], 0, 10_000, vec![], 5).unwrap();
    assert_eq!(s.reply_calendar_events().unwrap().iter().map(|e| e.event_id.as_str()).collect::<Vec<_>>(), ["mid"], "the window's events were replaced by none; the later one stays");
    // a calendar the shell no longer lists goes, with its switch and its events
    s.reply_sync_calendars(vec![info("b", "Home")], 20_000, 30_000, vec![], 6).unwrap();
    assert_eq!(s.reply_calendars().unwrap().iter().map(|c| c.calendar_id.as_str()).collect::<Vec<_>>(), ["b"]);
    assert_eq!(code(s.reply_set_source_enabled(ReplySourceRef::Calendar { calendar_id: "a".into() }, true, 7).unwrap_err()), "not_found:calendar", "its source row went with it");
    assert_eq!(count(&s, "SELECT count(*) FROM calendar_event WHERE calendar_id = 'a'"), 0);
}
// mutant S10: reply_sync_calendars no longer deletes the source row of a dropped calendar.
// mutant S12: a new calendar's source row starts ON.
// mutant S13: the events inside the window are not deleted before the kept ones are written.

#[test]
fn a_bad_calendar_sync_is_refused_whole_and_changes_nothing() {
    let (s, _, _d) = open();
    s.reply_sync_calendars(vec![info("a", "Work")], 0, 10_000, vec![], 1).unwrap();
    let bad_colour = ReplyCalendarInfo { colour: "red".into(), ..info("b", "Home") };
    assert_eq!(code(s.reply_sync_calendars(vec![info("a", "Renamed"), bad_colour], 0, 10_000, vec![], 2).unwrap_err()), CALENDAR_SYNC_REFUSAL_CODE);
    assert_eq!(code(s.reply_sync_calendars(vec![info("a", "Work")], 10_000, 10_000, vec![], 3).unwrap_err()), CALENDAR_SYNC_REFUSAL_CODE, "an empty window");
    assert_eq!(code(s.reply_sync_calendars(vec![info("a", "Work")], 0, 10_000, vec![event("a", "e", "Backwards", 200, 100)], 4).unwrap_err()), CALENDAR_SYNC_REFUSAL_CODE);
    let cals = s.reply_calendars().unwrap();
    assert_eq!((cals.len(), cals[0].name.as_str()), (1, "Work"), "refused syncs changed nothing");
}

#[test]
fn folder_progress_counts_messages_stored_bodies_stored_and_bodies_whose_chunks_are_all_embedded() {
    let (s, account, _d) = open();
    let with_body = |id: &str, thread: &str, at: i64| { let mut n = new_msg(msg(id, thread, &account, "x@ext.test", at), None); n.body_text = Some(format!("body {id}")); n };
    s.insert_messages(vec![with_body("m1", "t1", 1_000), with_body("m2", "t2", 2_000), with_body("m3", "t3", 3_000), new_msg(msg("m4", "t4", &account, "x@ext.test", 4_000), None)]).unwrap();
    let chunk = |m: &'static str, n: i64, embedded: &'static str| s.write(move |c| Ok(c.tx.execute(&format!(
        "INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text, embedded_at_ms) VALUES ('{m}', 'body', {n}, 1, 's', 'INBOX', 'subj', 'text', {embedded})"), [])?)).unwrap();
    chunk("m1", 0, "5"); chunk("m1", 1, "6");      // every chunk embedded: indexed
    chunk("m2", 0, "5"); chunk("m2", 1, "NULL");   // one chunk not embedded yet: not indexed
    let p = s.reply_folder_progress().unwrap();
    assert_eq!(p, vec![FolderProgress { account_id: account, folder: "INBOX".into(), messages: 4, bodies: 3, indexed: 1 }]);
}
// mutant S11: the "no chunk is still waiting for its vector" half of the indexed count is removed.

#[test]
fn registering_folders_keeps_the_owners_choice_switches_off_a_folder_the_server_lost_and_trusts_no_empty_list() {
    let (s, account, _d) = open();
    let f = |n: &str| ReplySourceRef::MailFolder { account_id: account.clone(), folder: n.into() };
    let state = |s: &Store| s.reply_sources().unwrap().into_iter().map(|x| (reply_source_key(x.source), x.enabled, x.item_count)).collect::<Vec<_>>();
    let on = |s: &Store, n: &str| s.reply_sources().unwrap().into_iter().find(|x| x.source == f(n)).map(|x| x.enabled);
    s.reply_register_folders(account.clone(), vec![("INBOX".into(), true), ("Sent".into(), true), ("Projects".into(), false)], 1).unwrap();
    assert_eq!((on(&s, "INBOX"), on(&s, "Sent"), on(&s, "Projects")), (Some(true), Some(true), Some(false)));
    s.reply_set_source_enabled(f("INBOX"), false, 2).unwrap();
    s.reply_set_source_enabled(f("Projects"), true, 2).unwrap();
    s.reply_source_progress(f("Projects"), 42, ReplySyncProgress::UpToDate, 2).unwrap();
    s.reply_register_folders(account.clone(), vec![("INBOX".into(), true), ("Sent".into(), true), ("Projects".into(), false)], 3).unwrap();
    assert_eq!((on(&s, "INBOX"), on(&s, "Projects")), (Some(false), Some(true)), "a second registration changes no switch");
    // Projects is removed on the server: its switch goes off, its row and its count stay
    s.reply_register_folders(account.clone(), vec![("INBOX".into(), true), ("Sent".into(), true)], 4).unwrap();
    assert_eq!(on(&s, "Projects"), Some(false), "off, and the row is kept");
    assert_eq!(s.reply_sources().unwrap().into_iter().find(|x| x.source == f("Projects")).unwrap().item_count, 42);
    // an empty LIST changes nothing
    s.reply_set_source_enabled(f("Sent"), true, 5).unwrap();
    let before = state(&s);
    s.reply_register_folders(account.clone(), vec![], 6).unwrap();
    assert_eq!(state(&s), before);
    assert!(s.reply_register_folders("no-such-account".into(), vec![("INBOX".into(), true)], 7).is_err(), "an unknown account is refused, not stored");
}

// delta 23 safety: a row written the body-r2 way (JSON in remote_id, columns empty) reads the same as one written to the columns
#[test]
fn attachment_reads_take_the_columns_first_and_the_old_json_as_fallback() {
    let (s, account, _d) = open();
    s.insert_messages(vec![new_msg(msg("m1", "t1", &account, "x@ext.test", 1_000), None)]).unwrap();
    s.write(|c| {
        c.tx.execute("INSERT INTO attachment (id, message_id, remote_id, file_name, mime_type, size_bytes) VALUES ('m1:att:001', 'm1', '{\"part\":\"2\",\"enc\":\"base64\",\"cid\":\"logo@x\",\"inline\":true}', 'a.png', 'image/png', 5)", [])?;
        c.tx.execute("INSERT INTO attachment (id, message_id, part_path, transfer_encoding, content_id, inline, file_name, mime_type, size_bytes) VALUES ('m1:att:002', 'm1', '1.2', 'quoted-printable', NULL, 0, 'b.pdf', 'application/pdf', 6)", [])?;
        Ok(())
    }).unwrap();
    let by = |id: &str| s.attachment_source(id).unwrap().unwrap();
    assert_eq!((by("m1:att:001").part.as_str(), by("m1:att:001").encoding.as_str()), ("2", "base64"), "JSON fallback");
    assert_eq!((by("m1:att:002").part.as_str(), by("m1:att:002").encoding.as_str()), ("1.2", "quoted-printable"), "columns first");
    let files = s.attachments("m1").unwrap();
    assert_eq!((files[0].inline, files[0].content_id.clone()), (true, Some("logo@x".into())), "inline and cid from the JSON");
    assert_eq!((files[1].inline, files[1].content_id.clone()), (false, None), "columns, empty cid is None");
}

// delta 24: a folder removed on the server stays, off, with its last count, and returns with the owner's last switch
#[test]
fn a_folder_gone_from_the_server_keeps_its_row_and_returns_with_the_owners_switch() {
    let (s, account, _d) = open();
    let src = ReplySourceRef::MailFolder { account_id: account.clone(), folder: "Old".into() };
    s.reply_register_folders(account.clone(), vec![("Old".into(), true), ("INBOX".into(), true)], 5).unwrap();
    s.reply_set_source_enabled(src.clone(), true, 6).unwrap();
    s.reply_source_progress(src.clone(), 1_840, ReplySyncProgress::UpToDate, 7).unwrap();
    let row = |s: &Store| s.reply_sources().unwrap().into_iter().find(|r| r.source == src).unwrap();
    assert!(row(&s).enabled);
    assert!(s.reply_source_presence(src.clone(), false, 8).unwrap());
    let g = row(&s);
    assert_eq!((g.enabled, g.item_count, g.progress.clone()), (false, 1_840, ReplySyncProgress::Gone), "off, last count kept, Gone");
    assert_eq!(s.reply_owner_choices().unwrap().iter().map(|c| c.owner_enabled).collect::<Vec<_>>(), vec![true], "the read carries what the owner last chose, after a relaunch too");
    s.reply_set_source_enabled(src.clone(), false, 8).unwrap();
    assert_eq!(s.reply_owner_choices().unwrap()[0].owner_enabled, false, "flipping a gone row rewrites the remembered choice");
    s.reply_set_source_enabled(src.clone(), true, 9).unwrap();
    assert!(!row(&s).enabled, "the switch of a gone folder does not turn it on");
    s.reply_source_presence(src.clone(), false, 10).unwrap();
    assert!(!row(&s).enabled && count(&s, "SELECT owner_enabled FROM reply_source WHERE gone = 1") == 1, "a second LIST that still misses it keeps the owner's choice");
    s.reply_source_presence(src.clone(), true, 11).unwrap();
    let back = row(&s);
    assert!(back.enabled && back.progress != ReplySyncProgress::Gone, "it returns with the switch the owner last chose");
    assert!(s.reply_owner_choices().unwrap().is_empty(), "a present folder carries no remembered choice");
    assert_eq!(count(&s, "SELECT count(*) FROM reply_source WHERE gone = 1 OR owner_enabled IS NOT NULL"), 0);
}
