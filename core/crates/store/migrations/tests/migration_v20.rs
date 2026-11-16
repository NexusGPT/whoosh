//! Migration 0020 (attach-engine): table `source_item`, the stamp of each item the reply sources' walks read. ADDITIVE ONLY.
//! MUTANTS (each must turn the test named red; logs in the lane's output folder):
//!   M1 `m20-down-drops-source-chunk`  the reversal also drops source_chunk                          -> the_reversal_gives_back_0019_and_touches_nothing_else
//!   M2 `m20-changes-an-old-table`     the migration also adds a column to source_chunk              -> a_copy_of_a_version_19_database_migrates_and_keeps_every_table_and_row
//!   M3 `m20-no-trigger`               a removed source keeps its items                              -> a_removed_source_takes_its_items_and_an_item_is_a_local_or_calendar_one
//!   M4 `m20-not-idempotent`           CREATE TABLE without IF NOT EXISTS                            -> the_migration_runs_twice_on_a_database_that_has_it
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
const LOCAL: &str = "local\u{1f}/Users/o/Contrats";

/// a version-19 database with a mail, a source and a chunk of it
fn v19_with_rows() -> Connection {
    let c = at(19);
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder, body_text) VALUES ('m1', 't1', 'acc', '7', 'S', 's@x.example', 's', 1, 'INBOX', 'the body')", []).unwrap();
    c.execute("INSERT INTO source_chunk (source_key, item_key, chunk_no, date_ms, title, text) VALUES (?1, 'acme.md', 0, 5, 'Acme', 'the price is 42')", [LOCAL]).unwrap();
    c
}
#[test]
fn a_copy_of_a_version_19_database_migrates_and_keeps_every_table_and_row() {
    let dir = tempfile_dir();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let before_tables;
    let before_hash;
    {
        let src = v19_with_rows();
        before_tables = table_names(&src).unwrap();
        before_hash = schema_hash(&src).unwrap();
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (19, MIGRATIONS.last().unwrap().0));
    let mut want = before_tables.clone();
    want.push("source_item".to_string());
    // 0021 (the full-text index over source_chunk) is the next migration: its table and shadow tables are in the run too
    // (lane flow3-core: once when 0021 is in the run, not once per later migration; with 0022 present the per-migration loop added them twice)
    if MIGRATIONS.iter().any(|m| m.0 >= 21) { for t in ["source_chunk_fts", "source_chunk_fts_config", "source_chunk_fts_data", "source_chunk_fts_idx"] { want.push(t.to_string()); } }
    // 0022 (the AI's kept "no", lane flow3-core) runs after it too
    if MIGRATIONS.iter().any(|m| m.0 >= 22) { want.push("ai_label_no".to_string()); }
    // 0033 (the notch's alert settings, owner's picks of 14 Nov 2026) adds one table too
    if MIGRATIONS.iter().any(|m| m.0 >= 33) { want.push("notch_alerts".to_string()); }
    want.sort();
    let mut got = table_names(&c).unwrap();
    got.sort();
    assert_eq!(got, want, "exactly the tables 0020 and later add, every earlier table still there");
    assert_eq!((n(&c, "SELECT count(*) FROM message"), n(&c, "SELECT count(*) FROM source_chunk"), n(&c, "SELECT count(*) FROM source_item")), (1, 1, 0), "no row moved, none added");
    let text: String = c.query_row("SELECT text FROM source_chunk WHERE item_key = 'acme.md'", [], |r| r.get(0)).unwrap();
    assert_eq!(text, "the price is 42");
    let cols: Vec<String> = c.prepare("PRAGMA table_info(source_chunk)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect();
    assert_eq!(cols, ["id", "source_key", "item_key", "chunk_no", "date_ms", "title", "text", "embedded_at_ms"], "source_chunk is as 0016 made it");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 19, "the original is untouched");
    assert_ne!(schema_hash(&c).unwrap(), before_hash);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_migration_runs_twice_on_a_database_that_has_it() {
    let mut c = at(19);
    migrate(&mut c).unwrap();
    c.execute("INSERT INTO source_item (source_key, item_key, version, excerpt_count) VALUES (?1, 'acme.md', '10:5', 1)", [LOCAL]).unwrap();
    // the 0020 file again, on a database that already has its table and trigger: nothing fails, nothing is lost
    c.execute_batch(MIGRATIONS[19].1).expect("idempotent");
    assert_eq!(n(&c, "SELECT count(*) FROM source_item"), 1);
}

#[test]
fn a_removed_source_takes_its_items_and_an_item_is_a_local_or_calendar_one() {
    let mut c = at(19);
    migrate(&mut c).unwrap();
    let ins = |key: &str, item: &str, count: i64| c.execute("INSERT INTO source_item (source_key, item_key, version, excerpt_count) VALUES (?1, ?2, 'v', ?3)", rusqlite::params![key, item, count]);
    assert!(ins("local\u{1f}/x", "a", 0).is_ok() && ins("calendar\u{1f}cal-1", "e1@5", 1).is_ok());
    assert!(ins("mail\u{1f}acc\u{1f}INBOX", "a", 0).is_err(), "a mail folder has no item stamps here");
    assert!(ins("local\u{1f}/x", " ", 0).is_err(), "an item has a key");
    assert!(ins("local\u{1f}/y", "a", -1).is_err(), "a count is not negative");
    assert!(ins("local\u{1f}/x", "a", 0).is_err(), "one stamp per item");
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES ('local\u{1f}/x', 'local_folder', 1)", []).unwrap();
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES ('calendar\u{1f}cal-1', 'calendar', 1)", []).unwrap();
    c.execute("DELETE FROM reply_source WHERE source_key = 'local\u{1f}/x'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM source_item WHERE source_key = 'local\u{1f}/x'"), 0, "a removed source takes its items");
    assert_eq!(n(&c, "SELECT count(*) FROM source_item WHERE source_key = 'calendar\u{1f}cal-1'"), 1, "another source's items stay");
}

#[test]
fn the_reversal_gives_back_0019_and_touches_nothing_else() {
    let mut c = v19_with_rows();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"source_item".to_string()));
    revert_to(&mut c, 19).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables, "source_chunk and every other table are still there");
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is 0019's, byte for byte");
    assert_eq!((n(&c, "SELECT count(*) FROM source_chunk"), n(&c, "SELECT count(*) FROM message")), (1, 1), "the rows are still there");
}

fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("whoosh-m20-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
