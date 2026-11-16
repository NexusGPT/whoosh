//! Migration 0002 on a 1M-message database (the shape core-store's tests/read_budget.rs builds: 30 accounts,
//! 4 messages per thread, six tabs, snoozes, pins). Built into a temp file at v1, given a 490-label tree and
//! 50 000 label assignments, then migrated. Ignored by default (it writes about 400 MB and takes a minute or two):
//!   cargo test -j 1 --test migration_v2_scale -- --ignored --nocapture
use rusqlite::{named_params, Connection};
use std::time::Instant;
use store_migrations::{migrate, MIGRATIONS};

const MESSAGES: u64 = 1_000_000;
const ACCOUNTS: u64 = 30;
const ROOTS: usize = 10;
/// Labels per root: 1 + 3 + 9 + 18 + 18 (levels 1 to 5)
const PER_ROOT: usize = 49;
const EVERY: u64 = 20; // every 20th message carries a label

fn build_v1_fixture(path: &std::path::Path) {
    let mut conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "OFF").unwrap();
    conn.pragma_update(None, "synchronous", "OFF").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    conn.execute_batch(MIGRATIONS[0].1).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute_batch(&format!(
        "WITH RECURSIVE s(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM s WHERE n < {a} - 1)
         INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms)
         SELECT 'acc-' || n, 'owner' || n || '@example.test', 'imap', 'Account ' || n, '#336699', n, 'manual', 'svc' || char(31) || n, 0 FROM s;
         WITH RECURSIVE s(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM s WHERE n < {t} - 1)
         INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms)
         SELECT 't' || n, 'acc-' || (n % {a}), 'Thread ' || n, '[]', 1700000000000 + n * 4000 FROM s;
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM s WHERE i < {m} - 1)
         INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, preview, received_at_ms, read, archived, folder)
         SELECT 'm' || i, 't' || (i / 4), 'acc-' || ((i / 4) % {a}), 'Sender ' || (i % 5000),
                CASE WHEN i % 10 = 9 THEN 'owner' || ((i / 4) % {a}) || '@example.test' ELSE 'sender' || (i % 5000) || '@ext.test' END,
                'Subject ' || i, 'Preview of message ' || i, 1700000000000 + i * 1000,
                CASE WHEN i % 5 = 0 THEN 0 ELSE 1 END, CASE WHEN i % 11 = 0 THEN 1 ELSE 0 END,
                CASE WHEN i % 10 = 9 THEN 'Sent' ELSE 'INBOX' END FROM s;
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM s WHERE i < {m} - 1)
         INSERT INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms)
         SELECT 'm' || i,
                CASE WHEN h < 40 THEN 'primary' WHEN h < 45 THEN 'needs-reply' WHEN h < 70 THEN 'updates'
                     WHEN h < 90 THEN 'promotions' WHEN h < 98 THEN 'social' ELSE 'vips' END,
                'default', NULL, NULL, 0, 0 FROM (SELECT i, (i * 2654435761 % 4294967296) % 100 AS h FROM s);
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 97 FROM s WHERE i < {m} - 97)
         INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) SELECT 'm' || i, 9000000000000, 0 FROM s;
         WITH RECURSIVE s(i) AS (SELECT 5 UNION ALL SELECT i + 1009 FROM s WHERE i < {m} - 1009)
         INSERT INTO pin (message_id, pinned_at_ms) SELECT 'm' || i, i FROM s;",
        a = ACCOUNTS, t = MESSAGES / 4, m = MESSAGES
    ))
    .unwrap();
    // labels in v1 shape (id, name, parent_id): per root 1 + 3 + 9 + 18 + 18, parents before children
    for r in 0..ROOTS {
        let base = r * PER_ROOT;
        let mut level: Vec<usize> = vec![base];
        let mut next = base + 1;
        tx.execute("INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES (?1, ?2, NULL, 0)", rusqlite::params![format!("lab-{base}"), format!("Root{r}")]).unwrap();
        for fan in [3usize, 3, 2, 1] {
            let mut deeper = Vec::new();
            for &p in &level {
                for k in 0..fan {
                    tx.execute("INSERT INTO label (id, name, parent_id, updated_at_ms) VALUES (?1, ?2, ?3, 0)", rusqlite::params![format!("lab-{next}"), format!("N{next}x{k}"), format!("lab-{p}")]).unwrap();
                    deeper.push(next);
                    next += 1;
                }
            }
            level = deeper;
        }
        assert_eq!(next - base, PER_ROOT, "tree shape");
    }
    tx.execute_batch(&format!(
        "WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + {e} FROM s WHERE i < {m} - {e})
         INSERT INTO label_assignment (label_id, message_id, updated_at_ms)
         SELECT 'lab-' || ((i / {e}) % {l}), 'm' || i, 0 FROM s;",
        e = EVERY, m = MESSAGES, l = ROOTS * PER_ROOT
    ))
    .unwrap();
    tx.commit().unwrap();
}

