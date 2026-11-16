//! Migration 0013 (contracts delta 20): AI replies. Eleven tables, no existing row or column touched, a reversal.
//! MUTANTS (each must turn the test named red; _captures/replies-contract/mutants.py applies and restores them):
//!   S01 reply_use loses its kind CHECK (a third use is stored)                                   -> the_two_uses_are_seeded_and_a_use_reads_the_voice_or_an_existing_tone_and_prompt
//!   S02 the reply_use tone trigger is dropped (an unknown tone is stored)                        -> the_two_uses_are_seeded_and_a_use_reads_the_voice_or_an_existing_tone_and_prompt
//!   S03 the reply_tone_removed trigger is dropped (a removed tone leaves a dangling use)         -> a_removed_tone_sends_the_uses_back_to_the_voice
//!   S04 reply_tone_one_name is not unique (two tones share a name, any case)                     -> a_tone_has_one_name_and_never_the_voices_id
//!   S05 reply_filter loses CHECK (id = 1) (a second filter row is stored)                         -> one_filter_one_row_that_reads_something_and_asks_something_when_the_ai_is_on
//!   S06 reply_filter loses the at-least-one-read CHECK                                            -> one_filter_one_row_that_reads_something_and_asks_something_when_the_ai_is_on
//!   S07 reply_filter loses the AI-needs-its-question CHECK                                        -> one_filter_one_row_that_reads_something_and_asks_something_when_the_ai_is_on
//!   S08 reply_filter_label has no ON DELETE CASCADE                                               -> a_deleted_label_leaves_the_ticked_labels
//!   S09 the reply_prompt text-is-sealed trigger is dropped (a version's text is edited)           -> prompt_versions_only_grow_and_one_is_current
//!   S10 the reply_prompt delete trigger is dropped (a version is deleted)                         -> prompt_versions_only_grow_and_one_is_current
//!   S11 reply_prompt_one_current is not unique (two current versions)                             -> prompt_versions_only_grow_and_one_is_current
//!   S12 reply_draft_one_live_per_thread is dropped (two live drafts in one thread)                -> one_live_draft_per_thread_and_a_sealed_draft_never_changes
//!   S13 the reply_draft_is_sealed trigger is dropped (a sent draft is edited)                     -> one_live_draft_per_thread_and_a_sealed_draft_never_changes
//!   S14 the reply_draft_never_goes_back trigger is dropped (edited goes back to proposed)         -> one_live_draft_per_thread_and_a_sealed_draft_never_changes
//!   S15 reply_draft's message reference has no ON DELETE CASCADE                                  -> a_draft_goes_with_its_message_and_its_thread
//!   S16 reply_pass loses the stop-leaves-later-steps-unreached CHECK                              -> a_pass_has_one_row_per_message_and_only_legal_shapes
//!   S17 reply_pass loses the draft-needs-all-three CHECK                                          -> a_pass_has_one_row_per_message_and_only_legal_shapes
//!   S18 reply_pass's key is not the message (a message gets two rows)                             -> a_pass_has_one_row_per_message_and_only_legal_shapes
//!   S19 reply_source loses the key-matches-kind CHECK                                             -> a_source_is_its_kind_its_account_and_a_consistent_progress
//!   S20 reply_source loses the mail-folder-needs-an-account CHECK                                 -> a_source_is_its_kind_its_account_and_a_consistent_progress
//!   S21 reply_source loses the progress consistency CHECKs (syncing with no percent)              -> a_source_is_its_kind_its_account_and_a_consistent_progress
//!   S22 reply_source's account reference has no ON DELETE CASCADE                                 -> a_removed_account_takes_its_sources
//!   S23 retrieval_settings accepts keep_n 0                                                       -> the_singletons_hold_one_row_with_the_boards_defaults
//!   S24 the seed starts the auto use ON                                                           -> the_two_uses_are_seeded_and_a_use_reads_the_voice_or_an_existing_tone_and_prompt
//!   S25 the reversal keeps reply_draft                                                            -> the_reversal_gives_back_0012
//!   S27 label.ai_reads_thread defaults to 1 (every existing label starts reading the thread)       -> a_copy_of_an_existing_database_migrates_and_keeps_every_row
//!   S26 the migration changes an existing table (message gains a column)                          -> a_copy_of_an_existing_database_migrates_and_keeps_every_row
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn latest() -> Connection { let mut c = at(12); migrate(&mut c).unwrap(); c }
fn mail(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    for id in ["m1", "m2", "m3"] {
        c.execute("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) VALUES (?1, 't1', 'acc', ?1, 'S', 's@x.example', 's', 1, 'INBOX')", params![id]).unwrap();
    }
}
fn message_cols(c: &Connection) -> Vec<String> {
    c.prepare("SELECT name FROM pragma_table_info('message')").unwrap().query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
}
// the columns `message` gained after the version this test starts from, at the end, in the order they were added: 0023 (lead ruling A144) the
// two threading headers, 0027 (lane real-replyto) the Reply-To. A later migration that adds a column to `message` must name it here.
fn message_gained_since_0022() -> Vec<String> { ["rfc_message_id", "rfc_references_json", "reply_to_json"].map(String::from).to_vec() }
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn refused(c: &Connection, sql: &str, why: &str) { assert!(c.execute(sql, []).is_err(), "must be refused: {why}"); }

