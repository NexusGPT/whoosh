//! Migration 0028 (lane real-settings): three columns on three existing tables, nothing else: `notch_setting.list_shows`,
//! `notification_setting.delivery`, `ai_provider.mail_consent_at_ms`. Written by lane fix-migtests (the migration shipped with no test of its
//! own). "The version before" is every migration below 28 (0024 to 0026 belong to other lanes, merged later), as migration_v27_reply_to.rs does.
//! Not run by this lane (no cargo here): the lead runs `cargo test` in core/crates/store/migrations.
//! RED-FIRST (one-line broken copy per test):
//!   M1 0028_settings_cmds.sql: `list_shows ... DEFAULT 'latest'` -> `DEFAULT 'unread'`                       -> a_copy_of_the_version_before_migrates_keeps_every_row_and_names_every_new_column
//!   M2 0028_settings_cmds.sql: drop the `CHECK (delivery IN (...))`                                        -> a_copy_of_the_version_before_migrates_keeps_every_row_and_names_every_new_column
//!   M3 0028_settings_cmds.sql: `mail_consent_at_ms INTEGER` -> `mail_consent_at_ms INTEGER NOT NULL DEFAULT 0` -> a_copy_of_the_version_before_migrates_keeps_every_row_and_names_every_new_column
//!   M4 0028_settings_cmds.down.sql: drop the line `ALTER TABLE ai_provider DROP COLUMN mail_consent_at_ms;`   -> the_reversal_gives_back_the_version_before
//!   M5 lib.rs: the `(28, ...down.sql)` entry removed from REVERSALS                                         -> the_reversal_gives_back_the_version_before
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, schema_hash, table_names, MIGRATIONS};

fn before() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    let mut last = 0;
    for (v, sql) in MIGRATIONS.iter().filter(|(v, _)| *v < 28) { c.execute_batch(sql).unwrap(); last = *v; }
    c.pragma_update(None, "user_version", last).unwrap();
    c
}
fn n(c: &Connection, sql: &str) -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn s(c: &Connection, sql: &str) -> String { c.query_row(sql, [], |r| r.get(0)).unwrap() }
fn cols(c: &Connection, t: &str) -> Vec<String> {
    c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect()
}
/// the owner's stored settings: one row in each of the three tables, every OLD column set to a value that is not its default
fn settings(c: &Connection) {
    c.execute("INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) VALUES ('acc', 'a@example.test', 'imap', 'W', '#336699', 0, 'manual', 'whoosh' || char(31) || 'acc', 1)", []).unwrap();
    c.execute("INSERT INTO notch_setting (id, show, buttons_json, hidden_in_screen_share, follow_focus) VALUES (1, 0, '[\"archive\"]', 0, 1)", []).unwrap();
    c.execute("INSERT INTO notification_setting (account_id, enabled, updated_at_ms) VALUES ('acc', 0, 5)", []).unwrap();
    c.execute("INSERT INTO ai_provider (id, name, models_json, connected, keychain_ref, updated_at_ms) VALUES ('p1', 'Provider', '[\"m\"]', 1, 'whoosh' || char(31) || 'p1', 9)", []).unwrap();
}
/// every old column of the three rows, as SQLite quotes them
const OLD: [&str; 3] = [
    "SELECT quote(id) || '|' || quote(show) || '|' || quote(buttons_json) || '|' || quote(hidden_in_screen_share) || '|' || quote(follow_focus) FROM notch_setting",
    "SELECT quote(account_id) || '|' || quote(enabled) || '|' || quote(updated_at_ms) FROM notification_setting",
    "SELECT quote(id) || '|' || quote(name) || '|' || quote(models_json) || '|' || quote(connected) || '|' || quote(keychain_ref) || '|' || quote(updated_at_ms) FROM ai_provider",
];
fn old_rows(c: &Connection) -> Vec<String> { OLD.iter().map(|q| s(c, q)).collect() }
fn plus(mut v: Vec<String>, names: &[&str]) -> Vec<String> { v.extend(names.iter().map(|x| x.to_string())); v }

/// The tables of the database without the ones a LATER migration adds (0033: notch_alerts, the notch's alert settings): this test judges its own migration.
fn earlier_tables(c: &rusqlite::Connection) -> Vec<String> { table_names(c).unwrap().into_iter().filter(|t| t != "notch_alerts").collect() }

