//! Migration 0008 (contracts delta 14): the detection source word 'mx' on account.detection_source and detection_cache.source,
//! every existing value carried forward, and the reversal to 0007 (its loss: the 'mx' notes and cache rows).
use rusqlite::Connection;
use store_migrations::{migrate, revert_to, MIGRATIONS};

const ACCOUNT: &str = "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms, detection_source) VALUES (?1, ?2, 'imap', 'A', '#336699', 0, 'manual', 'svc' || char(31) || ?1, 0, ?3)";
const CACHE: &str = "INSERT INTO detection_cache (domain, outcome, source, config_json, refusal_words, detected_at_ms) VALUES (?1, 'found', ?2, '{}', NULL, 1)";

fn at(version: usize) -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", true).unwrap();
    for (_, sql) in &MIGRATIONS[..version] { c.execute_batch(sql).unwrap(); }
    c.pragma_update(None, "user_version", version as i64).unwrap();
    c
}
fn source_of(c: &Connection, id: &str) -> Option<String> { c.query_row("SELECT detection_source FROM account WHERE id = ?1", [id], |r| r.get(0)).unwrap() }
fn cols(c: &Connection, t: &str) -> Vec<String> { c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect() }

#[test]
fn mx_is_accepted_and_every_older_value_is_carried_forward() {
    let mut c = at(7);
    c.execute(ACCOUNT, rusqlite::params!["a1", "a@x.test", "srv"]).unwrap();
    c.execute(ACCOUNT, rusqlite::params!["a2", "b@x.test", Option::<String>::None]).unwrap();
    c.execute(CACHE, rusqlite::params!["x.test", "autodiscover"]).unwrap();
    assert!(c.execute(ACCOUNT, rusqlite::params!["a3", "c@x.test", "mx"]).is_err(), "0007 refuses mx: the reason for 0008");
    migrate(&mut c).unwrap();
    assert_eq!((source_of(&c, "a1"), source_of(&c, "a2")), (Some("srv".to_string()), None));
    let cached: String = c.query_row("SELECT source FROM detection_cache WHERE domain = 'x.test'", [], |r| r.get(0)).unwrap();
    assert_eq!(cached, "autodiscover");
    c.execute(ACCOUNT, rusqlite::params!["a3", "c@x.test", "mx"]).unwrap();
    c.execute(CACHE, rusqlite::params!["nexus-example.com", "mx"]).unwrap();
    assert!(c.execute(ACCOUNT, rusqlite::params!["a4", "d@x.test", "nope"]).is_err(), "the list still refuses an unknown word");
    assert!(c.execute("INSERT INTO detection_cache (domain, outcome, source, config_json, refusal_words, detected_at_ms) VALUES ('r.test','refused','mx',NULL,'no',1)", []).is_err(),
            "the table CHECK still ties a refusal to no source");
    assert_eq!(cols(&c, "account").last().map(String::as_str), Some("detection_source"), "the column stays the table's last");
}

#[test]
fn the_reversal_gives_back_0007_and_loses_only_the_mx_notes() {
    let v7 = at(7);
    let mut c = at(7);
    migrate(&mut c).unwrap();
    c.execute(ACCOUNT, rusqlite::params!["a1", "a@x.test", "srv"]).unwrap();
    c.execute(ACCOUNT, rusqlite::params!["a3", "c@x.test", "mx"]).unwrap();
    c.execute(CACHE, rusqlite::params!["x.test", "autodiscover"]).unwrap();
    c.execute(CACHE, rusqlite::params!["nexus-example.com", "mx"]).unwrap();
    let applied = revert_to(&mut c, 7).unwrap();
    assert_eq!(applied.to_version, 7);
    assert_eq!(cols(&c, "account"), cols(&v7, "account"));
    assert_eq!(cols(&c, "detection_cache"), cols(&v7, "detection_cache"));
    assert_eq!((source_of(&c, "a1"), source_of(&c, "a3")), (Some("srv".to_string()), None), "the mx note is the one loss on the account");
    let domains: Vec<String> = c.prepare("SELECT domain FROM detection_cache ORDER BY domain").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(domains, ["x.test"], "the mx cache row goes, the others stay");
    assert!(c.execute(ACCOUNT, rusqlite::params!["a5", "e@x.test", "mx"]).is_err(), "0007's list is back");
}
