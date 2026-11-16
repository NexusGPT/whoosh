//! fn-migration-v1 and fn-account-icon-data acceptance. The expected table list is read from
//! docs/DATA-MAP.md (last column, "table / migration"), so a record added to the data map without a
//! table fails here, and a table with no record fails too. No count is typed in this file.
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, BTreeSet};
use store_migrations::{migrate, schema_hash, schema_text, schema_version, table_names, MigrationError};

fn data_map_text() -> String {
    let p = format!("{}/../../../../docs/DATA-MAP.md", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {p}: {e}"))
}

struct Expect {
    /// record (column 1) -> tables its last cell names
    records: BTreeMap<String, Vec<String>>,
    /// (table, column) pairs the last cell names, like account.icon_id
    columns: BTreeSet<(String, String)>,
}

/// One data-map row = a line `| record | ... | table / migration |`. The last cell reads
/// `<table> / v1`, or `none (<where>); <thing> / v1`, or `a.col, other_table / v1`.
fn expect() -> Expect {
    let mut records = BTreeMap::new();
    let mut columns = BTreeSet::new();
    for line in data_map_text().lines().filter(|l| l.starts_with("| ") && !l.starts_with("| record ") && !l.starts_with("|---")) {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        let name = cells[1].to_string();
        let last = cells[cells.len() - 2];
        let body = last.rsplit_once(" / ").filter(|(_, m)| *m == "v1" || (m.len() == 4 && m.chars().all(|c| c.is_ascii_digit()))).map(|(b, _)| b)
            .unwrap_or_else(|| panic!("{name}: last cell {last:?} does not end in ' / v1' or ' / 000N'"));
        let mut tables = Vec::new();
        for part in body.split(';').map(str::trim) {
            if part.starts_with("none") {
                continue;
            }
            for piece in part.split(',').map(str::trim) {
                let first = piece.split_whitespace().next().unwrap_or("");
                if let Some((t, c)) = first.split_once('.') {
                    columns.insert((t.to_string(), c.to_string()));
                } else if !first.is_empty() && first.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                    tables.push(first.to_string());
                } else {
                    panic!("{name}: cannot read table from {piece:?} in {last:?}");
                }
            }
        }
        records.insert(name, tables);
    }
    Expect { records, columns }
}

fn fresh() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    migrate(&mut c).unwrap();
    c
}

fn columns_of(c: &Connection, table: &str) -> Vec<String> {
    let mut s = c.prepare(&format!("PRAGMA table_info({table})")).unwrap();
    s.query_map([], |r| r.get::<_, String>(1)).unwrap().map(Result::unwrap).collect()
}

#[test]
fn v1_creates_exactly_the_data_map_tables() {
    let e = expect();
    let want: BTreeSet<String> = e.records.values().flatten().cloned().collect();
    let c = fresh();
    let got: BTreeSet<String> = table_names(&c).unwrap().into_iter().collect();
    let missing: Vec<_> = want.difference(&got).collect();
    let extra: Vec<_> = got.difference(&want).collect();
    assert!(missing.is_empty(), "DATA-MAP names tables the migration does not create: {missing:?}");
    assert!(extra.is_empty(), "the migration creates tables the DATA-MAP does not name: {extra:?}");
    let none: Vec<_> = e.records.iter().filter(|(_, t)| t.is_empty()).map(|(n, _)| n.as_str()).collect();
    println!("records: {} · with a table: {} · Keychain or relay only: {} {:?} · tables created: {}", e.records.len(), e.records.len() - none.len(), none.len(), none, got.len());
}

#[test]
fn every_column_the_data_map_names_exists() {
    let e = expect();
    let c = fresh();
    for (t, col) in &e.columns {
        assert!(columns_of(&c, t).contains(col), "DATA-MAP names {t}.{col}; the table has no such column");
    }
    assert!(!e.columns.is_empty(), "the data map names no table.column cell; the parser read nothing");
}

#[test]
fn second_run_changes_nothing() {
    let mut c = fresh();
    let (before_hash, before_text) = (schema_hash(&c).unwrap(), schema_text(&c).unwrap());
    let again = migrate(&mut c).unwrap();
    assert!(!again.changed_anything(), "a second run applied {again:?}");
    assert_eq!(schema_hash(&c).unwrap(), before_hash);
    assert_eq!(schema_text(&c).unwrap(), before_text);
    assert_eq!(schema_version(&c).unwrap(), store_migrations::latest_version());
}

