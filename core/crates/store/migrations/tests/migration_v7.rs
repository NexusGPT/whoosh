//! Migration 0007 (contracts delta 12): Bcc on messages and drafts, and the reversal to 0006.
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, MIGRATIONS};

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    migrate(&mut c).unwrap();
    c
}

#[test]
fn bcc_is_a_json_array_defaulting_to_empty_on_messages_and_drafts() {
    let c = fresh();
    c.execute_batch(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('a','o@x.test','imap','A','#336699',0,'manual','svc' || char(31) || 'a',0);
         INSERT INTO draft (id, from_account_id, updated_at_ms) VALUES ('d','a',1);",
    ).unwrap();
    assert_eq!(c.query_row("SELECT bcc_json FROM draft WHERE id='d'", [], |r| r.get::<_, String>(0)).unwrap(), "[]");
    assert!(c.execute("UPDATE draft SET bcc_json = 'nope' WHERE id = 'd'", []).is_err());
    let cols: Vec<String> = c.prepare("PRAGMA table_info(message)").unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect();
    assert!(cols.contains(&"bcc_json".to_string()), "message has bcc_json: {cols:?}");
}

#[test]
fn the_reversal_gives_back_exactly_0006s_message_and_draft() {
    let v6 = {
        let c = Connection::open_in_memory().unwrap();
        for (_, sql) in &MIGRATIONS[..6] { c.execute_batch(sql).unwrap(); }
        c
    };
    let cols = |c: &Connection, t: &str| -> Vec<String> { c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() };
    let mut c = fresh();
    let applied = revert_to(&mut c, 6).unwrap();
    assert_eq!(applied.to_version, 6);
    assert_eq!(cols(&c, "message"), cols(&v6, "message"));
    assert_eq!(cols(&c, "draft"), cols(&v6, "draft"));
    assert_eq!(cols(&c, "label_assignment"), cols(&v6, "label_assignment"));
}