#[test]
fn a_copy_of_an_existing_database_migrates_and_keeps_every_row() {
    // a real database FILE at version 12 with mail in it, copied, and the COPY migrated: the original stays at 12, the copy gets 0013 and every old row
    let dir = std::env::temp_dir().join(format!("whoosh-m13-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let mut c = Connection::open(&orig).unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        for (_, sql) in &MIGRATIONS[..12] { c.execute_batch(sql).unwrap(); }
        c.pragma_update(None, "user_version", 12i64).unwrap();
        mail(&c);
        c.execute("INSERT INTO junk_learned (message_id, label) VALUES ('m1', 'junk')", []).unwrap();
        let _ = &mut c;
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").ok();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let before = { let c = Connection::open(&copy).unwrap(); (n(&c, "SELECT count(*) FROM message"), n(&c, "SELECT count(*) FROM thread"), n(&c, "SELECT count(*) FROM account"), n(&c, "SELECT count(*) FROM junk_learned")) };
    let before_message_columns = { let c = Connection::open(&copy).unwrap(); message_cols(&c) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (12, MIGRATIONS.last().unwrap().0));
    assert_eq!((n(&c, "SELECT count(*) FROM message"), n(&c, "SELECT count(*) FROM thread"), n(&c, "SELECT count(*) FROM account"), n(&c, "SELECT count(*) FROM junk_learned")), before, "every old row is still there");
    let mut want_message_columns = before_message_columns.clone();
    want_message_columns.extend(message_gained_since_0022());
    assert_eq!(message_cols(&c), want_message_columns, "every old column of message is as it was, in its place, and message gains exactly rfc_message_id and rfc_references_json (0023, lead ruling A144) and reply_to_json (0027), at the end; no table is added or dropped (the three columns 0028 adds to other tables are named in migration_v28_settings_cmds.rs)");
    assert_eq!(n(&c, "SELECT count(*) FROM reply_use"), 2, "the new tables are seeded");
    c.execute("INSERT INTO label (id, name, path, depth, updated_at_ms) VALUES ('l1', 'Clients', 'Clients', 1, 1)", []).unwrap();
    assert_eq!(n(&c, "SELECT ai_reads_thread FROM label WHERE id = 'l1'"), 0, "a label reads no thread until its owner ticks it: every label behaves as before");
    assert!(c.execute("UPDATE label SET ai_reads_thread = 2", []).is_err(), "a tick is 0 or 1");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 12, "the original database file is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_two_uses_are_seeded_and_a_use_reads_the_voice_or_an_existing_tone_and_prompt() {
    let c = latest();
    let rows: Vec<(String, i64, String, i64, String)> = c.prepare("SELECT kind, enabled, tone_id, match_thread, prompt_id FROM reply_use ORDER BY kind").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(rows, vec![("auto".into(), 0, "voice".into(), 1, "replies".into()), ("on_demand".into(), 1, "voice".into(), 1, "replies".into())], "the auto use starts OFF: nothing is drafted and no mail text leaves until the owner says so");
    refused(&c, "INSERT INTO reply_use (kind, enabled, tone_id, match_thread, prompt_id) VALUES ('later', 0, 'voice', 1, 'replies')", "a third use");
    refused(&c, "UPDATE reply_use SET tone_id = 'ghost' WHERE kind = 'auto'", "a tone that does not exist");
    refused(&c, "UPDATE reply_use SET prompt_id = 'ghost' WHERE kind = 'auto'", "a prompt that does not exist");
    c.execute("INSERT INTO reply_tone (id, name, text, sort_order) VALUES ('t1', 'Formel', 'Formal.', 0)", []).unwrap();
    c.execute("UPDATE reply_use SET tone_id = 't1' WHERE kind = 'auto'", []).unwrap();
}

#[test]
fn a_removed_tone_sends_the_uses_back_to_the_voice() {
    let c = latest();
    c.execute("INSERT INTO reply_tone (id, name, text) VALUES ('t1', 'Formel', 'Formal.')", []).unwrap();
    c.execute("UPDATE reply_use SET tone_id = 't1'", []).unwrap();
    c.execute("DELETE FROM reply_tone WHERE id = 't1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM reply_use WHERE tone_id = 'voice'"), 2, "both uses fall back to Ma voix");
}

#[test]
fn a_tone_has_one_name_and_never_the_voices_id() {
    let c = latest();
    c.execute("INSERT INTO reply_tone (id, name, text) VALUES ('t1', 'Formel', 'Formal.')", []).unwrap();
    refused(&c, "INSERT INTO reply_tone (id, name, text) VALUES ('t2', ' formel ', 'Again.')", "the same name, any case, with spaces");
    refused(&c, "INSERT INTO reply_tone (id, name, text) VALUES ('voice', 'Mine', 'x')", "the voice's own id");
    refused(&c, "INSERT INTO reply_tone (id, name, text) VALUES ('t3', ' ', 'x')", "no name");
    refused(&c, "INSERT INTO reply_tone (id, name, text) VALUES ('t3', 'Bref', ' ')", "no text");
}

#[test]
fn one_filter_one_row_that_reads_something_and_asks_something_when_the_ai_is_on() {
    let c = latest();
    let (scope_all, e, a, x, subj, body, thread, att): (i64, i64, i64, i64, i64, i64, i64, i64) = c.query_row(
        "SELECT scope_all, entry_enabled, ai_enabled, exit_enabled, ai_reads_subject, ai_reads_body, ai_reads_thread, ai_reads_attachments FROM reply_filter", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))).unwrap();
    assert_eq!((scope_all, e, a, x, subj, body, thread, att), (1, 1, 1, 1, 1, 1, 1, 0), "every label, three cards on, the AI reads subject, body and the thread, not the attachments");
    assert!(n(&c, "SELECT length(ai_description) FROM reply_filter") > 10, "the seed question is not blank");
    refused(&c, "INSERT INTO reply_filter (id, ai_description) VALUES (2, 'x')", "a second filter");
    refused(&c, "UPDATE reply_filter SET ai_reads_subject = 0, ai_reads_body = 0, ai_reads_thread = 0, ai_reads_attachments = 0", "the AI reads nothing");
    c.execute("UPDATE reply_filter SET ai_reads_subject = 0, ai_reads_body = 0, ai_reads_attachments = 0", []).unwrap();
    refused(&c, "UPDATE reply_filter SET ai_description = '   '", "the AI card on with a blank question");
    c.execute("UPDATE reply_filter SET ai_enabled = 0, ai_description = ''", []).unwrap();
    refused(&c, "UPDATE reply_filter SET ai_enabled = 1", "turning the AI on with no question");
    refused(&c, "UPDATE reply_filter SET entry_conditions_json = '{\"not\":\"an array\"}'", "conditions that are not a list");
    refused(&c, "UPDATE reply_filter SET exit_match = 'both'", "a match mode that is not any or all");
}

