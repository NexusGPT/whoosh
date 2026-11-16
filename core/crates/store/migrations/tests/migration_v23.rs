//! Migration 0023 (lane reply-headers, SPEC A144): `message.rfc_message_id` and `message.rfc_references_json`, each mail's threading headers
//! kept on this Mac (NULL = not known yet), and the partial index `message_headers_unknown` the fill-in pass reads. Not run by this lane (no
//! cargo here): the lead runs it (`cargo test` in core/crates/store/migrations, its own workspace).
//! RED-FIRST (one-line broken copy per test):
//!   M1 0023_thread_headers.sql: `rfc_references_json TEXT` with no CHECK                       -> a_copy_of_a_version_22_database_migrates_and_every_row_reads_unknown
//!   M2 0023_thread_headers.sql: the index's `WHERE rfc_message_id IS NULL` dropped              -> the_index_holds_only_the_unknown_rows
//!   M3 0023_thread_headers.down.sql: the line `ALTER TABLE message DROP COLUMN rfc_message_id;` dropped -> the_reversal_gives_back_0022
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
fn a_copy_of_a_version_22_database_migrates_and_every_row_reads_unknown() {
    let dir = std::env::temp_dir().join(format!("whoosh-m23-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let (before_tables, before_cols) = {
        let src = at(22);
        mail(&src);
        let t = (table_names(&src).unwrap(), cols(&src, "message"));
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
        t
    };
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (22, MIGRATIONS.last().unwrap().0));
    assert_eq!(earlier_tables(&c), before_tables, "no table added or removed");
    let mut want = before_cols.clone();
    want.extend(["rfc_message_id", "rfc_references_json"].map(String::from));
    // 0027 (lane real-replyto) then adds reply_to_json: this test migrates to the LATEST version, so it is named here, after the two of 0023
    want.push("reply_to_json".to_string());
    assert_eq!(cols(&c, "message"), want, "message gains exactly rfc_message_id and rfc_references_json (0023), then reply_to_json (0027), at the end");
    assert_eq!(n(&c, "SELECT count(*) FROM message"), 2, "every row kept");
    assert_eq!(n(&c, "SELECT count(*) FROM message WHERE rfc_message_id IS NULL AND rfc_references_json IS NULL"), 2, "an old row's headers are not known yet");
    c.execute("UPDATE message SET rfc_message_id = '<m1@x.example>', rfc_references_json = '[\"<root@x.example>\"]' WHERE id = 'm1'", []).expect("a JSON chain is kept");
    c.execute("UPDATE message SET rfc_message_id = '', rfc_references_json = NULL WHERE id = 'm2'", []).expect("the not-answered mark, no chain");
    assert!(c.execute("UPDATE message SET rfc_references_json = 'not json' WHERE id = 'm2'", []).is_err(), "the chain is JSON when it is there");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 22, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_index_holds_only_the_unknown_rows() {
    let mut c = at(22);
    mail(&c);
    migrate(&mut c).unwrap();
    // which index the planner picks is SQLite's choice (no ANALYZE statistics): the test reads the index's own definition
    assert_eq!(n(&c, "SELECT count(*) FROM sqlite_master WHERE type = 'index' AND name = 'message_headers_unknown' AND sql LIKE '%WHERE rfc_message_id IS NULL%'"), 1);
}

#[test]
fn the_reversal_gives_back_0022() {
    let mut c = at(22);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    mail(&c);
    migrate(&mut c).unwrap();
    c.execute("UPDATE message SET rfc_message_id = '<m1@x.example>', rfc_references_json = '[]' WHERE id = 'm1'", []).unwrap();
    let back = revert_to(&mut c, 22).unwrap();
    assert_eq!(back.to_version, 22);
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is 0022's, byte for byte");
    assert_eq!(n(&c, "SELECT count(*) FROM message"), 2, "the mail stays; only its headers are lost");
}
