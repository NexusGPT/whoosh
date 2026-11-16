//! Migration 0015 (contracts delta 22): the files of a draft and of a queued message. One new table, a reversal.
//! MUTANTS (_captures/replies-contract/mutants.py U01..U08; each must turn the test named red):
//!   U01 the source-shape CHECK is dropped (a local file with an attachment id, a stored attachment with a path)  -> a_row_is_a_local_file_or_a_stored_attachment_never_both
//!   U02 the one-source-per-owner index is not unique                                                           -> the_same_source_is_one_row_per_owner
//!   U03 the state CHECK allows any word                                                                         -> a_row_is_a_local_file_or_a_stored_attachment_never_both
//!   U04 the draft-gone trigger is disabled (the files outlive their draft)                                      -> the_files_follow_their_owner_and_move_to_the_outbox_at_send
//!   U05 the outbox-gone trigger is disabled                                                                     -> the_files_follow_their_owner_and_move_to_the_outbox_at_send
//!   U06 size_bytes may be negative                                                                              -> a_row_is_a_local_file_or_a_stored_attachment_never_both
//!   U07 the reversal keeps the table                                                                            -> the_reversal_gives_back_0014
//!   U08 the migration changes an existing table (draft gains a column)                                          -> a_copy_of_a_version_14_database_migrates_and_keeps_every_row
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn latest() -> Connection { let mut c = at(14); migrate(&mut c).unwrap(); c }
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn account(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
}
fn draft(c: &Connection, id: &str) { c.execute("INSERT INTO draft (id, from_account_id, updated_at_ms) VALUES (?1, 'acc', 1)", [id]).unwrap(); }
fn local(c: &Connection, id: &str, owner: &str, kind: &str, path: &str) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO outgoing_attachment (id, owner_kind, owner_id, source_kind, path, file_name, media_type, size_bytes, state) VALUES (?1, ?2, ?3, 'local_file', ?4, 'Devis.pdf', 'application/pdf', 1200, 'ready')", [id, kind, owner, path])
}
fn stored(c: &Connection, id: &str, owner: &str, kind: &str, msg: &str, att: &str) -> rusqlite::Result<usize> {
    c.execute("INSERT INTO outgoing_attachment (id, owner_kind, owner_id, source_kind, message_id, attachment_id, file_name, media_type, size_bytes, state) VALUES (?1, ?2, ?3, 'stored_message', ?4, ?5, 'Plan.png', 'image/png', 900, 'will_download')", [id, kind, owner, msg, att])
}

#[test]
fn a_row_is_a_local_file_or_a_stored_attachment_never_both() {
    let c = latest();
    account(&c); draft(&c, "d1");
    local(&c, "a1", "d1", "draft", "~/Documents/Devis.pdf").unwrap();
    stored(&c, "a2", "d1", "draft", "m1", "att-1").unwrap();
    assert!(c.execute("INSERT INTO outgoing_attachment (id, owner_kind, owner_id, source_kind, path, attachment_id, file_name, media_type, size_bytes, state) VALUES ('x1', 'draft', 'd1', 'local_file', '~/a', 'att', 'a', 'b', 1, 'ready')", []).is_err(), "a local file with an attachment id");
    assert!(c.execute("INSERT INTO outgoing_attachment (id, owner_kind, owner_id, source_kind, path, message_id, attachment_id, file_name, media_type, size_bytes, state) VALUES ('x2', 'draft', 'd1', 'stored_message', '~/a', 'm', 'a', 'f', 'b', 1, 'ready')", []).is_err(), "a stored attachment with a path");
    assert!(c.execute("INSERT INTO outgoing_attachment (id, owner_kind, owner_id, source_kind, file_name, media_type, size_bytes, state) VALUES ('x3', 'draft', 'd1', 'local_file', 'a', 'b', 1, 'ready')", []).is_err(), "a local file with no path");
    assert!(local(&c, "x4", "d1", "draft", "  ").is_err(), "a blank path");
    assert!(local(&c, "x5", "d1", "somewhere", "~/b").is_err(), "an owner kind outside draft and outbox");
    assert!(c.execute("UPDATE outgoing_attachment SET state = 'sent' WHERE id = 'a1'", []).is_err(), "a state outside the four");
    assert!(c.execute("UPDATE outgoing_attachment SET size_bytes = -1 WHERE id = 'a1'", []).is_err(), "a negative size");
}