#[test]
fn a_deleted_label_leaves_the_ticked_labels() {
    let c = latest();
    c.execute("INSERT INTO label (id, name, path, depth, updated_at_ms) VALUES ('l1', 'Clients', 'Clients', 1, 1)", []).unwrap();
    c.execute("INSERT INTO reply_filter_label (label_id) VALUES ('l1')", []).unwrap();
    refused(&c, "INSERT INTO reply_filter_label (label_id) VALUES ('ghost')", "a label that does not exist");
    c.execute("DELETE FROM label WHERE id = 'l1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM reply_filter_label"), 0, "the tick goes with the label");
}

#[test]
fn prompt_versions_only_grow_and_one_is_current() {
    let c = latest();
    assert_eq!(n(&c, "SELECT count(*) FROM reply_prompt WHERE prompt_id = 'replies' AND current = 1 AND version = 1"), 1, "version 1 is seeded and current");
    refused(&c, "UPDATE reply_prompt SET text = 'rewritten' WHERE version = 1", "a version's text never changes");
    refused(&c, "UPDATE reply_prompt SET version = 9 WHERE version = 1", "a version's number never changes");
    refused(&c, "DELETE FROM reply_prompt WHERE version = 1", "a version is never deleted");
    refused(&c, "INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES ('replies', 2, 'v2 {{message}}', 5, 1)", "a second current version");
    c.execute("INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES ('replies', 2, 'v2 {{message}}', 5, 0)", []).unwrap();
    c.execute("UPDATE reply_prompt SET current = 0 WHERE version = 1", []).unwrap();
    c.execute("UPDATE reply_prompt SET current = 1 WHERE version = 2", []).unwrap();
    assert_eq!(n(&c, "SELECT version FROM reply_prompt WHERE current = 1"), 2, "only the flag moves");
    refused(&c, "INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES ('replies', 2, 'dup', 6, 0)", "the same version twice");
    refused(&c, "INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES ('replies', 0, 'zero', 6, 0)", "version 0");
    refused(&c, "INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES ('replies', 3, '  ', 6, 0)", "an empty prompt");
}

