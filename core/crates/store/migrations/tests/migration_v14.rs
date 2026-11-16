//! Migration 0014 (contracts delta 21): the crawl's place, the chunk with its own id, the calendars the shell pushes. Additive, with a reversal.
//! MUTANTS (_captures/replies-contract/mutants.py T01..T12; each must turn the test named red):
//!   T01 chunk.id loses AUTOINCREMENT (the id of a deleted newest chunk is reused)           -> a_chunk_id_is_never_reused_and_a_chunk_goes_with_its_message
//!   T02 chunk's message reference has no ON DELETE CASCADE                                  -> a_chunk_id_is_never_reused_and_a_chunk_goes_with_its_message
//!   T03 chunk loses UNIQUE (message_id, part, chunk_no)                                     -> a_chunk_is_one_part_one_number_and_has_text
//!   T04 chunk loses its part CHECK                                                          -> a_chunk_is_one_part_one_number_and_has_text
//!   T05 the crawl-place UPDATE trigger is disabled                                          -> a_crawl_place_has_both_halves_and_a_finished_walk_has_none
//!   T06 the crawl-place INSERT trigger is disabled                                          -> a_crawl_place_has_both_halves_and_a_finished_walk_has_none
//!   T07 calendar_event loses CHECK (end_ms >= start_ms)                                     -> calendars_and_events_follow_the_shells_list
//!   T08 the reply_calendar_gone trigger is disabled (a switch outlives its calendar)        -> calendars_and_events_follow_the_shells_list
//!   T09 calendar_event has no ON DELETE CASCADE                                             -> calendars_and_events_follow_the_shells_list
//!   T10 reply_calendar loses its colour CHECK                                               -> calendars_and_events_follow_the_shells_list
//!   T11 the reversal keeps chunk                                                            -> the_reversal_gives_back_0013
//!   T12 the migration adds a column to an existing table (message)                          -> a_copy_of_a_version_13_database_migrates_and_keeps_every_row
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn latest() -> Connection { let mut c = at(13); migrate(&mut c).unwrap(); c }
fn mail(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    for id in ["m1", "m2"] {
        c.execute("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) VALUES (?1, 't1', 'acc', ?1, 'S', 's@x.example', 's', 1, 'INBOX')", params![id]).unwrap();
    }
}
fn message_cols(c: &Connection) -> Vec<String> {
    c.prepare("SELECT name FROM pragma_table_info('message')").unwrap().query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
}
// the columns `message` gained after version 22, at the end, in the order they were added: 0023 (lead ruling A144) the two threading headers,
// 0027 (lane real-replyto) the Reply-To. A later migration that adds a column to `message` must name it here.
fn message_gained_since_0022() -> Vec<String> { ["rfc_message_id", "rfc_references_json", "reply_to_json"].map(String::from).to_vec() }
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn refused(c: &Connection, sql: &str, why: &str) { assert!(c.execute(sql, []).is_err(), "must be refused: {why}"); }
fn chunk(c: &Connection, msg: &str, part: &str, no: i64, text: &str) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text) VALUES (?1, ?2, ?3, 1, 'S', 'INBOX', 's', ?4)", params![msg, part, no, text])
}

#[test]
fn a_chunk_id_is_never_reused_and_a_chunk_goes_with_its_message() {
    let c = latest();
    mail(&c);
    chunk(&c, "m1", "body", 0, "one").unwrap();
    chunk(&c, "m2", "body", 0, "two").unwrap();
    let newest = n(&c, "SELECT max(id) FROM chunk");
    c.execute("DELETE FROM message WHERE id = 'm2'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM chunk WHERE message_id = 'm2'"), 0, "a chunk goes with its message");
    chunk(&c, "m1", "body", 1, "three").unwrap();
    assert!(n(&c, "SELECT max(id) FROM chunk") > newest, "the id of a deleted NEWEST chunk is never handed out again: a vector filed under it cannot point at another message");
}