#[test]
fn a_copy_of_the_version_before_migrates_keeps_every_row_and_names_every_new_column() {
    let dir = std::env::temp_dir().join(format!("whoosh-m28-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (orig, copy) = (dir.join("whoosh.sqlite"), dir.join("whoosh-copy.sqlite"));
    let (tables, notch, notif, provider, message, rows, before_version) = {
        let src = before();
        settings(&src);
        let t = (table_names(&src).unwrap(), cols(&src, "notch_setting"), cols(&src, "notification_setting"), cols(&src, "ai_provider"), cols(&src, "message"), old_rows(&src), n(&src, "PRAGMA user_version"));
        src.execute("VACUUM INTO ?1", [orig.to_str().unwrap()]).unwrap();
        t
    };
    std::fs::copy(&orig, &copy).unwrap();
    let mut c = Connection::open(&copy).unwrap();
    let applied = migrate(&mut c).unwrap();
    assert_eq!((applied.from_version, applied.to_version), (before_version, MIGRATIONS.last().unwrap().0));
    assert_eq!(earlier_tables(&c), tables, "no table added or removed (columns only)");
    assert_eq!(cols(&c, "notch_setting"), plus(notch, &["list_shows"]), "notch_setting gains exactly list_shows, at the end");
    assert_eq!(cols(&c, "notification_setting"), plus(notif, &["delivery"]), "notification_setting gains exactly delivery, at the end");
    assert_eq!(cols(&c, "ai_provider"), plus(provider, &["mail_consent_at_ms"]), "ai_provider gains exactly mail_consent_at_ms, at the end");
    // 0028 changes no other table than these three: message is what the migrations up to the latest left it (0027's column is named in migration_v27_reply_to.rs)
    assert_eq!(cols(&c, "message"), message, "message is untouched by 0028");
    assert_eq!(old_rows(&c), rows, "every old column of every stored row is as it was");
    assert_eq!((n(&c, "SELECT count(*) FROM notch_setting"), n(&c, "SELECT count(*) FROM notification_setting"), n(&c, "SELECT count(*) FROM ai_provider")), (1, 1, 1), "every row kept");
    // what the new columns read for an old row: the defaults the contract names, and no consent given
    assert_eq!(s(&c, "SELECT list_shows FROM notch_setting"), "latest", "an old notch keeps showing the latest mail");
    assert_eq!(s(&c, "SELECT delivery FROM notification_setting"), "both", "an old account announces as before: the notch and the notifications");
    assert_eq!(n(&c, "SELECT mail_consent_at_ms IS NULL FROM ai_provider"), 1, "no old provider has been given consent");
    // the new columns hold only what they may
    for ok in ["unread", "latest"] { c.execute("UPDATE notch_setting SET list_shows = ?1", [ok]).unwrap(); }
    for ok in ["notch", "notifications", "both"] { c.execute("UPDATE notification_setting SET delivery = ?1", [ok]).unwrap(); }
    c.execute("UPDATE ai_provider SET mail_consent_at_ms = 1700000000000", []).unwrap();
    assert!(c.execute("UPDATE notch_setting SET list_shows = 'all'", []).is_err(), "list_shows is latest or unread");
    assert!(c.execute("UPDATE notification_setting SET delivery = 'email'", []).is_err(), "delivery is notch, notifications or both");
    assert!(c.execute("UPDATE notification_setting SET delivery = NULL", []).is_err(), "delivery is never empty");
    assert_eq!(n(&Connection::open(&orig).unwrap(), "PRAGMA user_version"), before_version, "the original is untouched");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_reversal_gives_back_the_version_before() {
    let mut c = before();
    let (tables, hash) = (table_names(&c).unwrap(), schema_hash(&c).unwrap());
    let version_before = n(&c, "PRAGMA user_version");
    settings(&c);
    let rows = old_rows(&c);
    migrate(&mut c).unwrap();
    c.execute("UPDATE notch_setting SET list_shows = 'unread'", []).unwrap();
    c.execute("UPDATE notification_setting SET delivery = 'notch'", []).unwrap();
    c.execute("UPDATE ai_provider SET mail_consent_at_ms = 1", []).unwrap();
    let back = revert_to(&mut c, 27).unwrap();
    assert_eq!(back.to_version, version_before, "undoing 0028 leaves the version 0027 left");
    assert_eq!(table_names(&c).unwrap(), tables);
    assert_eq!(schema_hash(&c).unwrap(), hash, "the schema is the version before's, byte for byte");
    assert_eq!(old_rows(&c), rows, "the settings stay; only the three new choices are lost");
}