#[test]
fn file_database_survives_close_and_reopen() {
    let dir = std::env::temp_dir().join(format!("whoosh-migr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("whoosh.db");
    let _ = std::fs::remove_file(&path);
    let first = {
        let mut c = Connection::open(&path).unwrap();
        let a = migrate(&mut c).unwrap();
        assert!(a.changed_anything());
        schema_hash(&c).unwrap()
    };
    let mut c = Connection::open(&path).unwrap();
    assert!(!migrate(&mut c).unwrap().changed_anything());
    assert_eq!(schema_hash(&c).unwrap(), first);
    drop(c);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_newer_database_is_refused_not_downgraded() {
    let mut c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "user_version", store_migrations::latest_version() + 1).unwrap();
    match migrate(&mut c) {
        Err(MigrationError::NewerSchema { .. }) => {}
        other => panic!("expected NewerSchema, got {other:?}"),
    }
    assert!(table_names(&c).unwrap().is_empty(), "a refused database must stay untouched");
}

#[test]
fn no_raw_secret_column_and_one_jev_key_reference() {
    let c = fresh();
    let schema = schema_text(&c).unwrap();
    // The CLI's `.schema` prints the same stored statements; the lines below are the same lines.
    let lower = schema.to_lowercase();
    for banned in ["password", "token", "apikey"] {
        assert!(!lower.contains(banned), "schema text contains {banned:?}");
    }
    let key_ref_lines = schema.lines().filter(|l| l.to_lowercase().contains("key_ref")).count();
    assert_eq!(key_ref_lines, 1, "exactly the decision model's Jev key reference may match key_ref");
    assert!(columns_of(&c, "decision_model").contains(&"jev_api_key_ref".to_string()));
    for t in ["account", "ai_provider", "device_link"] {
        assert!(columns_of(&c, t).contains(&"keychain_ref".to_string()), "{t} needs its keychain_ref column");
    }
}

#[test]
fn account_icon_and_decision_data_exist() {
    let c = fresh();
    let tables: BTreeSet<String> = table_names(&c).unwrap().into_iter().collect();
    for t in ["split_tab", "decision_model", "account_icon_file"] {
        assert!(tables.contains(t), "missing table {t}");
    }
    assert!(columns_of(&c, "account").contains(&"icon_id".to_string()));
    assert!(columns_of(&c, "message_category").contains(&"decided_by".to_string()));
    assert!(tables.contains("folder_sync_state"), "gap G8: the per-folder sync cursor table");
}

const SEP: &str = "\u{1f}";

fn add_account(c: &Connection, id: &str) {
    c.execute(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms)
         VALUES (?1, ?2, 'google', 'Work', '#336699', 0, 'oauth', ?3, 1)",
        params![id, format!("{id}@example.test"), format!("whoosh{SEP}{id}")],
    )
    .unwrap();
}

fn add_message(c: &Connection, id: &str, account: &str) {
    c.execute("INSERT INTO thread (id, account_id, subject) VALUES (?1, ?2, 's')", params![format!("t-{id}"), account]).unwrap();
    c.execute(
        "INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder)
         VALUES (?1, ?2, ?3, 'A', 'a@example.test', 's', 1, 'INBOX')",
        params![id, format!("t-{id}"), account],
    )
    .unwrap();
}

#[test]
fn the_real_statements_hold_their_rules() {
    let c = fresh();
    add_account(&c, "a1");
    add_message(&c, "m1", "a1");

    // a credential must be a Keychain reference, never a bare secret
    let bare = c.execute(
        "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms)
         VALUES ('a2','a2@example.test','imap','X','#000000',1,'manual','plainsecret',1)",
        [],
    );
    assert!(bare.is_err(), "a keychain_ref without the separator must be refused");

    // a category decided by a rule names the rule; one decided by anyone else does not
    let rule_without_id = c.execute("INSERT INTO message_category (message_id, tab_id, decided_by, decided_at_ms, updated_at_ms) VALUES ('m1','primary','rule',1,1)", []);
    assert!(rule_without_id.is_err());
    c.execute("INSERT INTO message_category (message_id, tab_id, decided_by, probability, decided_at_ms, updated_at_ms) VALUES ('m1','updates','jev',0.91,1,1)", []).unwrap();

    // a failed outbox entry always carries the words the owner reads
    let failed_silent = c.execute("INSERT INTO outbox_entry (id, draft_json, state, created_at_ms, updated_at_ms) VALUES ('o1','{}','failed',1,1)", []);
    assert!(failed_silent.is_err());
    c.execute("INSERT INTO outbox_entry (id, draft_json, state, due_at_ms, created_at_ms, updated_at_ms) VALUES ('o2','{}','undoable',9,1,1)", []).unwrap();

    // a snooze always has a return time
    assert!(c.execute("INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) VALUES ('m1', NULL, 1)", []).is_err());

    // each cursor kind carries only its own fields
    c.execute("INSERT INTO folder_sync_state (account_id, folder, cursor_kind, imap_uid_validity, imap_highest_mod_seq, last_uid) VALUES ('a1','INBOX','imap',7,99,42)", []).unwrap();
    c.execute("INSERT INTO folder_sync_state (account_id, folder, cursor_kind, cursor_value) VALUES ('a1','Archive','graph_delta','d1')", []).unwrap();
    assert!(c.execute("INSERT INTO folder_sync_state (account_id, folder, cursor_kind, cursor_value, imap_uid_validity) VALUES ('a1','Sent','jmap','s',1)", []).is_err());

    // an orphan message is refused (foreign keys are on); removing the account removes its mail
    assert!(c.execute("INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, received_at_ms, folder) VALUES ('m9','tx','nope','A','a@x','s',1,'INBOX')", []).is_err());
    c.execute("DELETE FROM account WHERE id = 'a1'", []).unwrap();
    let left: i64 = c.query_row("SELECT (SELECT count(*) FROM message) + (SELECT count(*) FROM message_category) + (SELECT count(*) FROM folder_sync_state)", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);

    // the decision model is one row, and its Jev key is a reference
    c.execute("INSERT INTO decision_model (id, provider, jev_api_key_ref, updated_at_ms) VALUES (1,'jev',?1,1)", params![format!("whoosh.jev{SEP}key")]).unwrap();
    assert!(c.execute("INSERT INTO decision_model (id, provider, updated_at_ms) VALUES (2,'jev',1)", []).is_err());
    assert!(c.execute("UPDATE decision_model SET jev_api_key_ref = 'sk-live-raw' WHERE id = 1", []).is_err());
}
