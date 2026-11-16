//! Migration 0029 (lane fix-nesting, J2.6): one column, `smart_folder.parent_label_id TEXT REFERENCES label(id) ON DELETE SET NULL`, so a smart folder
//! can sit under a label (0001's `parent_id` has a foreign key to smart_folder(id) and cannot hold a label id). "The version before" is every
//! migration below 29 (0024 to 0026 belong to other lanes), as migration_v28_settings_cmds.rs does.
//! Not run by this lane (no cargo here): the lead runs `cargo test` in core/crates/store/migrations.
//! RED-FIRST (one-line broken copy per test):
//!   M1 0029_smart_folder_label_parent.sql: `REFERENCES label(id)` removed                     -> a_copy_of_the_version_before_migrates_keeps_every_row_and_names_the_new_column (an unknown label id is stored)
//!   M2 0029_smart_folder_label_parent.down.sql: drop the line `ALTER TABLE smart_folder DROP COLUMN parent_label_id;` -> the_reversal_gives_back_the_version_before
//!   M3 lib.rs: the `(29, ...down.sql)` entry removed from REVERSALS                          -> the_reversal_gives_back_the_version_before
//!   M4 0029_smart_folder_label_parent.sql: ` ON DELETE SET NULL` removed                      -> a_deleted_label_sends_its_smart_folders_to_the_top_level
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn before() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    let mut last = 0;
    for (v, sql) in MIGRATIONS.iter().filter(|(v, _)| *v < 29) { c.execute_batch(sql).unwrap(); last = *v; }
    c.pragma_update(None, "user_version", last).unwrap();
    c
}
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn cols(c: &Connection, t: &str) -> Vec<String> {
    c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect()
}
/// the owner's folders: a label, a top-level folder and one inside it
fn folders(c: &Connection) {
    c.execute("INSERT INTO label (id, name, updated_at_ms) VALUES ('work', 'Work', 1)", []).unwrap();
    c.execute("INSERT INTO smart_folder (id, name, rule_words, parent_id, updated_at_ms) VALUES ('sf-a', 'Supplier quotes', 'quote', NULL, 2)", []).unwrap();
    c.execute("INSERT INTO smart_folder (id, name, rule_words, parent_id, updated_at_ms) VALUES ('sf-b', 'Urgent', 'quote urgent', 'sf-a', 3)", []).unwrap();
}
/// every old column of every smart folder, as SQLite quotes them, in id order
fn old_rows(c: &Connection) -> Vec<String> {
    c.prepare("SELECT quote(id) || '|' || quote(name) || '|' || quote(rule_words) || '|' || quote(parent_id) || '|' || quote(updated_at_ms) FROM smart_folder ORDER BY id")
        .unwrap().query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
}

/// The tables of the database without the ones a LATER migration adds (0033: notch_alerts, the notch's alert settings): this test judges its own migration.
fn earlier_tables(c: &rusqlite::Connection) -> Vec<String> { table_names(c).unwrap().into_iter().filter(|t| t != "notch_alerts").collect() }

#[test]
fn a_copy_of_the_version_before_migrates_keeps_every_row_and_names_the_new_column() {
    let dir = std::env::temp_dir().join(format!("whoosh-m29-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let (tables, smart, label, rows, before_version) = {
        let src = before();
        folders(&src);
        let t = (table_names(&src).unwrap(), cols(&src, "smart_folder"), cols(&src, "label"), old_rows(&src), n(&src, "PRAGMA user_version"));
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
        t
    };
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (before_version, MIGRATIONS.last().unwrap().0));
    assert_eq!(earlier_tables(&c), tables, "no table added or removed (one column only)");
    let mut want = smart.clone();
    want.push("parent_label_id".into());
    assert_eq!(cols(&c, "smart_folder"), want, "smart_folder gains exactly parent_label_id, at the end");
    assert_eq!(cols(&c, "label"), label, "label is untouched by 0029");
    assert_eq!(old_rows(&c), rows, "every old column of every stored folder is as it was");
    assert_eq!(n(&c, "SELECT count(*) FROM smart_folder WHERE parent_label_id IS NULL"), 2, "no old folder sits under a label");
    // the new column holds a label's id, and only that
    c.execute("UPDATE smart_folder SET parent_label_id = 'work' WHERE id = 'sf-a'", []).unwrap();
    assert!(c.execute("UPDATE smart_folder SET parent_label_id = 'no-such-label' WHERE id = 'sf-b'", []).is_err(), "a label that does not exist is refused");
    assert!(c.execute("UPDATE smart_folder SET parent_id = 'work' WHERE id = 'sf-b'", []).is_err(), "parent_id still takes a smart folder only");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), before_version, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_the_version_before() {
    let mut c = before();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    let version_before = n(&c, "PRAGMA user_version");
    folders(&c);
    let rows = old_rows(&c);
    migrate(&mut c).unwrap();
    c.execute("UPDATE smart_folder SET parent_label_id = 'work' WHERE id = 'sf-a'", []).unwrap();
    let back = revert_to(&mut c, 28).unwrap();
    assert_eq!(back.to_version, version_before, "undoing 0029 leaves the version 0028 left");
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is the version before's, byte for byte");
    assert_eq!(old_rows(&c), rows, "the folders stay; only the label parent is lost");
}

#[test]
fn a_deleted_label_sends_its_smart_folders_to_the_top_level() {
    let mut c = before();
    folders(&c);
    migrate(&mut c).unwrap();
    c.execute("UPDATE smart_folder SET parent_label_id = 'work' WHERE id = 'sf-a'", []).unwrap();
    c.execute("DELETE FROM label WHERE id = 'work'", []).expect("a label under which a smart folder sits can be deleted");
    assert_eq!(n(&c, "SELECT count(*) FROM smart_folder"), 2, "no smart folder is deleted with the label");
    assert_eq!(n(&c, "SELECT count(*) FROM smart_folder WHERE id = 'sf-a' AND parent_label_id IS NULL AND parent_id IS NULL"), 1, "back to the top level");
    assert_eq!(n(&c, "SELECT count(*) FROM smart_folder WHERE id = 'sf-b' AND parent_id = 'sf-a'"), 1, "a smart folder parent is untouched");
}
