//! Migration 0011 (engine-act): `pending_action`, the server write-backs waiting to be applied. One table, one index, a reversal.
//! MUTANTS (each must turn the test named red):
//!   M1 the kind CHECK is dropped (a local-only kind like 'snooze' is stored)          -> the_table_takes_a_write_back_and_refuses_what_is_not_one
//!   M2 json_valid is dropped from payload_json                                        -> the_table_takes_a_write_back_and_refuses_what_is_not_one
//!   M3 the account reference has no ON DELETE CASCADE                                 -> a_removed_account_takes_its_rows_with_it
//!   M4 the reversal keeps the table                                                   -> the_reversal_gives_back_0010
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn account(c: &Connection) {
    c.execute("INSERT OR IGNORE INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'google', 'W', '#336699', 0, 'oauth', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
}
fn add(c: &Connection, kind: &str, payload: &str) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO pending_action (account_id, message_id, folder, uid, kind, payload_json, created_ms, next_at_ms) VALUES ('acc', 'm1', 'INBOX', 7, ?1, ?2, 1, 1)", params![kind, payload])
}

#[test]
fn the_table_takes_a_write_back_and_refuses_what_is_not_one() {
    let mut c = at(10);
    migrate(&mut c).unwrap();
    account(&c);
    for k in ["set_archived", "set_read", "label", "move", "move_to_folder"] { add(&c, k, "{}").unwrap_or_else(|e| panic!("{k}: {e}")); }
    assert!(add(&c, "snooze", "{}").is_err(), "snooze and pin are local to Whoosh: no server kind");
    assert!(add(&c, "set_read", "not json").is_err(), "the payload is JSON");
    assert!(c.execute("INSERT INTO pending_action (account_id, folder, kind, payload_json, created_ms, next_at_ms) VALUES ('nobody', 'INBOX', 'set_read', '{}', 1, 1)", []).is_err(), "an unknown account is refused");
}

#[test]
fn a_removed_account_takes_its_rows_with_it() {
    let mut c = at(10);
    migrate(&mut c).unwrap();
    account(&c);
    add(&c, "set_read", r#"{"read":true}"#).unwrap();
    c.execute("DELETE FROM account WHERE id = 'acc'", []).unwrap();
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM pending_action", [], |r| r.get(0)).unwrap(), 0);
}

#[test]
fn the_reversal_gives_back_0010() {
    let mut c = at(10);
    let before: Vec<String> = table_names(&c).unwrap();
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"pending_action".to_string()));
    revert_to(&mut c, 10).unwrap();
    assert_eq!(table_names(&c).unwrap(), before, "0010's tables exactly");
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM sqlite_master WHERE name = 'pending_action_due'", [], |r| r.get(0)).unwrap(), 0, "the index goes too");
}
