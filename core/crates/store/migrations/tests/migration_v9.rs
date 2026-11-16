//! Migration 0009 (contracts delta 17): a label is a flow of three cards. Defaults that keep every existing label behaving as before,
//! the chain numbers of existing rules, the backfill of which card placed a filing, the rules that keep the flow legal, the reversal.
//! MUTANTS (the lead runs each at the build word; the test named must go red):
//!   M1 entry_enabled defaults to 0 (a label's rules stop firing)               -> a_label_that_existed_before_behaves_as_before
//!   M2 exit_enabled defaults to 1                                              -> a_label_that_existed_before_behaves_as_before
//!   M3 ai_reads_attachments defaults to 1                                      -> a_label_that_existed_before_behaves_as_before
//!   M4 the chain backfill numbers every rule 0                                 -> existing_rules_become_entry_chains_numbered_by_position
//!   M5 the unique index on (label, placement, chain) is dropped                -> existing_rules_become_entry_chains_numbered_by_position
//!   M6 the backfill leaves a rule-decided filing with no card                  -> a_filing_keeps_the_card_that_placed_it
//!   M7 the exit-needs-a-fallback trigger is dropped                            -> the_flow_stays_legal_in_the_store_too
//!   M8 the fallback-is-a-place trigger accepts an unknown id                   -> the_flow_stays_legal_in_the_store_too
//!   M9 the AI-reads-something trigger is dropped                               -> the_flow_stays_legal_in_the_store_too
//!   M10 the delete-fallback trigger is dropped                                 -> a_label_that_is_a_fallback_leaves_with_its_filter
//!   M11 the reversal keeps the exit chains                                     -> the_reversal_gives_back_0008_and_loses_only_the_flow
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn cols(c: &Connection, t: &str) -> Vec<String> { c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() }
fn one<T: rusqlite::types::FromSql>(c: &Connection, sql: &str) -> T { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn refused(r: rusqlite::Result<usize>) -> String { match r { Ok(_) => panic!("accepted but must be refused"), Err(e) => e.to_string() } }
fn label(c: &Connection, id: &str) { c.execute("INSERT INTO label (id, name, updated_at_ms) VALUES (?1, ?1, 1)", [id]).unwrap(); }
fn rule(c: &Connection, id: &str, position: i64, label_id: &str) {
    c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode) VALUES (?1, ?2, '[{\"kind\":\"from\",\"words\":\"a\"}]', '', 1, 1, 'label', ?3, 'any')", params![id, position, label_id]).unwrap();
}
fn message(c: &Connection, id: &str) {
    c.execute("INSERT OR IGNORE INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'google', 'W', '#336699', 0, 'oauth', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject) VALUES (?1, 'acc', 's')", [format!("t-{id}")]).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder) VALUES (?1, ?2, 'acc', 'A', 'a@example.test', 's', 1, 'INBOX')", params![id, format!("t-{id}")]).unwrap();
}

