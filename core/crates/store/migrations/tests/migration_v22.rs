//! Migration 0022 (lane flow3-core, SPEC A140 part 3): `ai_label_no`, the label AI's "no" kept per (message, label). ADDITIVE ONLY.
//! MUTANTS (each must turn the test named red; none was run, no cargo in this lane):
//!   N1 the key is message_id alone (one "no" per message)          -> a_no_is_one_row_per_message_and_label_named_by_its_model
//!   N2 the decided_by CHECK allows any word                        -> a_no_is_one_row_per_message_and_label_named_by_its_model
//!   N3 no ON DELETE CASCADE on message_id                          -> a_deleted_message_or_label_takes_its_rows
//!   N4 the reversal keeps the table                                -> the_reversal_gives_back_0021
//!   N5 the migration alters label_assignment                       -> a_copy_of_a_version_21_database_migrates_and_keeps_every_row
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }

#[test]
fn a_no_is_one_row_per_message_and_label_named_by_its_model() {
    let mut c = at(21);
    migrate(&mut c).unwrap();
    // the parents are checked by the foreign keys (a_deleted_message_or_label_takes_its_rows); this test reads the row's own shape
    c.pragma_update(None, "foreign_keys", false).unwrap();
    c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m1', 'l1', 'laya', 5)", []).unwrap();
    c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m1', 'l2', 'jev', 5)", []).expect("another label of the same message");
    assert!(c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m1', 'l1', 'jev', 6)", []).is_err(), "one row per message and label");
    assert!(c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m2', 'l1', 'rule:x', 6)", []).is_err(), "only a model says no");
    assert!(c.execute("INSERT INTO ai_label_no (message_id, label_id, decided_by, decided_at_ms) VALUES ('m2', 'l1', 'default', 6)", []).is_err(), "only a model says no");
    assert_eq!(n(&c, "SELECT count(*) FROM ai_label_no"), 2);
}

#[test]
fn a_deleted_message_or_label_takes_its_rows() {
    let mut c = at(21);
    migrate(&mut c).unwrap();
    let fks: Vec<(String, String, String)> = c
        .prepare("SELECT \"table\", \"from\", on_delete FROM pragma_foreign_key_list('ai_label_no') ORDER BY \"from\"").unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(fks, vec![("label".to_string(), "label_id".to_string(), "CASCADE".to_string()), ("message".to_string(), "message_id".to_string(), "CASCADE".to_string())]);
}

#[test]
fn a_copy_of_a_version_21_database_migrates_and_keeps_every_row() {
    let dir = std::env::temp_dir().join(format!("whoosh-m22-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let (before_tables, before_hash) = {
        let src = at(21);
        let t = (table_names(&src).unwrap(), schema_hash(&src).unwrap());
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
        t
    };
    std::fs::copy(&orig, &copy).unwrap();
    let cols = |c: &Connection, t: &str| n(c, &format!("SELECT count(*) FROM pragma_table_info('{t}')"));
    let before_cols = { let c = Connection::open(&copy).unwrap(); (cols(&c, "label_assignment"), cols(&c, "flow_rejection"), cols(&c, "label")) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (21, MIGRATIONS.last().unwrap().0));
    let mut want = before_tables.clone();
    want.push("ai_label_no".to_string());
    // 0033 (the notch's alert settings, owner's picks of 14 Nov 2026) adds one table too
    if MIGRATIONS.iter().any(|m| m.0 >= 33) { want.push("notch_alerts".to_string()); }
    want.sort();
    let mut got = table_names(&c).unwrap();
    got.sort();
    assert_eq!(got, want, "exactly one table added");
    // 0031 (which model decided) adds one column to label_assignment: a LATER migration's change, not this one's (this line compared the counts whole
    // and stayed red from 0031 on: seen on 14 Nov 2026 on the code before the notch work, 124 passed, 1 failed)
    let later = if MIGRATIONS.iter().any(|m| m.0 >= 31) { 1 } else { 0 };
    assert_eq!((cols(&c, "label_assignment") - later, cols(&c, "flow_rejection"), cols(&c, "label")), before_cols, "no existing table changes");
    assert_eq!(n(&c, "SELECT count(*) FROM ai_label_no"), 0, "an existing store holds no kept no");
    assert_ne!(schema_hash(&c).unwrap(), before_hash);
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 21, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_migration_runs_twice_on_a_database_that_has_it() {
    let mut c = at(21);
    migrate(&mut c).unwrap();
    c.execute_batch(MIGRATIONS[21].1).expect("idempotent");
    assert!(table_names(&c).unwrap().contains(&"ai_label_no".to_string()));
}

#[test]
fn the_reversal_gives_back_0021() {
    let mut c = at(21);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"ai_label_no".to_string()));
    revert_to(&mut c, 21).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is 0021's, byte for byte");
}
