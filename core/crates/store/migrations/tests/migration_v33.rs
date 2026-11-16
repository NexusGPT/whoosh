//! Migration 0033 (owner's picks of 14 Nov 2026): the notch's alert settings table. Additive: a database of the version before keeps every row it had,
//! gains an empty `notch_alerts`, and takes one row whose words the CHECKs hold to the contract's spellings.
//! MUTANTS: M1 the CHECK of `stay` dropped                    -> a_word_outside_the_contract_is_refused
//!          M2 `CHECK (id = 1)` dropped                        -> the_table_holds_one_row
//!          M3 the reversal's DROP TABLE removed               -> the_reversal_gives_back_the_version_before
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, MIGRATIONS};

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", false).unwrap();
    for (_, sql) in MIGRATIONS.iter().filter(|(v, _)| (*v as usize) <= version) { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
const ROW: &str = "INSERT INTO notch_alerts (id, vip, needs_reply, primary_mail, updates, promotions, sign_in_codes, stay, sound, full_mail_loads_pictures) \
                   VALUES (?1, 'card', 'card', 'line', 'light', 'nothing', 'card', ?2, 0, 1)";
fn tables(c: &Connection) -> Vec<String> {
    c.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

#[test]
fn a_copy_of_the_version_before_gains_the_empty_table_and_keeps_its_rows() {
    let mut c = at(32);
    c.execute("INSERT INTO notch_setting (id, show, buttons_json, hidden_in_screen_share, follow_focus, list_shows) VALUES (1, 1, '[\"archive\"]', 1, 1, 'unread')", []).unwrap();
    migrate(&mut c).unwrap();
    assert_eq!(c.query_row("SELECT count(*) FROM notch_alerts", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    assert_eq!(c.query_row("SELECT buttons_json || '|' || list_shows FROM notch_setting WHERE id = 1", [], |r| r.get::<_, String>(0)).unwrap(), "[\"archive\"]|unread");
    c.execute(ROW, rusqlite::params![1, "10s"]).unwrap();
    assert_eq!(c.query_row("SELECT primary_mail || '|' || stay FROM notch_alerts", [], |r| r.get::<_, String>(0)).unwrap(), "line|10s");
}

#[test]
fn a_word_outside_the_contract_is_refused() {
    let mut c = at(32);
    migrate(&mut c).unwrap();
    assert!(c.execute(ROW, rusqlite::params![1, "forever"]).is_err(), "a stay outside 5s, 10s, until-acted was stored");
    assert!(c.execute("INSERT INTO notch_alerts (id, vip, needs_reply, primary_mail, updates, promotions, sign_in_codes, stay, sound, full_mail_loads_pictures) \
                       VALUES (1, 'banner', 'card', 'line', 'light', 'nothing', 'card', '10s', 0, 1)", []).is_err(), "a style outside the five words was stored");
}

#[test]
fn the_table_holds_one_row() {
    let mut c = at(32);
    migrate(&mut c).unwrap();
    c.execute(ROW, rusqlite::params![1, "5s"]).unwrap();
    assert!(c.execute(ROW, rusqlite::params![2, "5s"]).is_err(), "a second row was stored");
}

#[test]
fn the_reversal_gives_back_the_version_before() {
    let before = tables(&at(32));
    let mut c = at(32);
    migrate(&mut c).unwrap();
    assert!(tables(&c).contains(&"notch_alerts".to_string()));
    revert_to(&mut c, 32).unwrap();
    assert_eq!(tables(&c), before);
}
