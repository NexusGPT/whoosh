//! Migration 0021 (attach-engine round 4): the FTS5 index over `source_chunk`. ADDITIVE ONLY; the index is derived data.
//! MUTANTS (each must turn the test named red):
//!   M1 `m21-down-drops-source-chunk`  the reversal also drops source_chunk                          -> the_reversal_gives_back_0020_and_loses_no_excerpt
//!   M2 `m21-no-delete-trigger`        a deleted excerpt stays in the index                          -> the_index_follows_every_insert_delete_and_text_change
//!   M3 `m21-no-rebuild`               the excerpts already stored are not indexed                   -> a_copy_of_a_version_20_database_migrates_and_indexes_what_it_holds
//!   M4 `m21-not-idempotent`           CREATE VIRTUAL TABLE without IF NOT EXISTS                    -> the_migration_runs_twice_on_a_database_that_has_it
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
fn hits(c: &Connection, word: &str) -> i64 { n(c, &format!("SELECT count(*) FROM source_chunk_fts WHERE source_chunk_fts MATCH '\"{word}\"'")) }
fn put(c: &Connection, item: &str, title: &str, text: &str) {
    c.execute("INSERT INTO source_chunk (source_key, item_key, chunk_no, date_ms, title, text) VALUES (?1, ?2, 0, 1, ?3, ?4)", rusqlite::params![LOCAL, item, title, text]).unwrap();
}

fn v20_with_rows() -> Connection {
    let c = at(20);
    put(&c, "acme.md", "Acme", "the contract price is 42 euros");
    put(&c, "other.md", "Other", "the office is open at nine");
    c
}

#[test]
fn a_copy_of_a_version_20_database_migrates_and_indexes_what_it_holds() {
    let dir = std::env::temp_dir().join(format!("whoosh-m21-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let before = {
        let src = v20_with_rows();
        let t = table_names(&src).unwrap();
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
        t
    };
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (20, MIGRATIONS.last().unwrap().0));
    // 0022 (lane flow3-core) is the next migration: 0021 is the 21st entry, and 0022's table is in the run too (as migration_v20.rs does for 0021)
    assert_eq!(MIGRATIONS[20].0, 21);
    let mut want = before.clone();
    want.extend(["source_chunk_fts", "source_chunk_fts_config", "source_chunk_fts_data", "source_chunk_fts_idx"].map(String::from));
    if MIGRATIONS.iter().any(|m| m.0 >= 22) { want.push("ai_label_no".to_string()); }
    // 0033 (the notch's alert settings, owner's picks of 14 Nov 2026) adds one table too
    if MIGRATIONS.iter().any(|m| m.0 >= 33) { want.push("notch_alerts".to_string()); }
    want.sort();
    let mut got = table_names(&c).unwrap();
    got.sort();
    assert_eq!(got, want, "the index and its shadow tables are added, every earlier table is still there");
    assert_eq!(n(&c, "SELECT count(*) FROM source_chunk"), 2, "no excerpt moved or lost");
    assert_eq!((hits(&c, "contract"), hits(&c, "office"), hits(&c, "nothing")), (1, 1, 0), "what the database held is in the index");
    let cols: Vec<String> = c.prepare("PRAGMA table_info(source_chunk)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect();
    assert_eq!(cols, ["id", "source_key", "item_key", "chunk_no", "date_ms", "title", "text", "embedded_at_ms"], "source_chunk is as 0016 made it");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 20, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_index_follows_every_insert_delete_and_text_change() {
    let mut c = at(20);
    migrate(&mut c).unwrap();
    put(&c, "a.md", "Alpha", "the contract price");
    put(&c, "b.md", "Beta", "the office hours");
    assert_eq!((hits(&c, "contract"), hits(&c, "alpha"), hits(&c, "office")), (1, 1, 1), "title and text are both indexed");
    c.execute("UPDATE source_chunk SET text = 'the invoice total' WHERE item_key = 'a.md'", []).unwrap();
    assert_eq!((hits(&c, "contract"), hits(&c, "invoice")), (0, 1), "a changed text is indexed again, the old words are gone");
    c.execute("DELETE FROM source_chunk WHERE item_key = 'a.md'", []).unwrap();
    assert_eq!((hits(&c, "invoice"), hits(&c, "alpha")), (0, 0), "a deleted excerpt leaves the index");
    // a removed source deletes its excerpts through 0016's trigger, which fires these
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES (?1, 'local_folder', 1)", [LOCAL]).unwrap();
    assert_eq!(hits(&c, "office"), 1);
    c.execute("DELETE FROM reply_source WHERE source_key = ?1", [LOCAL]).unwrap();
    assert_eq!((n(&c, "SELECT count(*) FROM source_chunk"), hits(&c, "office")), (0, 0), "a removed source takes its index rows too");
    // the default splitter folds Latin diacritics: café and cafe are one word
    put(&c, "c.md", "Menu", "un caf\u{e9} noir");
    assert_eq!((hits(&c, "caf\u{e9}"), hits(&c, "cafe")), (1, 1), "café and cafe are one word to the index");
}

#[test]
fn the_migration_runs_twice_on_a_database_that_has_it() {
    let mut c = v20_with_rows();
    migrate(&mut c).unwrap();
    c.execute_batch(MIGRATIONS[20].1).expect("idempotent");
    assert_eq!((n(&c, "SELECT count(*) FROM source_chunk"), hits(&c, "contract"), hits(&c, "office")), (2, 1, 1), "run again: nothing lost, nothing indexed twice");
}

#[test]
fn the_reversal_gives_back_0020_and_loses_no_excerpt() {
    let mut c = v20_with_rows();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"source_chunk_fts".to_string()));
    revert_to(&mut c, 20).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables, "the index and its shadow tables are gone, source_chunk and every other table stay");
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is 0020's, byte for byte");
    assert_eq!(n(&c, "SELECT count(*) FROM source_chunk"), 2, "the excerpts are still there");
}
