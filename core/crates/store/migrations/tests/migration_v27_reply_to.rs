//! Migration 0027 (lane real-replyto): `message.reply_to_json`, each mail's Reply-To kept on this Mac (NULL = not read yet). The migration
//! files are numbered 0027 and the tree's migration before it is 0023 (0024 to 0026 belong to other lanes, merged later), so the tests build
//! "the version before" as every migration below 27. Not run by this lane (no cargo here): the lead runs `cargo test` in core/crates/store/migrations.
//! RED-FIRST (one-line broken copy per test):
//!   M1 0027_reply_to.sql: `reply_to_json TEXT CHECK (...)` -> `reply_to_json INTEGER`                       -> a_copy_of_the_version_before_migrates_and_every_row_reads_unknown
//!   M2 0027_reply_to.sql: drop the CHECK                                                                   -> a_copy_of_the_version_before_migrates_and_every_row_reads_unknown
//!   M3 0027_reply_to.down.sql: drop the line `ALTER TABLE message DROP COLUMN reply_to_json;`              -> the_reversal_gives_back_the_version_before
//!   M4 lib.rs: the `(27, ...down.sql)` entry removed from REVERSALS                                         -> the_reversal_gives_back_the_version_before
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn before() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    let mut last = 0;
    for (v, sql) in MIGRATIONS.iter().filter(|(v, _)| *v < 27) { c.execute_batch(sql).unwrap(); last = *v; }
    c.pragma_update(None, "user_version", last).unwrap();
    c
}
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn cols(c: &Connection, t: &str) -> Vec<String> {
    c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect()
}
fn mail(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    for (id, uid) in [("m1", 7), ("m2", 8)] {
        c.execute(&format!("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) VALUES ('{id}', 't1', 'acc', '{uid}', 'S', 's@x.example', 's', {uid}, 'INBOX')"), []).unwrap();
    }
}

/// The tables of the database without the ones a LATER migration adds (0033: notch_alerts, the notch's alert settings): this test judges its own migration.
fn earlier_tables(c: &rusqlite::Connection) -> Vec<String> { table_names(c).unwrap().into_iter().filter(|t| t != "notch_alerts").collect() }

#[test]
fn a_copy_of_the_version_before_migrates_and_every_row_reads_unknown() {
    let dir = std::env::temp_dir().join(format!("whoosh-m27-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let (before_tables, before_cols, before_version) = {
        let src = before();
        mail(&src);
        let t = (table_names(&src).unwrap(), cols(&src, "message"), n(&src, "PRAGMA user_version"));
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
        t
    };
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (before_version, MIGRATIONS.last().unwrap().0));
    assert_eq!(earlier_tables(&c), before_tables, "no table added or removed");
    let mut want = before_cols.clone();
    want.push("reply_to_json".to_string());
    assert_eq!(cols(&c, "message"), want, "message gains exactly the one column, at the end");
    assert_eq!(n(&c, "SELECT count(*) FROM message"), 2, "every row kept");
    assert_eq!(n(&c, "SELECT count(*) FROM message WHERE reply_to_json IS NULL"), 2, "an old row's Reply-To is not read yet");
    c.execute("UPDATE message SET reply_to_json = '[{\"name\":\"L\",\"address\":\"l@x.example\"}]' WHERE id = 'm1'", []).expect("a JSON list is kept");
    c.execute("UPDATE message SET reply_to_json = '[]' WHERE id = 'm2'", []).expect("read, and none");
    assert!(c.execute("UPDATE message SET reply_to_json = 'not json' WHERE id = 'm2'", []).is_err(), "the column is JSON when it is there");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), before_version, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_the_version_before() {
    let mut c = before();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    mail(&c);
    migrate(&mut c).unwrap();
    c.execute("UPDATE message SET reply_to_json = '[]' WHERE id = 'm1'", []).unwrap();
    let back = revert_to(&mut c, 26).unwrap();
    assert_eq!(back.to_version, 26, "undoing 0027 leaves the version one below it");
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is the version before's, byte for byte");
    assert_eq!(n(&c, "SELECT count(*) FROM message"), 2, "the mail stays; only its Reply-To is lost");
}
