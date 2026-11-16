//! Migration 0030 (lane fix-aicore): two columns on `thread`, nothing else: `summary_text` (the reader's summary the writing model made) and
//! `summary_of` (the newest message it was made from). "The version before" is every migration below 30 (0029 is another lane's: whether it has
//! landed or not, the reversal of 0030 alone leaves version 29, and the schema is the one before 0030 byte for byte).
//! Not run by this lane (no cargo here): the lead runs `cargo test` in core/crates/store/migrations.
//! Broken copies (one line each):
//!   M1 0030_thread_summary.sql: `ALTER TABLE thread ADD COLUMN summary_of TEXT;` removed           -> a_copy_of_the_version_before_migrates_keeps_every_thread_and_adds_the_two_columns
//!   M2 0030_thread_summary.sql: `summary_text TEXT` -> `summary_text TEXT NOT NULL DEFAULT ''`        -> a_copy_of_the_version_before_migrates_keeps_every_thread_and_adds_the_two_columns
//!   M3 0030_thread_summary.down.sql: the line `ALTER TABLE thread DROP COLUMN summary_of;` removed  -> the_reversal_gives_back_the_version_before
//!   M4 lib.rs: the `(30, ...down.sql)` entry removed from REVERSALS                                 -> the_reversal_gives_back_the_version_before
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn before() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    let mut last = 0;
    for (v, sql) in MIGRATIONS.iter().filter(|(v, _)| *v < 30) { c.execute_batch(sql).unwrap(); last = *v; }
    c.pragma_update(None, "user_version", last).unwrap();
    c
}
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn s(c: &Connection, sql: &str) -> String { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn cols(c: &Connection, t: &str) -> Vec<String> {
    c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect()
}
/// one account, one thread with every old column set, one message in it
fn mail(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, remote_id, subject, participants_json, last_message_at_ms, labels_removed_json) VALUES ('t1', 'acc', 'r1', 'Quote', '[\"ann@x.test\"]', 7, '[\"l1\"]')", []).unwrap();
}
const OLD: &str = "SELECT quote(id) || '|' || quote(account_id) || '|' || quote(remote_id) || '|' || quote(subject) || '|' || quote(participants_json) || '|' || quote(last_message_at_ms) || '|' || quote(labels_removed_json) FROM thread";

/// The tables of the database without the ones a LATER migration adds (0033: notch_alerts, the notch's alert settings): this test judges its own migration.
fn earlier_tables(c: &rusqlite::Connection) -> Vec<String> { table_names(c).unwrap().into_iter().filter(|t| t != "notch_alerts").collect() }

#[test]
fn a_copy_of_the_version_before_migrates_keeps_every_thread_and_adds_the_two_columns() {
    let mut c = before();
    mail(&c);
    let (tables, thread, message, row, version_before) = (table_names(&c).unwrap(), cols(&c, "thread"), cols(&c, "message"), s(&c, OLD), n(&c, "PRAGMA user_version"));
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (version_before, MIGRATIONS.last().unwrap().0));
    assert_eq!(earlier_tables(&c), tables, "no table added or removed (columns only)");
    let mut want = thread.clone();
    want.extend(["summary_text".to_string(), "summary_of".to_string()]);
    assert_eq!(cols(&c, "thread"), want, "thread gains exactly summary_text and summary_of, at the end");
    assert_eq!(cols(&c, "message"), message, "message is untouched by 0030");
    assert_eq!(s(&c, OLD), row, "every old column of the stored thread is as it was");
    assert_eq!(n(&c, "SELECT summary_text IS NULL AND summary_of IS NULL FROM thread WHERE id = 't1'"), 1, "an old thread has no summary yet");
    c.execute("UPDATE thread SET summary_text = 'A quote for brochures.', summary_of = 'acc:INBOX:1' WHERE id = 't1'", []).unwrap();
    assert_eq!(s(&c, "SELECT summary_text FROM thread WHERE id = 't1'"), "A quote for brochures.");
    c.execute("UPDATE thread SET summary_text = NULL, summary_of = NULL WHERE id = 't1'", []).expect("a summary can be cleared");
}

#[test]
fn the_reversal_gives_back_the_version_before() {
    let mut c = before();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    mail(&c);
    let row = s(&c, OLD);
    migrate(&mut c).unwrap();
    c.execute("UPDATE thread SET summary_text = 'x', summary_of = 'm' WHERE id = 't1'", []).unwrap();
    let back = revert_to(&mut c, 29).unwrap();
    assert_eq!(back.to_version, 29, "undoing 0030 alone leaves version 29");
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is the version before's, byte for byte");
    assert_eq!(s(&c, OLD), row, "the thread stays; only its summary is lost");
}
