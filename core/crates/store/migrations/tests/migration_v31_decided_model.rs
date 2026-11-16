//! Migration 0031 (lane fix-aisort round 8; the lead's ruling: additive only): `decided_model` on `label_assignment` and on `ai_label_no`,
//! NULL for every existing row; 'apple' marks a filing or a "no" Apple's on-device model made (stored as the on-device decider 'laya').
//! "The version before" is every migration below 31.
//! Not run by this lane (no cargo here): the lead runs `cargo test --test migration_v31_decided_model` in core/crates/store/migrations.
//! Broken copies (one edit each):
//!   M1 0031_decided_model.sql: `ALTER TABLE ai_label_no ADD COLUMN decided_model TEXT;` removed -> a_database_with_laya_and_jev_rows_migrates_up_with_every_row_and_two_new_columns
//!   M2 0031_decided_model.sql: `decided_model TEXT` -> `decided_model TEXT NOT NULL DEFAULT 'apple'` on label_assignment
//!        -> a_database_with_laya_and_jev_rows_migrates_up_with_every_row_and_two_new_columns (old rows would read as Apple's)
//!   M3 0031_decided_model.down.sql: `ALTER TABLE label_assignment DROP COLUMN decided_model;` removed -> the_way_down_gives_back_the_version_before_with_every_row
//!   M4 lib.rs: the `(31, ...down.sql)` entry removed from REVERSALS -> the_way_down_gives_back_the_version_before_with_every_row
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn before() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    let mut last = 0;
    for (v, sql) in MIGRATIONS.iter().filter(|(v, _)| *v < 31) { c.execute_batch(sql).unwrap(); last = *v; }
    c.pragma_update(None, "user_version", last).unwrap();
    c
}
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn all(c: &Connection, sql: &str) -> Vec<String> { c.prepare(sql).unwrap().query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect() }
const NOS: &str = "SELECT quote(message_id) || '|' || quote(label_id) || '|' || quote(decided_by) || '|' || quote(decided_at_ms) FROM ai_label_no ORDER BY message_id, label_id";
const FILINGS: &str = "SELECT quote(label_id) || '|' || quote(message_id) || '|' || quote(updated_at_ms) || '|' || quote(decided_by) || '|' || quote(rule_id) || '|' || quote(probability) || '|' || quote(decided_at_ms) || '|' || quote(decided_entry_id) || '|' || quote(flow_card) || '|' || quote(flow_chain_id) FROM label_assignment ORDER BY label_id, message_id";

/// one account, one thread, three messages, two labels; a "no" of Laya and one of Jev; a filing by Laya (every column set) and one by the owner
fn seed(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    for (id, uid) in [("m1", 1), ("m2", 2), ("m3", 3)] {
        c.execute(&format!("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) VALUES ('{id}', 't1', 'acc', '{uid}', 'S', 's@x.example', 's', {uid}, 'INBOX')"), []).unwrap();
    }
    c.execute("INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES ('l1', 'Check label one', NULL, 0)", []).unwrap();
    c.execute("INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES ('l2', 'Check label two', NULL, 0)", []).unwrap();
    c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m1', 'l1', 'laya', 5)", []).unwrap();
    c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m2', 'l2', 'jev', 6)", []).unwrap();
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, rule_id, probability, decided_at_ms, decided_entry_id, flow_card, flow_chain_id) VALUES ('l1', 'm2', 9, 'laya', NULL, 0.91, 8, 'labels/model', 'ai', NULL)", []).unwrap();
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by) VALUES ('l2', 'm3', 10, 'owner')", []).unwrap();
}

/// The tables of the database without the ones a LATER migration adds (0033: notch_alerts, the notch's alert settings): this test judges its own migration.
fn earlier_tables(c: &rusqlite::Connection) -> Vec<String> { table_names(c).unwrap().into_iter().filter(|t| t != "notch_alerts").collect() }

#[test]
fn a_database_with_laya_and_jev_rows_migrates_up_with_every_row_and_two_new_columns() {
    let mut c = before();
    seed(&c);
    let (nos, filings, tables) = (all(&c, NOS), all(&c, FILINGS), table_names(&c).unwrap());
    let applied = migrate(&mut c).unwrap();
    assert_eq!(applied.to_version, MIGRATIONS.last().unwrap().0);
    assert_eq!(earlier_tables(&c), tables, "no table added or removed (columns only)");
    assert_eq!((all(&c, NOS), all(&c, FILINGS)), (nos, filings), "every row, every old column as it was");
    assert_eq!(n(&c, "SELECT count(*) FROM ai_label_no WHERE decided_model IS NOT NULL") + n(&c, "SELECT count(*) FROM label_assignment WHERE decided_model IS NOT NULL"), 0,
               "an old row names no model: it reads as before (Laya stays Laya)");
    // an Apple judgement: the on-device decider the CHECKs allow, the model named beside it
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, probability, decided_at_ms, decided_entry_id, flow_card, decided_model) VALUES ('l1', 'm3', 11, 'laya', 1.0, 11, 'labels/model', 'ai', 'apple')", []).expect("a filing by Apple's model");
    c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms, decided_model) VALUES ('m3', 'l2', 'laya', 11, 'apple')", []).expect("a no by Apple's model");
    assert!(c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m3', 'l1', 'apple', 11)", []).is_err(), "the CHECKs are as they were: 'apple' is a model, not a decider");
    assert_eq!(n(&c, "SELECT count(*) FROM label_assignment WHERE decided_by = 'laya' AND decided_model = 'apple'"), 1);
}

#[test]
fn the_way_down_gives_back_the_version_before_with_every_row() {
    let mut c = before();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    seed(&c);
    let (nos, filings) = (all(&c, NOS), all(&c, FILINGS));
    migrate(&mut c).unwrap();
    c.execute("INSERT INTO label_assignment (label_id, message_id, updated_at_ms, decided_by, decided_model) VALUES ('l1', 'm1', 12, 'laya', 'apple')", []).unwrap();
    let back = revert_to(&mut c, 30).unwrap();
    assert_eq!(back.to_version, 30);
    assert_eq!((table_names(&c).unwrap(), schema_hash(&c).unwrap()), (tables, hash), "the schema is the one before, byte for byte");
    assert_eq!(all(&c, NOS), nos, "every no comes back");
    let mut want = filings.clone();
    want.insert(0, "'l1'|'m1'|12|'laya'|NULL|NULL|NULL|NULL|NULL|NULL".to_string());
    assert_eq!(all(&c, FILINGS), want, "every filing comes back; Apple's reads as Laya's");
    migrate(&mut c).unwrap();
    assert_eq!((all(&c, NOS), all(&c, FILINGS)), (nos, want), "up again: nothing lost");
}