fn draft(c: &Connection, id: &str, thread: &str, message: &str, state: &str) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, state, created_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, 'Bonjour,', 'auto', ?4, 1, 1)", params![id, thread, message, state])
}

#[test]
fn one_live_draft_per_thread_and_a_sealed_draft_never_changes() {
    let c = latest();
    mail(&c);
    draft(&c, "d1", "t1", "m1", "proposed").unwrap();
    assert!(draft(&c, "d2", "t1", "m2", "proposed").is_err(), "a second live draft in the thread");
    assert!(draft(&c, "d2", "t1", "m2", "edited").is_err(), "an edited one is live too");
    c.execute("UPDATE reply_draft SET state = 'discarded', updated_at_ms = 2 WHERE id = 'd1'", []).unwrap();
    draft(&c, "d2", "t1", "m2", "proposed").expect("the old one is discarded: the thread has room");
    draft(&c, "d3", "t1", "m3", "sent").expect("sealed rows do not count as live");
    refused(&c, "UPDATE reply_draft SET text = 'rewritten' WHERE id = 'd1'", "a discarded draft is sealed");
    refused(&c, "UPDATE reply_draft SET state = 'proposed' WHERE id = 'd3'", "a sent draft is sealed");
    c.execute("UPDATE reply_draft SET state = 'edited' WHERE id = 'd2'", []).unwrap();
    refused(&c, "UPDATE reply_draft SET state = 'proposed' WHERE id = 'd2'", "an edited draft never goes back to proposed");
    c.execute("UPDATE reply_draft SET text = 'Bonjour, voici.', revision = 1 WHERE id = 'd2'", []).unwrap();
    refused(&c, "INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, state, created_at_ms, updated_at_ms) VALUES ('d9', 't1', 'm1', 'x', 'robot', 'discarded', 1, 1)", "a maker that is neither auto nor on_demand");
    refused(&c, "INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, state, created_at_ms, updated_at_ms) VALUES ('d9', 't1', 'm1', ' ', 'auto', 'discarded', 1, 1)", "an empty draft");
    refused(&c, "INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, state, created_at_ms, updated_at_ms) VALUES ('d9', 't1', 'm1', 'x', 'auto', 'rejected', 1, 1)", "a state outside the four");
}

#[test]
fn a_draft_goes_with_its_message_and_its_thread() {
    let c = latest();
    mail(&c);
    draft(&c, "d1", "t1", "m1", "sent").unwrap();
    c.execute("DELETE FROM message WHERE id = 'm1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM reply_draft"), 0, "a sealed draft still goes when its message is deleted: a delete is not an update");
    assert!(draft(&c, "d2", "t1", "nobody", "proposed").is_err(), "a draft for an unknown message");
    assert!(draft(&c, "d2", "ghost", "m2", "proposed").is_err(), "a draft for an unknown thread");
}

