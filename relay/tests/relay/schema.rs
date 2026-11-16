//! The store holds device tokens and opaque routing ids, never a credential or message content.
//! The allowlist below is written from the requirement, not read from the schema file.
use crate::common::*;
use rusqlite::{Connection, OpenFlags};
use std::collections::BTreeMap;

const ALLOWED: &[(&str, &[&str])] = &[
    ("push_job", &["apns_reason", "apns_status", "device_token", "enqueued_at", "environment", "finished_at", "job_id", "opaque_ref", "routing_id", "signal_id", "state"]),
    ("registration", &["device_token", "environment", "registered_at", "routing_id"]),
    ("tracked_message", &["created_at", "token"]),
    ("tracking_event", &["event_id", "kind", "occurred_at", "token"]),
];
const FORBIDDEN_WORDS: &[&str] = &[
    "password", "passwd", "secret", "credential", "oauth", "refresh", "access", "auth", "subject", "body", "snippet", "preview", "sender", "from", "recipient", "address", "email", "content", "text", "html", "url", "ip_", "agent",
];

fn schema_of(db: &std::path::Path) -> BTreeMap<String, Vec<String>> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).expect("open relay database read-only");
    let tables: Vec<String> = c
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    tables
        .into_iter()
        .map(|t| {
            let mut cols: Vec<String> = c.prepare(&format!("PRAGMA table_info({t})")).unwrap().query_map([], |r| r.get::<_, String>(1)).unwrap().collect::<Result<_, _>>().unwrap();
            cols.sort();
            (t, cols)
        })
        .collect()
}

#[tokio::test]
async fn schema_has_no_credential_or_message_content_column() {
    let r = start_relay(&work_dir("schema"), None);
    r.get("/v1/health").await;
    let schema = schema_of(&r.db);
    let expected: BTreeMap<String, Vec<String>> = ALLOWED.iter().map(|(t, c)| (t.to_string(), c.iter().map(|s| s.to_string()).collect())).collect();
    assert_eq!(schema, expected, "the store holds exactly the allowlisted tables and columns");
    for (table, cols) in &schema {
        for col in cols {
            for word in FORBIDDEN_WORDS {
                assert!(!col.contains(word), "column {table}.{col} looks like a credential or content column ({word})");
            }
        }
    }
}

#[test]
fn the_forbidden_word_scan_catches_a_planted_column() {
    // Control for the scan above: a credential column must be caught by the same rule.
    assert!(FORBIDDEN_WORDS.iter().any(|w| "mailbox_password".contains(w)));
    assert!(FORBIDDEN_WORDS.iter().any(|w| "subject_line".contains(w)));
    assert!(!FORBIDDEN_WORDS.iter().any(|w| "device_token".contains(w)));
}