#[test]
fn a_chunk_is_one_part_one_number_and_has_text() {
    let c = latest();
    mail(&c);
    chunk(&c, "m1", "body", 0, "x").unwrap();
    chunk(&c, "m1", "attachment:att-1", 0, "y").unwrap();
    assert!(chunk(&c, "m1", "body", 0, "dup").is_err(), "one chunk per (message, part, number)");
    assert!(chunk(&c, "m1", "footer", 0, "z").is_err(), "a part is body or attachment:<id>");
    assert!(chunk(&c, "m1", "attachment:", 0, "z").is_err(), "an attachment part names its attachment");
    assert!(chunk(&c, "m1", "body", 1, "  ").is_err(), "no text");
    assert!(chunk(&c, "m1", "body", -1, "z").is_err(), "a negative number");
    assert!(chunk(&c, "ghost", "body", 0, "z").is_err(), "an unknown message");
    assert_eq!(n(&c, "SELECT count(*) FROM chunk WHERE embedded_at_ms IS NULL"), 2, "a new chunk has no vector yet");
}

#[test]
fn a_crawl_place_has_both_halves_and_a_finished_walk_has_none() {
    let c = latest();
    mail(&c);
    c.execute("INSERT INTO folder_sync_state (account_id, folder) VALUES ('acc', 'INBOX')", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM folder_sync_state WHERE body_crawl_before_ms IS NULL AND body_crawl_before_id IS NULL AND body_crawl_done_ms IS NULL"), 1, "an existing row starts with no place");
    c.execute("UPDATE folder_sync_state SET body_crawl_before_ms = 5, body_crawl_before_id = 'm9'", []).unwrap();
    refused(&c, "UPDATE folder_sync_state SET body_crawl_before_id = NULL", "a time with no id");
    refused(&c, "UPDATE folder_sync_state SET body_crawl_done_ms = 9", "finished and still walking");
    c.execute("UPDATE folder_sync_state SET body_crawl_before_ms = NULL, body_crawl_before_id = NULL, body_crawl_done_ms = 9", []).unwrap();
    refused(&c, "UPDATE folder_sync_state SET body_crawl_before_ms = 5, body_crawl_before_id = 'm9'", "a place after the walk finished");
    refused(&c, "INSERT INTO folder_sync_state (account_id, folder, body_crawl_before_ms) VALUES ('acc', 'Sent', 5)", "a new row with half a place");
}

#[test]
fn calendars_and_events_follow_the_shells_list() {
    let c = latest();
    c.execute("INSERT INTO reply_calendar (calendar_id, name, account_label, colour) VALUES ('work', 'Travail', 'a', '#007AFF')", []).unwrap();
    refused(&c, "INSERT INTO reply_calendar (calendar_id, name, account_label, colour) VALUES ('x', 'X', 'a', 'blue')", "a colour that is not #RRGGBB");
    refused(&c, "INSERT INTO reply_calendar (calendar_id, name, account_label, colour) VALUES (' ', 'X', 'a', '#007AFF')", "no id");
    c.execute("INSERT INTO calendar_event (calendar_id, event_id, title, start_ms, end_ms) VALUES ('work', 'e1', 'Meeting', 5, 5)", []).unwrap();
    refused(&c, "INSERT INTO calendar_event (calendar_id, event_id, title, start_ms, end_ms) VALUES ('work', 'e2', 'Meeting', 9, 5)", "ends before it starts");
    refused(&c, "INSERT INTO calendar_event (calendar_id, event_id, title, start_ms, end_ms) VALUES ('ghost', 'e3', 'Meeting', 1, 5)", "an unknown calendar");
    refused(&c, "INSERT INTO calendar_event (calendar_id, event_id, title, start_ms, end_ms) VALUES ('work', 'e1', 'Again', 1, 5)", "the same event twice");
    refused(&c, "INSERT INTO calendar_event (calendar_id, event_id, title, start_ms, end_ms, attendees_json) VALUES ('work', 'e4', 'M', 1, 5, '{}')", "attendees that are not a list");
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES ('calendar' || char(31) || 'work', 'calendar', 1)", []).unwrap();
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES ('calendar' || char(31) || 'other', 'calendar', 1)", []).unwrap();
    c.execute("DELETE FROM reply_calendar WHERE calendar_id = 'work'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM calendar_event"), 0, "the events go with their calendar");
    assert_eq!(n(&c, "SELECT count(*) FROM reply_source WHERE kind = 'calendar'"), 1, "the switch of a calendar that is gone goes too, another's stays");
}