fn pass(c: &Connection, id: &str, e: &str, a: &str, x: &str, draft: Option<&str>) -> rusqlite::Result<usize> {
    c.execute("INSERT OR REPLACE INTO reply_pass (message_id, received_at_ms, entry_result, ai_result, exit_result, draft_id) VALUES (?1, 1, ?2, ?3, ?4, ?5)", params![id, e, a, x, draft])
}

#[test]
fn a_pass_has_one_row_per_message_and_only_legal_shapes() {
    let c = latest();
    mail(&c);
    draft(&c, "d1", "t1", "m1", "proposed").unwrap();
    pass(&c, "m1", "passed", "passed", "passed", Some("d1")).unwrap();
    pass(&c, "m2", "stopped", "not_reached", "not_reached", None).unwrap();
    pass(&c, "m3", "passed", "stopped", "not_reached", None).unwrap();
    pass(&c, "m3", "passed", "passed", "stopped", None).expect("a message looked at again REPLACES its row");
    assert_eq!(n(&c, "SELECT count(*) FROM reply_pass"), 3, "one row per message: a count cannot count one arrival twice");
    assert!(pass(&c, "m2", "stopped", "passed", "not_reached", None).is_err(), "after a stop at the entry card nothing is reached");
    assert!(pass(&c, "m2", "passed", "stopped", "passed", None).is_err(), "after the AI stopped it the exit card is not reached");
    assert!(pass(&c, "m2", "stopped", "not_reached", "not_reached", Some("d1")).is_err(), "a draft after a stop");
    assert!(pass(&c, "m2", "passed", "passed", "stopped", Some("d1")).is_err(), "a draft after the exit card stopped it");
    assert!(pass(&c, "m2", "not_reached", "not_reached", "not_reached", None).is_err(), "the entry step is always reached");
    assert!(pass(&c, "ghost", "passed", "passed", "passed", None).is_err(), "a pass for an unknown message");
    c.execute("DELETE FROM reply_draft WHERE id = 'd1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM reply_pass WHERE draft_id IS NULL"), 3, "a removed draft leaves the pass, without the link");
}

fn source(c: &Connection, key: &str, kind: &str, account: Option<&str>, progress: &str, percent: Option<i64>, words: Option<&str>) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO reply_source (source_key, kind, account_id, progress_state, progress_percent, progress_words) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![key, kind, account, progress, percent, words])
}

#[test]
fn a_source_is_its_kind_its_account_and_a_consistent_progress() {
    let c = latest();
    mail(&c);
    source(&c, "mail\u{1f}acc\u{1f}INBOX", "mail_folder", Some("acc"), "up_to_date", None, None).unwrap();
    source(&c, "local\u{1f}~/Documents/Contrats", "local_folder", None, "not_started", None, None).unwrap();
    source(&c, "calendar\u{1f}cal-1", "calendar", None, "syncing", Some(62), None).unwrap();
    source(&c, "mail\u{1f}acc\u{1f}Clients", "mail_folder", Some("acc"), "failed", None, Some("the server said no")).unwrap();
    assert!(source(&c, "local\u{1f}~/x", "calendar", None, "up_to_date", None, None).is_err(), "the key does not match the kind");
    assert!(source(&c, "mail\u{1f}acc\u{1f}Sent", "mail_folder", None, "up_to_date", None, None).is_err(), "a mail folder with no account");
    assert!(source(&c, "local\u{1f}~/y", "local_folder", Some("acc"), "up_to_date", None, None).is_err(), "a folder of this Mac with an account");
    assert!(source(&c, "mail\u{1f}ghost\u{1f}INBOX", "mail_folder", Some("ghost"), "up_to_date", None, None).is_err(), "an account that does not exist");
    assert!(source(&c, "calendar\u{1f}c2", "calendar", None, "syncing", None, None).is_err(), "syncing with no percent");
    assert!(source(&c, "calendar\u{1f}c3", "calendar", None, "up_to_date", Some(50), None).is_err(), "a percent while up to date");
    assert!(source(&c, "calendar\u{1f}c4", "calendar", None, "syncing", Some(101), None).is_err(), "past a full ring");
    assert!(source(&c, "calendar\u{1f}c5", "calendar", None, "failed", None, None).is_err(), "failed with no words");
    assert!(source(&c, "calendar\u{1f}c6", "calendar", None, "paused", None, None).is_err(), "a state outside the four");
    refused(&c, "UPDATE reply_source SET item_count = -1", "a negative count");
}

