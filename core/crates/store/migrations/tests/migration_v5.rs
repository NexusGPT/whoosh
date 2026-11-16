//! Migration 0005 (contracts delta 10): star and mute as stored state, the List-Unsubscribe header, remind-if-no-reply.
//! Each column group with the case that breaks it, and the reversal back to 0004's exact schema objects.
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_version, MIGRATIONS};

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    migrate(&mut c).unwrap();
    c
}

fn at_v4() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..4] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", 4).unwrap();
    c
}

fn seed(c: &Connection) {
    c.execute_batch(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('a','o@x.test','imap','A','#336699',0,'manual','svc' || char(31) || 'a',0);
         INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t','a','s','[]',1);
         INSERT INTO message (id, thread_id, account_id, sender_name, from_address, to_json, cc_json, subject, preview, received_at_ms, read, archived, folder)
           VALUES ('m','t','a','S','s@x.test','[]','[]','s','p',1,0,0,'INBOX');
         INSERT INTO draft (id, from_account_id, updated_at_ms) VALUES ('d','a',0);",
    ).unwrap();
}

#[test]
fn star_and_mute_default_to_off_and_take_only_zero_or_one() {
    let c = fresh();
    seed(&c);
    assert_eq!(c.query_row("SELECT starred + muted FROM message WHERE id='m'", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    c.execute("UPDATE message SET starred = 1, muted = 1 WHERE id = 'm'", []).unwrap();
    assert!(c.execute("UPDATE message SET starred = 2 WHERE id = 'm'", []).is_err(), "a flag is 0 or 1");
}

#[test]
fn the_list_header_is_json_or_nothing() {
    let c = fresh();
    seed(&c);
    c.execute("UPDATE message SET list_unsubscribe_json = '[\"<https://l.example/u>\"]', list_unsubscribe_one_click = 1 WHERE id = 'm'", []).unwrap();
    assert!(c.execute("UPDATE message SET list_unsubscribe_json = 'not json' WHERE id = 'm'", []).is_err());
}

#[test]
fn a_reply_reminder_rides_the_draft_then_the_reminder_table() {
    let c = fresh();
    seed(&c);
    c.execute("UPDATE draft SET remind_if_no_reply_at_ms = 10 WHERE id = 'd'", []).unwrap();
    c.execute("INSERT INTO reminder (id, message_id, when_ms, updated_at_ms, if_no_reply) VALUES ('r','m',10,0,1)", []).unwrap();
    assert!(c.execute("INSERT INTO reminder (id, message_id, when_ms, updated_at_ms, if_no_reply) VALUES ('r2','m',10,0,2)", []).is_err());
}

#[test]
fn the_reversal_gives_back_exactly_0004s_schema() {
    let v4 = at_v4();
    let objects = |c: &Connection| -> Vec<(String, String)> {
        c.prepare("SELECT type, name FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect()
    };
    let cols = |c: &Connection, t: &str| -> Vec<String> { c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() };
    let mut c = fresh();
    seed(&c);
    c.execute("INSERT INTO reminder (id, message_id, when_ms, updated_at_ms, if_no_reply) VALUES ('r','m',10,0,1)", []).unwrap();
    let applied = revert_to(&mut c, 4).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (store_migrations::latest_version(), 4)); // from the latest (0006 since contracts delta 11)
    assert_eq!(objects(&c), objects(&v4));
    for t in ["message", "draft", "reminder"] { assert_eq!(cols(&c, t), cols(&v4, t), "{t} columns"); }
    assert_eq!(c.query_row("SELECT count(*) FROM message", [], |r| r.get::<_, i64>(0)).unwrap(), 1, "the mail stays");
    migrate(&mut c).unwrap();
    assert_eq!(schema_version(&c).unwrap(), store_migrations::latest_version());
}
