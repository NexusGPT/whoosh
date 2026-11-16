//! The label subtree read on the generated 1M-message database: the newest page under a root label that holds about five
//! thousand messages, sub-labels included, answered warm inside the budget. Built into a temp file, never committed.
//! Named "million" so the workspace run (`--skip million`) leaves it out; the capture in _captures/core-store/ is its run.
mod common;
use common::*;
use rusqlite::Connection;
use std::time::Instant;

const ROOTS: usize = 10;
/// Labels per root: 1 + 3 + 9 + 18 + 18 (levels 1 to 5), the migrations lane's shape.
const PER_ROOT: usize = 49;
const EVERY: u64 = 20; // every 20th message carries a label

fn add_labels(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "OFF").unwrap();
    conn.pragma_update(None, "synchronous", "OFF").unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    let tx = conn.unchecked_transaction().unwrap();
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

#[test]
fn newest_50_under_a_5018_message_root_label_is_inside_the_budget_on_one_million_messages() {
    let (budget_ms, source) = match constant("LABEL_SUBTREE_PAGE_MS") {
        Some(v) => (v as f64, "LABEL_SUBTREE_PAGE_MS"),
        None => (20.0, "the lane brief's 20 ms (LABEL_SUBTREE_PAGE_MS is not in docs/CONSTANTS.md)"),
    };
    let db = TempDb::new("million-labels");
    let t = Instant::now();
    build_fixture(&db.0);
    add_labels(&db.0);
    println!("fixture: {MESSAGES} messages, {} labels, {} assignments, built in {:.1}s, file {} MB", ROOTS * PER_ROOT, MESSAGES / EVERY, t.elapsed().as_secs_f64(), std::fs::metadata(&db.0).unwrap().len() / 1_000_000);
    let store = store::Store::open(&db.0).unwrap();

    // Oracle from the generator, not from the schema: assignment j goes to label (j % 490); a root owns labels r*49 .. r*49+48.
    let expect_root0 = (0..MESSAGES / EVERY).filter(|j| (*j as usize % (ROOTS * PER_ROOT)) / PER_ROOT == 0).count();
    println!("messages under Root0 by the generator: {expect_root0}");
    assert_eq!(expect_root0, 5018);

    {
        let c = Connection::open_with_flags(&db.0, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut st = c.prepare(&format!("EXPLAIN QUERY PLAN {}", store::read::messages_under_label_sql())).unwrap();
        let plan: Vec<String> = st.query_map(rusqlite::params!["lab-0", 50], |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
        println!("plan:\n  {}", plan.join("\n  "));
        assert!(plan.iter().any(|p| p.contains("label_by_path")), "the subtree lookup uses label_by_path: {plan:?}");
        assert!(!plan.iter().any(|p| p == "SCAN m" || p == "SCAN message"), "the read must not walk every message: {plan:?}");
    }

    let first = Instant::now();
    let rows = store.messages_under_label("lab-0", 50).unwrap();
    println!("first (cold) run: {:.1} ms", first.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(rows.len(), 50);
    let mut runs = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let rows = store.messages_under_label("lab-0", 50).unwrap();
        runs.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(rows.len(), 50);
    }
    let mut sorted = runs.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95 = sorted[((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
    println!("newest 50 under Root0 (5018 messages), 20 warm runs: p95 = {p95:.2} ms, max = {:.2} ms, budget = {budget_ms} ms from {source}", sorted.last().unwrap());
    let newest: Vec<i64> = rows.iter().map(|r| r.received_at_ms).collect();
    assert!(newest.windows(2).all(|w| w[0] >= w[1]), "newest first");
    // Every row really carries Root0 or a label below it (the oracle: label number 0..48).
    let all = store.write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM label_assignment WHERE label_id IN (SELECT id FROM label WHERE id = 'lab-0' OR path LIKE 'Root0 > %')", [], |r| r.get::<_, i64>(0))?)).unwrap();
    assert_eq!(all as usize, expect_root0, "the subtree of Root0 holds the generator's count");
    assert!(p95 < budget_ms, "newest-50 under a label took p95 {p95:.2} ms, budget {budget_ms} ms");
}