#[test]
fn a_removed_account_takes_its_sources() {
    let c = latest();
    mail(&c);
    source(&c, "mail\u{1f}acc\u{1f}INBOX", "mail_folder", Some("acc"), "up_to_date", None, None).unwrap();
    source(&c, "local\u{1f}~/Documents", "local_folder", None, "up_to_date", None, None).unwrap();
    c.execute("DELETE FROM account WHERE id = 'acc'", []).unwrap();
    assert_eq!((n(&c, "SELECT count(*) FROM reply_source WHERE kind = 'mail_folder'"), n(&c, "SELECT count(*) FROM reply_source WHERE kind = 'local_folder'")), (0, 1), "the account's folders go, the folders of this Mac stay");
}

#[test]
fn the_singletons_hold_one_row_with_the_boards_defaults() {
    let c = latest();
    for t in ["reply_filter", "reply_on_demand", "retrieval_settings", "reply_voice"] { assert_eq!(n(&c, &format!("SELECT count(*) FROM {t}")), 1, "{t}"); }
    refused(&c, "INSERT INTO reply_on_demand (id) VALUES (2)", "a second on-demand row");
    refused(&c, "INSERT INTO reply_voice (id, sample_size, same_person_examples) VALUES (2, 100, 3)", "a second voice row");
    let (q, e, k, size, clash, filter, scope): (i64, i64, i64, String, String, Option<String>, String) = c.query_row(
        "SELECT questions_max, excerpts_per_question, keep_n, excerpt_size, clash_rule, ai_filter_question, scope FROM retrieval_settings", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).unwrap();
    assert_eq!((q, e, k, size.as_str(), clash.as_str(), scope.as_str()), (3, 5, 6, "paragraph", "keep_both_dated", "this_account"), "the Recherche pane's lines");
    assert!(filter.is_some());
    refused(&c, "UPDATE retrieval_settings SET keep_n = 0", "keep none");
    refused(&c, "UPDATE retrieval_settings SET questions_max = 0", "no questions");
    refused(&c, "UPDATE retrieval_settings SET ai_filter_question = '  '", "a blank filter question: off is NULL");
    c.execute("UPDATE retrieval_settings SET ai_filter_question = NULL", []).unwrap();
    refused(&c, "UPDATE retrieval_settings SET excerpt_size = 'page'", "a size outside the three");
    refused(&c, "UPDATE reply_voice SET sample_size = 0", "an analysis of nothing");
    refused(&c, "UPDATE reply_voice SET same_person_examples = -1", "a negative number of examples");
    assert_eq!((n(&c, "SELECT instruction_before FROM reply_on_demand"), n(&c, "SELECT change_after FROM reply_on_demand")), (1, 1));
    c.execute("INSERT INTO fixed_note (id, title, text) VALUES ('n1', 'Prix', 'Never quote a price.')", []).unwrap();
    refused(&c, "INSERT INTO fixed_note (id, title, text) VALUES ('n2', ' ', 'x')", "a note with no title");
}

#[test]
fn the_reversal_gives_back_0012() {
    let mut c = at(12);
    let (before_tables, before_hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    for t in ["reply_use", "reply_filter", "reply_filter_label", "reply_on_demand", "reply_pass", "reply_source", "retrieval_settings", "reply_voice", "reply_tone", "reply_prompt", "fixed_note", "reply_draft"] {
        assert!(table_names(&c).unwrap().contains(&t.to_string()), "{t}");
    }
    revert_to(&mut c, 12).unwrap();
    assert_eq!(table_names(&c).unwrap(), before_tables, "0012's tables exactly");
    assert_eq!(schema_hash(&c).unwrap(), before_hash, "0012's schema exactly: every index and trigger of 0013 is gone");
}