#[test]
fn the_same_source_is_one_row_per_owner() {
    let c = latest();
    account(&c); draft(&c, "d1"); draft(&c, "d2");
    local(&c, "a1", "d1", "draft", "~/Documents/Devis.pdf").unwrap();
    assert!(local(&c, "a2", "d1", "draft", "~/Documents/Devis.pdf").is_err(), "the same file twice on one draft");
    local(&c, "a3", "d2", "draft", "~/Documents/Devis.pdf").expect("another draft may attach the same file");
    stored(&c, "s1", "d1", "draft", "m1", "att-1").unwrap();
    assert!(stored(&c, "s2", "d1", "draft", "m1", "att-1").is_err(), "the same stored attachment twice");
    stored(&c, "s3", "d1", "draft", "m1", "att-2").expect("another attachment of the same message");
}

#[test]
fn the_files_follow_their_owner_and_move_to_the_outbox_at_send() {
    let c = latest();
    account(&c); draft(&c, "d1");
    local(&c, "a1", "d1", "draft", "~/a.pdf").unwrap();
    stored(&c, "a2", "d1", "draft", "m1", "att-1").unwrap();
    // Send: the rows are re-owned to the outbox entry, then the draft goes; the files stay
    c.execute("INSERT INTO outbox_entry (id, draft_json, state, due_at_ms, created_at_ms, updated_at_ms) VALUES ('o1', '{}', 'undoable', 5, 1, 1)", []).unwrap();
    c.execute("UPDATE outgoing_attachment SET owner_kind = 'outbox', owner_id = 'o1' WHERE owner_kind = 'draft' AND owner_id = 'd1'", []).unwrap();
    c.execute("DELETE FROM draft WHERE id = 'd1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM outgoing_attachment WHERE owner_kind = 'outbox' AND owner_id = 'o1'"), 2, "the queued message keeps its files after its draft went");
    c.execute("DELETE FROM outbox_entry WHERE id = 'o1'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM outgoing_attachment"), 0, "a removed outbox entry takes its files");
    draft(&c, "d2");
    local(&c, "b1", "d2", "draft", "~/b.pdf").unwrap();
    c.execute("DELETE FROM draft WHERE id = 'd2'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM outgoing_attachment"), 0, "a deleted draft takes its files");
}

#[test]
fn a_copy_of_a_version_14_database_migrates_and_keeps_every_row() {
    let dir = std::env::temp_dir().join(format!("whoosh-m15-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let c = Connection::open(&orig).unwrap();
        c.pragma_update(None, "foreign_keys", true).unwrap();
        for (_, sql) in &MIGRATIONS[..14] { c.execute_batch(sql).unwrap(); }
        c.pragma_update(None, "user_version", 14i64).unwrap();
        account(&c); draft(&c, "d1");
        c.execute("INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text) VALUES ('nobody', 'body', 0, 1, 's', 'f', 's', 't')", []).ok();
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").ok();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let cols = |c: &Connection, t: &str| n(c, &format!("SELECT count(*) FROM pragma_table_info('{t}')"));
    let before = { let c = Connection::open(&copy).unwrap(); (n(&c, "SELECT count(*) FROM draft"), n(&c, "SELECT count(*) FROM account"), cols(&c, "draft"), cols(&c, "outbox_entry")) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (14, MIGRATIONS.last().unwrap().0));
    assert_eq!((n(&c, "SELECT count(*) FROM draft"), n(&c, "SELECT count(*) FROM account"), cols(&c, "draft"), cols(&c, "outbox_entry")), before, "every row and every column of an old table is as it was");
    assert_eq!(n(&c, "SELECT count(*) FROM outgoing_attachment"), 0);
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 14, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_0014() {
    let mut c = at(14);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"outgoing_attachment".to_string()));
    revert_to(&mut c, 14).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables, "0014's tables exactly");
    assert_eq!(schema_hash(&c).unwrap(), hash, "0014's schema exactly: the index and both triggers are gone");
}
