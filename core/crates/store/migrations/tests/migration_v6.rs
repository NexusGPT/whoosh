//! Migration 0006 (contracts delta 11): the labels the owner removed from a thread, and the reversal to 0005.
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, MIGRATIONS};

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    migrate(&mut c).unwrap();
    c
}

#[test]
fn the_removed_labels_are_a_json_array_defaulting_to_empty() {
    let c = fresh();
    c.execute_batch(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('a','o@x.test','imap','A','#336699',0,'manual','svc' || char(31) || 'a',0);
         INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t','a','s','[]',1);",
    ).unwrap();
    assert_eq!(c.query_row("SELECT labels_removed_json FROM thread WHERE id='t'", [], |r| r.get::<_, String>(0)).unwrap(), "[]");
    assert!(c.execute("UPDATE thread SET labels_removed_json = 'nope' WHERE id = 't'", []).is_err());
}

#[test]
fn the_reversal_gives_back_exactly_0005s_thread() {
    let v5 = {
        let c = Connection::open_in_memory().unwrap();
        for (_, sql) in &MIGRATIONS[..5] { c.execute_batch(sql).unwrap(); }
        c
    };
    let cols = |c: &Connection| -> Vec<String> { c.prepare("PRAGMA table_info(thread)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() };
    let mut c = fresh();
    let applied = revert_to(&mut c, 5).unwrap();
    assert_eq!(applied.to_version, 5);
    assert_eq!(cols(&c), cols(&v5));
}
