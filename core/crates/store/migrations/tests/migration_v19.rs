//! Migration 0019 (contracts delta 26, body-wire): table `message_prepared`, the mail content engine's result kept with the body.
//! MUTANTS (each must turn the test named red; logs in the body-wire output folder):
//!   M1 `m19-no-cascade`     the row does not go with its message (`ON DELETE CASCADE` dropped)    -> a_copy_of_a_version_18_database_migrates_and_keeps_every_body
//!   M2 `m19-reversal-keeps` the reversal keeps the table                                           -> the_reversal_gives_back_0018
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
fn a_copy_of_a_version_18_database_migrates_and_keeps_every_body() {
    let dir = tempfile_dir();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let src = at(18);
        src.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
        src.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
        for (id, uid, text) in [("m1", 7, "the first old body"), ("m2", 8, "the second old body")] {
            src.execute(&format!("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder, body_text, body_html) VALUES ('{id}', 't1', 'acc', '{uid}', 'S', 's@x.example', 's', 1, 'INBOX', '{text}', '<p>{text}</p>')"), []).unwrap();
        }
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (18, MIGRATIONS.last().unwrap().0));
    assert_eq!(n(&c, "SELECT count(*) FROM message"), 2, "every row kept");
    assert_eq!(n(&c, "SELECT count(*) FROM message_prepared"), 0, "an old body has no prepared form");
    let text: String = c.query_row("SELECT body_text FROM message WHERE id = 'm2'", [], |r| r.get(0)).unwrap();
    assert_eq!(text, "the second old body");
    c.execute("INSERT INTO message_prepared (message_id, prepared) VALUES ('m1', '{\"v\":1}')", []).expect("the fetch writes it");
    assert!(c.execute("INSERT INTO message_prepared (message_id, prepared) VALUES ('nope', '{}')", []).is_err(), "only for a stored message");
    c.execute("DELETE FROM message WHERE id = 'm1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM message_prepared"), 0, "the prepared form goes with its message");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 18);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_0018() {
    let mut c = at(18);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"message_prepared".to_string()));
    revert_to(&mut c, 18).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the table is gone");
}

fn tempfile_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("whoosh-m19-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
