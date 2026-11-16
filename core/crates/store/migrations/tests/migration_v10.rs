//! Migration 0010 (contracts delta 18): imported mail names its source. No new table; three columns on `message`, two triggers, one index.
//! MUTANTS (the lead runs each at the build word; the test named must go red):
//!   M1 `source` defaults to 'imported' (every synced row would read as imported)                -> a_row_that_existed_before_is_synced_with_no_import_mark
//!   M2 the backfill skips the 'imp:' rows (an earlier import reads as synced mail)               -> earlier_import_rows_are_marked_imported_once
//!   M3 the backfill leaves import_source_id NULL for 'imp:' rows                                  -> earlier_import_rows_are_marked_imported_once
//!   M4 the insert trigger is dropped                                                              -> the_import_mark_is_whole_in_the_store_too
//!   M5 the update trigger is dropped                                                              -> the_import_mark_is_whole_in_the_store_too
//!   M6 the trigger lets a synced row carry an import source                                       -> the_import_mark_is_whole_in_the_store_too
//!   M7 the CHECK on `source` is dropped (a third word is accepted)                                -> the_import_mark_is_whole_in_the_store_too
//!   M8 the partial index is dropped or indexes synced rows                                        -> a_source_is_counted_from_its_rows_with_no_table_of_its_own
//!   M9 a table `import_source` is created (the count would be stored a second time)              -> a_source_is_counted_from_its_rows_with_no_table_of_its_own
//!   M10 the reversal leaves one of the three columns                                              -> the_reversal_gives_back_0009_and_keeps_the_messages
//!   M11 the reversal keeps a trigger                                                              -> the_reversal_gives_back_0009_and_keeps_the_messages
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, table_names, MIGRATIONS};

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
fn account(c: &Connection) {
    c.execute("INSERT OR IGNORE INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'google', 'W', '#336699', 0, 'oauth', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
}
fn message(c: &Connection, id: &str, received: i64) {
    account(c);
    c.execute("INSERT INTO thread (id, account_id, subject) VALUES (?1, 'acc', 's')", [format!("t-{id}")]).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder) VALUES (?1, ?2, 'acc', 'A', 'a@example.test', 's', ?3, 'INBOX')", params![id, format!("t-{id}"), received]).unwrap();
}
fn insert_with(c: &Connection, id: &str, source: &str, src: Option<&str>, at_ms: Option<i64>) -> rusqlite::Result<usize> {
    account(c);
    c.execute("INSERT OR IGNORE INTO thread (id, account_id, subject) VALUES ('t-x', 'acc', 's')", []).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder, source, import_source_id, imported_at_ms) VALUES (?1, 't-x', 'acc', 'A', 'a@example.test', 's', 1, 'INBOX', ?2, ?3, ?4)", params![id, source, src, at_ms])
}

#[test]
fn a_row_that_existed_before_is_synced_with_no_import_mark() {
    let mut c = at(9);
    message(&c, "synced-1", 100);
    migrate(&mut c).unwrap();
    let row: (String, Option<String>, Option<i64>) = c.query_row("SELECT source, import_source_id, imported_at_ms FROM message WHERE id = 'synced-1'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(row, ("synced".into(), None, None));
}

#[test]
fn earlier_import_rows_are_marked_imported_once() {
    let mut c = at(9);
    message(&c, "imp:acc:<a@x>", 777); message(&c, "plain", 5);
    migrate(&mut c).unwrap();
    let row: (String, Option<String>, Option<i64>) = c.query_row("SELECT source, import_source_id, imported_at_ms FROM message WHERE id = 'imp:acc:<a@x>'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(row, ("imported".into(), Some("earlier-import".into()), Some(777)), "the run time was not kept: the message's own time stands in");
    assert_eq!(one::<String>(&c, "SELECT source FROM message WHERE id = 'plain'"), "synced", "a synced row is untouched");
}

#[test]
fn the_import_mark_is_whole_in_the_store_too() {
    let mut c = at(9);
    migrate(&mut c).unwrap();
    insert_with(&c, "i1", "imported", Some("olm:~/a.olm"), Some(10)).unwrap();
    insert_with(&c, "s1", "synced", None, None).unwrap();
    assert!(refused(insert_with(&c, "i2", "imported", None, Some(10))).contains("names its source"), "imported with no source");
    assert!(refused(insert_with(&c, "i3", "imported", Some("olm:x"), None)).contains("names its source"), "imported with no time");
    assert!(refused(insert_with(&c, "s2", "synced", Some("olm:x"), None)).contains("names its source"), "synced with a source");
    assert!(refused(insert_with(&c, "s3", "synced", None, Some(1))).contains("names its source"), "synced with a time");
    // a third source word: SQLite runs the BEFORE INSERT trigger ahead of the column CHECK, so the trigger's words come first
    assert!(refused(insert_with(&c, "w1", "downloaded", None, None)).contains("names its source"), "a third word");
    assert!(refused(insert_with(&c, "e1", "imported", Some("  "), Some(1))).contains("CHECK"), "a blank source id");
    assert!(refused(c.execute("UPDATE message SET import_source_id = NULL WHERE id = 'i1'", [])).contains("names its source"), "update: strip the source off an imported row");
    assert!(refused(c.execute("UPDATE message SET source = 'synced' WHERE id = 'i1'", [])).contains("names its source"), "update: call an imported row synced");
    c.execute("UPDATE message SET source = 'imported', import_source_id = 'olm:y', imported_at_ms = 2 WHERE id = 's1'", []).unwrap();
}

#[test]
fn a_source_is_counted_from_its_rows_with_no_table_of_its_own() {
    let mut c = at(9);
    migrate(&mut c).unwrap();
    for (i, src, at_ms) in [("a1", "apple-mail:~/Library/Mail", 100), ("a2", "apple-mail:~/Library/Mail", 300), ("o1", "olm:~/a.olm", 50)] { insert_with(&c, i, "imported", Some(src), Some(at_ms)).unwrap(); }
    insert_with(&c, "s9", "synced", None, None).unwrap();
    let (n, last): (i64, i64) = c.query_row("SELECT count(*), max(imported_at_ms) FROM message WHERE import_source_id = 'apple-mail:~/Library/Mail'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((n, last), (2, 300), "imported count and last run, both derived");
    assert!(!table_names(&c).unwrap().iter().any(|t| t.contains("import")), "no table keeps the count a second time");
    let plan: String = c.query_row("EXPLAIN QUERY PLAN SELECT count(*) FROM message WHERE source = 'imported' AND import_source_id = 'olm:~/a.olm'", [], |r| r.get(3)).unwrap();
    assert!(plan.contains("message_by_import_source"), "the per-source count reads the partial index: {plan}");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'message_by_import_source' AND sql LIKE '%WHERE source = ''imported''%'"), 1, "the index holds imported rows only");
}

#[test]
fn the_reversal_gives_back_0009_and_keeps_the_messages() {
    let v9 = at(9);
    let mut c = at(9);
    migrate(&mut c).unwrap();
    insert_with(&c, "i1", "imported", Some("mbox:~/in.mbox"), Some(10)).unwrap();
    let applied = revert_to(&mut c, 9).unwrap();
    assert_eq!(applied.to_version, 9);
    assert_eq!(cols(&c, "message"), cols(&v9, "message"), "the columns are 0009's");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM message WHERE id = 'i1'"), 1, "the imported message stays, as an ordinary row");
    assert_eq!(one::<i64>(&c, "SELECT count(*) FROM sqlite_master WHERE name IN ('message_by_import_source', 'message_import_mark_is_whole_insert', 'message_import_mark_is_whole_update')"), 0, "the index and both triggers go");
}
