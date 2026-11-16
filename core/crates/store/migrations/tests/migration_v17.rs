//! Migration 0017 (contracts delta 23 addition): stored consent for a hosted decision model.
//! MUTANTS (_captures/replies-contract/mutants.py R01..R04; each must turn the test named red):
//!   R01 the provider CHECK allows any word                       -> a_consent_is_one_row_per_hosted_provider_and_none_means_not_given
//!   R02 the key is not unique (two rows for jev)                 -> a_consent_is_one_row_per_hosted_provider_and_none_means_not_given
//!   R03 the reversal keeps the table                              -> the_reversal_gives_back_0016
//!   R04 the migration changes decision_model                     -> a_copy_of_a_version_16_database_migrates_and_keeps_every_row
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
fn a_consent_is_one_row_per_hosted_provider_and_none_means_not_given() {
    let mut c = at(16);
    migrate(&mut c).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM decision_consent"), 0, "the default is not given");
    c.execute("INSERT INTO decision_consent (provider, given_at_ms) VALUES ('jev', 5)", []).unwrap();
    assert!(c.execute("INSERT INTO decision_consent (provider, given_at_ms) VALUES ('jev', 6)", []).is_err(), "one row per provider");
    assert!(c.execute("INSERT INTO decision_consent (provider, given_at_ms) VALUES ('local', 6)", []).is_err(), "a model that stays on this Mac holds no consent");
    c.execute("DELETE FROM decision_consent WHERE provider = 'jev'", []).unwrap();
    assert_eq!(n(&c, "SELECT count(*) FROM decision_consent"), 0, "withdrawing deletes the row");
    c.execute("INSERT INTO decision_consent (provider, given_at_ms) VALUES ('jev', 7)", []).expect("and it can be given again");
}

#[test]
fn a_copy_of_a_version_16_database_migrates_and_keeps_every_row() {
    let dir = std::env::temp_dir().join(format!("whoosh-m17-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    {
        let src = at(16);
        src.execute("INSERT INTO decision_model (id, provider, model, updated_at_ms) VALUES (1, 'local', 'laya', 9)", []).unwrap();
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
    }
    std::fs::copy(&orig, &copy).unwrap();
    let cols = |c: &Connection| n(c, "SELECT count(*) FROM pragma_table_info('decision_model')");
    let before = { let c = Connection::open(&copy).unwrap(); (n(&c, "SELECT count(*) FROM decision_model"), cols(&c)) };
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (16, MIGRATIONS.last().unwrap().0));
    assert_eq!((n(&c, "SELECT count(*) FROM decision_model"), cols(&c)), before, "the owner's decision model is untouched");
    assert_eq!(n(&c, "SELECT count(*) FROM decision_model WHERE provider = 'local' AND model = 'laya'"), 1);
    assert_eq!(n(&c, "SELECT count(*) FROM decision_consent"), 0, "an existing owner has given no consent");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), 16, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_0016() {
    let mut c = at(16);
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    migrate(&mut c).unwrap();
    assert!(table_names(&c).unwrap().contains(&"decision_consent".to_string()));
    revert_to(&mut c, 16).unwrap();
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash);
}
