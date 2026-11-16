//! Migration 0012 (junk-r2): the local spam filter's state. Three tables, two nullable message columns, a reversal.
//! MUTANTS (each must turn the test named red):
//!   M1 junk_model loses CHECK (id = 1) (a second model row is stored)               -> one_model_per_profile_and_honest_counts
//!   M2 junk_learned loses its label CHECK ('spam' is stored)                          -> a_lesson_is_junk_or_not_junk_and_goes_with_its_message
//!   M3 junk_learned's message reference has no ON DELETE CASCADE                      -> a_lesson_is_junk_or_not_junk_and_goes_with_its_message
//!   M4 message.junk_score loses its range CHECK (1.5 is stored)                      -> a_score_is_a_chance_and_a_state_is_suspect_or_moved
//!   M5 the reversal keeps junk_learned                                               -> the_reversal_gives_back_0011
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn account_and_message(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) VALUES ('m1', 't1', 'acc', '7', 'S', 's@x.example', 's', 1, 'INBOX')", []).unwrap();
}

#[test]
fn one_model_per_profile_and_honest_counts() {
    let mut c = at(11);
    migrate(&mut c).unwrap();
    c.execute("INSERT INTO junk_model (id, model_bytes, trained_junk, trained_good, updated_at_ms) VALUES (1, x'57', 1, 2, 3)", []).unwrap();
    assert!(c.execute("INSERT INTO junk_model (id, model_bytes, trained_junk, trained_good, updated_at_ms) VALUES (2, x'57', 1, 2, 3)", []).is_err(), "one model per profile");
    assert!(c.execute("UPDATE junk_model SET trained_junk = -1", []).is_err(), "a negative count is refused");
}

#[test]
fn a_lesson_is_junk_or_not_junk_and_goes_with_its_message() {
    let mut c = at(11);
    migrate(&mut c).unwrap();
    account_and_message(&c);
    c.execute("INSERT INTO junk_learned (message_id, label) VALUES ('m1', 'junk')", []).unwrap();
    assert!(c.execute("UPDATE junk_learned SET label = 'spam'", []).is_err(), "only junk and not_junk");
    assert!(c.execute("INSERT INTO junk_learned (message_id, label) VALUES ('nobody', 'junk')", []).is_err(), "an unknown message is refused");
    c.execute("DELETE FROM message WHERE id = 'm1'", []).unwrap();
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM junk_learned", [], |r| r.get(0)).unwrap(), 0, "the lesson goes with its message");
    c.execute("INSERT INTO junk_bootstrap (account_id, done_at_ms, junk_learned, good_learned) VALUES ('acc', 1, 3, 4)", []).unwrap();
    c.execute("DELETE FROM account WHERE id = 'acc'", []).unwrap();
    assert_eq!(c.query_row::<i64, _, _>("SELECT count(*) FROM junk_bootstrap", [], |r| r.get(0)).unwrap(), 0, "a removed account takes its bootstrap row");
}

#[test]
fn a_score_is_a_chance_and_a_state_is_suspect_or_moved() {
    let mut c = at(11);
    migrate(&mut c).unwrap();
    account_and_message(&c);
    let (score, state): (Option<f64>, Option<String>) = c.query_row("SELECT junk_score, junk_state FROM message WHERE id = 'm1'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((score, state), (None, None), "a message stored before 0012 (or never scored) carries nothing");
    c.execute("UPDATE message SET junk_score = ?1, junk_state = 'moved' WHERE id = 'm1'", params![0.95]).unwrap();
    c.execute("UPDATE message SET junk_state = 'suspect' WHERE id = 'm1'", []).unwrap();
    assert!(c.execute("UPDATE message SET junk_score = 1.5 WHERE id = 'm1'", []).is_err(), "a score is in [0, 1]");
    assert!(c.execute("UPDATE message SET junk_score = -0.1 WHERE id = 'm1'", []).is_err(), "a score is in [0, 1]");
    assert!(c.execute("UPDATE message SET junk_state = 'junk' WHERE id = 'm1'", []).is_err(), "only suspect and moved");
}

#[test]
fn the_reversal_gives_back_0011() {
    let mut c = at(11);
    let (before_tables, before_hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    for t in ["junk_bootstrap", "junk_learned", "junk_model"] { assert!(table_names(&c).unwrap().contains(&t.to_string()), "{t}"); }
    revert_to(&mut c, 11).unwrap();
    assert_eq!(table_names(&c).unwrap(), before_tables, "0011's tables exactly");
    assert_eq!(schema_hash(&c).unwrap(), before_hash, "0011's schema exactly, the two message columns gone");
}
