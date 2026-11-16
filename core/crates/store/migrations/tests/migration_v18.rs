//! Migration 0018 (contracts delta 24): a folder gone from the server stays as a row. Two columns, two triggers, a reversal.
//! MUTANTS (_captures/replies-contract/mutants.py Q01..Q05; each must turn the test named red):
//!   Q01 the insert trigger is disabled (a gone row that is on)                   -> a_gone_row_is_off_and_the_owners_choice_exists_only_while_gone
//!   Q02 the update trigger is disabled                                           -> a_gone_row_is_off_and_the_owners_choice_exists_only_while_gone
//!   Q03 gone loses its 0/1 CHECK                                                 -> a_gone_row_is_off_and_the_owners_choice_exists_only_while_gone
//!   Q04 the reversal keeps the columns                                           -> the_reversal_gives_back_0017
//!   Q05 an existing row starts gone                                              -> a_copy_of_a_version_17_database_migrates_and_keeps_every_row
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
fn account(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
}
const KEY: &str = "mail' || char(31) || 'acc' || char(31) || 'Old";

#[test]
fn a_gone_row_is_off_and_the_owners_choice_exists_only_while_gone() {
    let mut c = at(17);
    account(&c);
    c.execute(&format!("INSERT INTO reply_source (source_key, kind, account_id, enabled, item_count) VALUES ('{KEY}', 'mail_folder', 'acc', 1, 1840)"), []).unwrap();
    migrate(&mut c).unwrap();
    assert_eq!(n(&c, "SELECT gone FROM reply_source"), 0, "an existing row is present");
    assert!(c.execute("UPDATE reply_source SET gone = 1", []).is_err(), "gone while still on");
    assert!(c.execute("UPDATE reply_source SET owner_enabled = 1", []).is_err(), "a remembered choice while present");
    assert!(c.execute("UPDATE reply_source SET gone = 2, enabled = 0", []).is_err(), "gone is 0 or 1");
    c.execute("UPDATE reply_source SET gone = 1, enabled = 0, owner_enabled = 1", []).expect("goes: off, the choice remembered, the count stays");
    assert_eq!((n(&c, "SELECT item_count FROM reply_source"), n(&c, "SELECT owner_enabled FROM reply_source")), (1840, 1));
    assert!(c.execute("UPDATE reply_source SET enabled = 1", []).is_err(), "a gone row cannot be switched on");
    c.execute("UPDATE reply_source SET gone = 0, enabled = owner_enabled, owner_enabled = NULL", []).expect("comes back with the owner's choice");
    assert_eq!(n(&c, "SELECT enabled FROM reply_source"), 1);
    assert!(c.execute(&format!("INSERT INTO reply_source (source_key, kind, account_id, enabled, gone) VALUES ('{}', 'mail_folder', 'acc', 1, 1)", KEY.replace("Old", "New")), []).is_err(), "a new gone row that is on");
}

#[test]
fn a_copy_of_a_version_17_database_migrates_and_keeps_every_row() {
    let dir = std::env::temp_dir().join(format!("whoosh-m18-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let src = at(17);
        account(&src);
        src.execute(&format!("INSERT INTO reply_source (source_key, kind, account_id, enabled, item_count) VALUES ('{KEY}', 'mail_folder', 'acc', 1, 1840)"), []).unwrap();
        src.execute("INSERT INTO reply_source (source_key, kind, enabled) VALUES ('local' || char(31) || '~/Docs', 'local_folder', 1)", []).unwrap();
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let cols = |c: &Connection| n(c, "SELECT count(*) FROM pragma_table_info('reply_source')");
    let before = { let c = Connection::open(&copy).unwrap(); (n(&c, "SELECT count(*) FROM reply_source"), cols(&c)) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (17, MIGRATIONS.last().unwrap().0));
    assert_eq!((n(&c, "SELECT count(*) FROM reply_source"), cols(&c)), (before.0, before.1 + 2), "every row kept, two columns added");
    assert_eq!(n(&c, "SELECT count(*) FROM reply_source WHERE gone = 0 AND owner_enabled IS NULL AND enabled = 1"), 2, "nothing starts gone, the owner's switches are as they were");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 17);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_0017() {
    let mut c = at(17);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    revert_to(&mut c, 17).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the two columns and both triggers are gone");
}
