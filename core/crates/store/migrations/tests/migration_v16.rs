//! Migration 0016 (contracts delta 23): the four columns of a received attachment (moved out of `remote_id`'s JSON) and the source chunk table.
//! MUTANTS (_captures/replies-contract/mutants.py Y01..Y11; each must turn the test named red):
//!   Y01 the move leaves the JSON in remote_id                                                -> the_json_of_body_r2_moves_into_the_columns_and_other_remote_ids_stay
//!   Y02 the inline flag is not moved                                                         -> the_json_of_body_r2_moves_into_the_columns_and_other_remote_ids_stay
//!   Y03 the move also reads remote_ids that are not JSON                                     -> a_copy_of_a_version_15_database_migrates_and_keeps_every_row
//!   Y04 an empty content id is moved as an empty string                                      -> the_json_of_body_r2_moves_into_the_columns_and_other_remote_ids_stay
//!   Y05 source_chunk loses its source-key CHECK                                              -> a_source_chunk_is_a_file_or_an_event_item_and_goes_with_its_source
//!   Y06 source_chunk loses UNIQUE (source, item, number)                                     -> a_source_chunk_is_a_file_or_an_event_item_and_goes_with_its_source
//!   Y07 the source-gone trigger is disabled                                                  -> a_source_chunk_is_a_file_or_an_event_item_and_goes_with_its_source
//!   Y08 the inline column loses its CHECK                                                    -> the_json_of_body_r2_moves_into_the_columns_and_other_remote_ids_stay
//!   Y09 the reversal does not put the JSON back                                              -> the_reversal_gives_back_0015_and_the_json
//!   Y10 the reversal keeps source_chunk                                                      -> the_reversal_gives_back_0015_and_the_json
//!   Y11 the migration changes the chunk table                                                -> a_copy_of_a_version_15_database_migrates_and_keeps_every_row
use rusqlite::{params, Connection};
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn message_cols(c: &Connection) -> Vec<String> {
    c.prepare("SELECT name FROM pragma_table_info('message')").unwrap().query_map([], |r| r.get::<_, String>(0)).unwrap().map(Result::unwrap).collect()
}
// the columns `message` gained after version 22, at the end, in the order they were added: 0023 (lead ruling A144) the two threading headers,
// 0027 (lane real-replyto) the Reply-To. A later migration that adds a column to `message` must name it here.
fn message_gained_since_0022() -> Vec<String> { ["rfc_message_id", "rfc_references_json", "reply_to_json"].map(String::from).to_vec() }
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn mail(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES ('t1', 'acc', 's', '[]', 1)", []).unwrap();
    c.execute("INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) VALUES ('m1', 't1', 'acc', '7', 'S', 's@x.example', 's', 1, 'INBOX')", []).unwrap();
}
fn att(c: &Connection, id: &str, remote: Option<&str>) {
    c.execute("INSERT INTO attachment (id, message_id, remote_id, file_name, mime_type, size_bytes) VALUES (?1, 'm1', ?2, 'f.pdf', 'application/pdf', 10)", params![id, remote]).unwrap();
}
/// a version-15 database with the four shapes of `remote_id` body-r2 and the server could have written
fn v15_with_attachments() -> Connection {
    let c = at(15);
    mail(&c);
    att(&c, "m1:att:001", Some(r#"{"part":"2","enc":"base64","cid":"logo@x.example","inline":true}"#));
    att(&c, "m1:att:002", Some(r#"{"part":"1.2","enc":"quoted-printable","cid":"","inline":false}"#));
    att(&c, "m1:att:003", Some("uid-7"));
    att(&c, "m1:att:004", None);
    att(&c, "m1:att:005", Some(r#"{"other":"object, no part"}"#));
    c
}
type Row = (Option<String>, Option<String>, Option<String>, Option<String>, i64);
fn row(c: &Connection, id: &str) -> Row {
    c.query_row("SELECT remote_id, part_path, transfer_encoding, content_id, inline FROM attachment WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).unwrap()
}

#[test]
fn the_json_of_body_r2_moves_into_the_columns_and_other_remote_ids_stay() {
    let mut c = v15_with_attachments();
    migrate(&mut c).unwrap();
    assert_eq!(row(&c, "m1:att:001"), (None, Some("2".into()), Some("base64".into()), Some("logo@x.example".into()), 1), "the JSON is moved, remote_id is emptied");
    assert_eq!(row(&c, "m1:att:002"), (None, Some("1.2".into()), Some("quoted-printable".into()), None, 0), "an empty content id is NULL, not an empty string");
    assert_eq!(row(&c, "m1:att:003"), (Some("uid-7".into()), None, None, None, 0), "a remote id that is not JSON is left alone");
    assert_eq!(row(&c, "m1:att:004"), (None, None, None, None, 0));
    assert_eq!(row(&c, "m1:att:005").0.as_deref(), Some(r#"{"other":"object, no part"}"#), "a JSON object with no part is not body-r2's: left alone");
    assert!(c.execute("UPDATE attachment SET inline = 2 WHERE id = 'm1:att:001'", []).is_err(), "inline is 0 or 1");
    assert!(c.execute("UPDATE attachment SET part_path = ' ' WHERE id = 'm1:att:001'", []).is_err(), "a blank part");
    att(&c, "m1:att:006", None);
    assert_eq!(n(&c, "SELECT inline FROM attachment WHERE id = 'm1:att:006'"), 0, "a new row is not inline unless said");
}

#[test]
fn a_source_chunk_is_a_file_or_an_event_item_and_goes_with_its_source() {
    let mut c = at(15);
    migrate(&mut c).unwrap();
    let file_key = "local\u{1f}~/Documents/Propositions";
    let ins = |c: &Connection, src: &str, item: &str, no: i64, text: &str| c.execute("INSERT INTO source_chunk (source_key, item_key, chunk_no, title, text) VALUES (?1, ?2, ?3, 't', ?4)", params![src, item, no, text]);
    ins(&c, file_key, "a.pdf", 0, "one").unwrap();
    ins(&c, file_key, "a.pdf", 1, "two").unwrap();
    ins(&c, "calendar\u{1f}cal-1", "ev-1", 0, "event").unwrap();
    assert!(ins(&c, file_key, "a.pdf", 0, "dup").is_err(), "one chunk per (source, item, number)");
    assert!(ins(&c, "mail\u{1f}acc\u{1f}INBOX", "x", 0, "t").is_err(), "a mail folder has its own chunk table");
    assert!(ins(&c, "local\u{1f}", "x", 0, "t").is_err(), "a source key with no path");
    assert!(ins(&c, file_key, " ", 0, "t").is_err(), "no item");
    assert!(ins(&c, file_key, "b.pdf", 0, "  ").is_err(), "no text");
    assert!(ins(&c, file_key, "b.pdf", -1, "t").is_err(), "a negative number");
    // a removed source takes its chunks, another source's stay
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES (?1, 'local_folder', 1)", [file_key]).unwrap();
    c.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES ('calendar' || char(31) || 'cal-1', 'calendar', 1)", []).unwrap();
    c.execute("DELETE FROM reply_source WHERE source_key = ?1", [file_key]).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM source_chunk WHERE source_key = 'local' || char(31) || '~/Documents/Propositions'"), 0, "the folder's excerpts go with it");
    assert_eq!(n(&c, "SELECT count(*) FROM source_chunk"), 1, "the calendar's excerpt stays");
    // ids are never reused, as for chunk
    let newest = n(&c, "SELECT max(id) FROM source_chunk");
    c.execute("DELETE FROM source_chunk", []).unwrap();
    ins(&c, "calendar\u{1f}cal-1", "ev-2", 0, "again").unwrap();
    assert!(n(&c, "SELECT max(id) FROM source_chunk") > newest, "an id is never handed out again");
}

#[test]
fn a_copy_of_a_version_15_database_migrates_and_keeps_every_row() {
    let dir = std::env::temp_dir().join(format!("whoosh-m16-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let src = v15_with_attachments();
        src.execute("INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text) VALUES ('m1', 'body', 0, 1, 'S', 'INBOX', 's', 'text')", []).unwrap();
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let cols = |c: &Connection, t: &str| n(c, &format!("SELECT count(*) FROM pragma_table_info('{t}')"));
    let before = { let c = Connection::open(&copy).unwrap(); (n(&c, "SELECT count(*) FROM attachment"), n(&c, "SELECT count(*) FROM chunk"), n(&c, "SELECT count(*) FROM message"), cols(&c, "chunk"), cols(&c, "message"), cols(&c, "attachment")) };
    let before_message_names = { let c = Connection::open(&copy).unwrap(); message_cols(&c) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (15, MIGRATIONS.last().unwrap().0));
    assert_eq!((n(&c, "SELECT count(*) FROM attachment"), n(&c, "SELECT count(*) FROM chunk"), n(&c, "SELECT count(*) FROM message"), cols(&c, "chunk"), cols(&c, "message")), (before.0, before.1, before.2, before.3, before.4 + 3), "every row kept; chunk untouched, message plus rfc_message_id and rfc_references_json (0023, lead ruling A144) and reply_to_json (0027)");
    let mut want_message_names = before_message_names.clone();
    want_message_names.extend(message_gained_since_0022());
    assert_eq!(message_cols(&c), want_message_names, "every old column of message in its place, then exactly the three added since, by name, at the end");
    assert_eq!(cols(&c, "attachment"), before.5 + 4, "attachment gains exactly four columns");
    assert_eq!(n(&c, "SELECT count(*) FROM chunk WHERE id = 1 AND message_id = 'm1'"), 1, "an existing chunk keeps its id and its message");
    assert_eq!(row(&c, "m1:att:003").0.as_deref(), Some("uid-7"));
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 15, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_0015_and_the_json() {
    let mut c = v15_with_attachments();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"source_chunk".to_string()));
    revert_to(&mut c, 15).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables, "0015's tables exactly");
    assert_eq!(schema_hash(&c).unwrap(), hash, "0015's schema exactly: four columns, the table, its index and trigger are gone");
    let back = |id: &str| c.query_row("SELECT json_extract(remote_id, '$.part'), json_extract(remote_id, '$.enc'), json_extract(remote_id, '$.cid'), json_extract(remote_id, '$.inline') FROM attachment WHERE id = ?1", [id], |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<i64>>(3)?))).unwrap();
    assert_eq!(back("m1:att:001"), (Some("2".into()), Some("base64".into()), Some("logo@x.example".into()), Some(1)), "the JSON body-r2 wrote is back, readable by its own json_extract");
    assert_eq!(back("m1:att:002").0.as_deref(), Some("1.2"));
    assert_eq!(n(&c, "SELECT count(*) FROM attachment WHERE remote_id = 'uid-7'"), 1);
}