fn ledger_row(name: &str) -> Option<String> {
    let p = format!("{}/../../../../docs/CONSTANTS.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(p).unwrap();
    text.lines().find(|l| l.starts_with(&format!("| {name} |"))).map(|l| l.split('|').nth(2).unwrap().trim().to_string())
}

#[test]
#[ignore = "builds a 1M-message database (about 400 MB, one to two minutes)"]
fn v2_applies_to_one_million_messages_without_a_table_rewrite() {
    let path = std::env::temp_dir().join(format!("whoosh-migr-v2-scale-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let t = Instant::now();
    build_v1_fixture(&path);
    let size_before = std::fs::metadata(&path).unwrap().len();
    println!("fixture at v1: {MESSAGES} messages, {} labels, {} assignments, built in {:.1}s, file {} MB", ROOTS * PER_ROOT, MESSAGES / EVERY, t.elapsed().as_secs_f64(), size_before / 1_000_000);

    let mut conn = Connection::open(&path).unwrap();
    conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
    conn.pragma_update(None, "synchronous", "NORMAL").unwrap();
    let msgs_before: i64 = conn.query_row("SELECT count(*) FROM message", [], |r| r.get(0)).unwrap();
    let t = Instant::now();
    let applied = migrate(&mut conn).unwrap();
    let migrate_ms = t.elapsed().as_secs_f64() * 1000.0;
    assert_eq!((applied.from_version, applied.to_version), (1, 2));
    let size_after = std::fs::metadata(&path).unwrap().len();
    println!("migrate 1 -> 2 on {msgs_before} messages: {migrate_ms:.1} ms; file {} MB -> {} MB", size_before / 1_000_000, size_after / 1_000_000);
    match ledger_row("MIGRATION_MAX_MS") {
        Some(v) => {
            let max: f64 = v.parse().unwrap();
            assert!(migrate_ms <= max, "migration took {migrate_ms:.1} ms, ledger MIGRATION_MAX_MS is {max}");
            println!("budget MIGRATION_MAX_MS = {max}: met");
        }
        None => println!("docs/CONSTANTS.md has no MIGRATION_MAX_MS row; the time above is the measurement"),
    }
    let msgs_after: i64 = conn.query_row("SELECT count(*) FROM message", [], |r| r.get(0)).unwrap();
    assert_eq!(msgs_after, msgs_before, "no message lost");
    let unset: i64 = conn.query_row("SELECT count(*) FROM label WHERE path IS NULL OR depth IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(unset, 0, "backfill left labels without a path");
    let deepest: i64 = conn.query_row("SELECT max(depth) FROM label", [], |r| r.get(0)).unwrap();
    assert_eq!(deepest, 5);

    // the descendant query, against an oracle computed from the generator (not from the schema)
    let sql = "SELECT DISTINCT la.message_id FROM label_assignment la WHERE la.label_id IN (
        SELECT :id UNION ALL SELECT d.id FROM label t JOIN label d ON d.path >= t.path || ' > ' AND d.path < t.path || ' ?' WHERE t.id = :id)";
    for r in [0usize, 7] {
        let expect = (0..MESSAGES / EVERY).filter(|j| (*j as usize % (ROOTS * PER_ROOT)) / PER_ROOT == r).count();
        let t = Instant::now();
        let got: usize = conn.prepare(sql).unwrap().query_map(named_params! {":id": format!("lab-{}", r * PER_ROOT)}, |row| row.get::<_, String>(0)).unwrap().count();
        println!("messages under Root{r}: {got} (oracle {expect}) in {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(got, expect);
    }
    let first_page = format!("SELECT m.id FROM message m WHERE m.id IN ({sql}) ORDER BY m.received_at_ms DESC LIMIT 50");
    let plan: Vec<String> = conn.prepare(&format!("EXPLAIN QUERY PLAN {first_page}")).unwrap().query_map(named_params! {":id": "lab-0"}, |row| row.get::<_, String>(3)).unwrap().map(Result::unwrap).collect();
    println!("plan of the newest-50 query:\n  {}", plan.join("\n  "));
    let mut runs = Vec::new();
    for _ in 0..5 {
        let t = Instant::now();
        let page: usize = conn.prepare(&first_page).unwrap().query_map(named_params! {":id": "lab-0"}, |row| row.get::<_, String>(0)).unwrap().count();
        assert_eq!(page, 50);
        runs.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("newest 50 messages under Root0, 5 runs (ms): {:?}", runs.iter().map(|x| (x * 10.0).round() / 10.0).collect::<Vec<_>>());

    // renaming a root rewrites its 49 labels
    let t = Instant::now();
    conn.execute("UPDATE label SET name = 'Renamed' WHERE id = 'lab-0'", []).unwrap();
    println!("rename of Root0 (rewrites {PER_ROOT} labels): {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    let bad: i64 = conn.query_row("SELECT count(*) FROM label WHERE path = 'Root0' OR path LIKE 'Root0 > %'", [], |r| r.get(0)).unwrap();
    assert_eq!(bad, 0, "no path may still start with the old name");
    let renamed: i64 = conn.query_row("SELECT count(*) FROM label WHERE path = 'Renamed' OR path LIKE 'Renamed > %'", [], |r| r.get(0)).unwrap();
    assert_eq!(renamed as usize, PER_ROOT);
    drop(conn);
    std::fs::remove_file(&path).unwrap();
}