#[test]
fn a_copy_of_a_version_13_database_migrates_and_keeps_every_row() {
    let dir = std::env::temp_dir().join(format!("whoosh-m14-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let c = Connection::open(&orig).unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        for (_, sql) in &MIGRATIONS[..13] { c.execute_batch(sql).unwrap(); }
        c.pragma_update(None, "user_version", 13i64).unwrap();
        mail(&c);
        c.execute("INSERT INTO folder_sync_state (account_id, folder) VALUES ('acc', 'INBOX')", []).unwrap();
        c.execute("UPDATE reply_use SET enabled = 1 WHERE kind = 'auto'", []).unwrap();
        c.execute("INSERT INTO reply_tone (id, name, text) VALUES ('t1', 'Formel', 'Formal.')", []).unwrap();
        c.execute("INSERT INTO reply_source (source_key, kind, account_id, enabled) VALUES ('mail' || char(31) || 'acc' || char(31) || 'INBOX', 'mail_folder', 'acc', 1)", []).unwrap();
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").ok();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let cols = |c: &Connection, t: &str| n(c, &format!("SELECT count(*) FROM pragma_table_info('{t}')"));
    let before = { let c = Connection::open(&copy).unwrap(); (n(&c, "SELECT count(*) FROM message"), n(&c, "SELECT count(*) FROM folder_sync_state"), n(&c, "SELECT count(*) FROM reply_tone"), n(&c, "SELECT count(*) FROM reply_source"), cols(&c, "message"), cols(&c, "reply_use")) };
    let before_message_names = { let c = Connection::open(&copy).unwrap(); message_cols(&c) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (13, MIGRATIONS.last().unwrap().0));
    assert_eq!((n(&c, "SELECT count(*) FROM message"), n(&c, "SELECT count(*) FROM folder_sync_state"), n(&c, "SELECT count(*) FROM reply_tone"), n(&c, "SELECT count(*) FROM reply_source"), cols(&c, "message"), cols(&c, "reply_use")), (before.0, before.1, before.2, before.3, before.4 + 3, before.5), "every row of 0013 and every column of an old table is as it was, message plus rfc_message_id and rfc_references_json (0023, lead ruling A144) and reply_to_json (0027)");
    let mut want_message_names = before_message_names.clone();
    want_message_names.extend(message_gained_since_0022());
    assert_eq!(message_cols(&c), want_message_names, "every old column of message in its place, then exactly the three added since, by name, at the end");
    assert_eq!(n(&c, "SELECT enabled FROM reply_use WHERE kind = 'auto'"), 1, "the owner's switch survives");
    assert_eq!(n(&c, "SELECT count(*) FROM pragma_table_info('folder_sync_state')"), 9 + 3, "folder_sync_state gains exactly the three crawl columns");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 13, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_0013() {
    let mut c = at(13);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    for t in ["chunk", "reply_calendar", "calendar_event"] { assert!(table_names(&c).unwrap().contains(&t.to_string()), "{t}"); }
    revert_to(&mut c, 13).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables, "0013's tables exactly");
    assert_eq!(schema_hash(&c).unwrap(), hash, "0013's schema exactly: the three columns, the triggers and the indexes are gone");
}