#[test]
fn a_label_that_existed_before_behaves_as_before() {
    let mut c = at(8);
    label(&c, "old");
    migrate(&mut c).unwrap();
    let row: (i64, i64, i64, i64, i64, i64, Option<String>) = c.query_row(
        "SELECT entry_enabled, ai_enabled, exit_enabled, ai_reads_subject, ai_reads_body, ai_reads_attachments, exit_fallback FROM label WHERE id = 'old'", [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).unwrap();
    assert_eq!(row, (1, 1, 0, 1, 1, 0, None), "entry on, AI on, exit off, subject and body read, no fallback");
    let built_in: (i64, i64) = c.query_row("SELECT entry_enabled, exit_enabled FROM label WHERE id = 'needs-reply'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(built_in, (1, 0), "the built-in categories too");
}

#[test]
fn existing_rules_become_entry_chains_numbered_by_position() {
    let mut c = at(8);
    label(&c, "a"); label(&c, "b");
    rule(&c, "r-late", 5, "a"); rule(&c, "r-early", 2, "a"); rule(&c, "r-only", 1, "b");
    c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms) VALUES ('r-tab', 0, '[]', 'updates', 1, 1)", []).unwrap();
    migrate(&mut c).unwrap();
    let chain = |id: &str| -> (String, i64) { c.query_row("SELECT placement, chain_id FROM rule WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap() };
    assert_eq!(chain("r-early"), ("entry".into(), 0), "the first by position is chain 0");
    assert_eq!(chain("r-late"), ("entry".into(), 1));
    assert_eq!(chain("r-only"), ("entry".into(), 0), "another label numbers from 0 again");
    assert_eq!(chain("r-tab"), ("entry".into(), 0), "a tab rule keeps the defaults");
    assert!(refused(c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode, placement, chain_id) VALUES ('dup', 9, '[]', '', 1, 1, 'label', 'a', 'any', 'entry', 1)", [])).contains("UNIQUE"),
            "two chains of one (label, placement) cannot share a number");
    c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode, placement, chain_id) VALUES ('x0', 9, '[]', '', 1, 1, 'label', 'a', 'any', 'exit', 0)", []).unwrap();
    assert!(refused(c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, placement) VALUES ('tabx', 9, '[]', 'updates', 1, 1, 'exit')", [])).contains("belongs to a label"),
            "an exit chain files into a label, never a tab");
}

#[test]
fn a_filing_keeps_the_card_that_placed_it() {
    let mut c = at(8);
    label(&c, "a"); rule(&c, "r1", 1, "a"); rule(&c, "r2", 2, "a");
    for m in ["m-rule", "m-jev", "m-laya", "m-owner"] { message(&c, m); }
    let file = |m: &str, by: &str, rule: Option<&str>| c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, rule_id) VALUES ('a', ?1, 1, ?2, ?3)", params![m, by, rule]).unwrap();
    file("m-rule", "rule", Some("r2")); file("m-jev", "jev", None); file("m-laya", "laya", None); file("m-owner", "owner", None);
    migrate(&mut c).unwrap();
    let card = |m: &str| -> (Option<String>, Option<i64>) { c.query_row("SELECT flow_card, flow_chain_id FROM label_assignment WHERE message_id = ?1", [m], |r| Ok((r.get(0)?, r.get(1)?))).unwrap() };
    assert_eq!(card("m-rule"), (Some("entry".into()), Some(1)), "placed by the entry card, chain 1 (r2 is the second by position)");
    assert_eq!(card("m-jev"), (Some("ai".into()), None));
    assert_eq!(card("m-laya"), (Some("ai".into()), None));
    assert_eq!(card("m-owner"), (None, None), "by hand: no card");
    assert!(refused(c.execute("UPDATE label_assignment SET flow_card = 'sorcery' WHERE message_id = 'm-jev'", [])).contains("CHECK"));
}

#[test]
fn the_flow_stays_legal_in_the_store_too() {
    let mut c = at(8);
    label(&c, "a"); label(&c, "b");
    migrate(&mut c).unwrap();
    assert!(refused(c.execute("UPDATE label SET exit_enabled = 1 WHERE id = 'a'", [])).contains("Say where the other messages go"), "exit on with no fallback");
    assert!(refused(c.execute("INSERT INTO label (id, name, updated_at_ms, exit_enabled) VALUES ('n', 'n', 1, 1)", [])).contains("Say where"), "also on insert");
    c.execute("UPDATE label SET exit_enabled = 1, exit_fallback = 'primary' WHERE id = 'a'", []).unwrap();
    assert!(refused(c.execute("UPDATE label SET exit_fallback = NULL WHERE id = 'a'", [])).contains("Say where"), "clearing the fallback while the filter is on");
    assert!(refused(c.execute("UPDATE label SET exit_fallback = 'ghost' WHERE id = 'a'", [])).contains("another label"), "an unknown place");
    assert!(refused(c.execute("UPDATE label SET exit_fallback = 'all' WHERE id = 'a'", [])).contains("another label"), "the All view is no place");
    assert!(refused(c.execute("UPDATE label SET exit_fallback = 'a' WHERE id = 'a'", [])).contains("another label"), "not itself");
    for ok in ["b", "junk", "primary"] { c.execute("UPDATE label SET exit_fallback = ?1 WHERE id = 'a'", [ok]).unwrap(); }
    assert!(refused(c.execute("UPDATE label SET ai_reads_subject = 0, ai_reads_body = 0 WHERE id = 'a'", [])).contains("at least one"), "the AI reads something");
    c.execute("UPDATE label SET ai_reads_subject = 0, ai_reads_body = 0, ai_reads_attachments = 1 WHERE id = 'a'", []).unwrap();
}

#[test]
fn a_label_that_is_a_fallback_leaves_with_its_filter() {
    let mut c = at(8);
    label(&c, "a"); label(&c, "b");
    migrate(&mut c).unwrap();
    c.execute("UPDATE label SET exit_enabled = 1, exit_fallback = 'b' WHERE id = 'a'", []).unwrap();
    c.execute("DELETE FROM label WHERE id = 'b'", []).unwrap();
    let row: (i64, Option<String>) = c.query_row("SELECT exit_enabled, exit_fallback FROM label WHERE id = 'a'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(row, (0, None), "the label that sent mail there switches its exit filter off and forgets the place");
}

#[test]
fn a_rejection_goes_with_its_message_and_its_label() {
    let mut c = at(8);
    label(&c, "a"); message(&c, "m1"); message(&c, "m2");
    migrate(&mut c).unwrap();
    c.execute("INSERT INTO flow_rejection (message_id, ai_label_id, sent_to, chain_id, decided_at_ms) VALUES ('m1', 'a', 'primary', 0, 5)", []).unwrap();
    c.execute("INSERT INTO flow_rejection (message_id, ai_label_id, sent_to, chain_id, decided_at_ms) VALUES ('m2', 'a', 'junk', NULL, 6)", []).unwrap();
    assert!(refused(c.execute("INSERT INTO flow_rejection (message_id, ai_label_id, sent_to, decided_at_ms) VALUES ('m1', 'a', 'junk', 7)", [])).contains("UNIQUE"), "one row per message");
    c.execute("DELETE FROM message WHERE id = 'm1'", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM flow_rejection"), 1);
    c.execute("DELETE FROM label WHERE id = 'a'", []).unwrap();
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM flow_rejection"), 0, "the AI's pick is gone with its label");
}

#[test]
fn the_reversal_gives_back_0008_and_loses_only_the_flow() {
    let v8 = at(8);
    let mut c = at(8);
    label(&c, "a"); rule(&c, "r1", 1, "a"); rule(&c, "r2", 2, "a");
    migrate(&mut c).unwrap();
    c.execute("INSERT INTO rule (id, position, conditions_json, tab_id, enabled, updated_at_ms, action_kind, action_label_id, match_mode, placement, chain_id) VALUES ('x0', 9, '[]', '', 1, 1, 'label', 'a', 'any', 'exit', 0)", []).unwrap();
    c.execute("UPDATE label SET exit_enabled = 1, exit_fallback = 'junk', entry_enabled = 0 WHERE id = 'a'", []).unwrap();
    let applied = revert_to(&mut c, 8).unwrap();
    assert_eq!(applied.to_version, 8);
    for t in ["label", "rule", "label_assignment"] { assert_eq!(cols(&c, t), cols(&v8, t), "{t}: the columns are 0008's"); }
    let tables: i64 = one(&c, "SELECT count(*) FROM sqlite_master WHERE name = 'flow_rejection'");
    assert_eq!(tables, 0);
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rule WHERE id = 'x0'"), 0, "the exit chain is the one deleted row");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM rule WHERE id IN ('r1', 'r2')"), 2, "the entry rules stay as the alternatives delta 9 read");
}
