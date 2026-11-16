//! The read budget on a generated 1M-message database (built here into a temp file, never committed).
//! Budget: STORE_READ_P95_MS from docs/CONSTANTS.md. That row is not in the ledger at this commit, so the
//! test falls back to the ledger's COMMIT_MS / INTERACTION_COMMIT_MS (50) and says so in its output.
mod common;
use common::*;
use contracts::{CoreEvent, InboxQuery, StreamKind};
use rusqlite::Connection;
use std::sync::Arc;
use std::time::Instant;

use common::{build_fixture, ACCOUNTS, MESSAGES};
const SAMPLES: usize = 300;
const RARE_SAMPLES: usize = 20;

fn p95(mut ms: Vec<f64>) -> f64 {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ms[((ms.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]
}

fn sample(store: &store::Store, tab: &str, n: usize) -> Vec<f64> {
    (0..n)
        .map(|_| {
            let t = Instant::now();
            let rows = store.inbox(&InboxQuery { tab_id: tab.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(rows.len(), 50, "the page is full");
            ms
        })
        .collect()
}

#[test]
fn primary_first_50_p95_is_under_the_budget_on_one_million_messages() {
    let (budget, source) = match constant("STORE_READ_P95_MS") {
        Some(v) => (v, "STORE_READ_P95_MS"),
        None => (constant("COMMIT_MS").expect("the ledger has COMMIT_MS / INTERACTION_COMMIT_MS"), "COMMIT_MS / INTERACTION_COMMIT_MS (STORE_READ_P95_MS is not in docs/CONSTANTS.md)"),
    };
    let db = TempDb::new("million");
    let t = Instant::now();
    build_fixture(&db.0);
    println!("fixture: {MESSAGES} messages over {ACCOUNTS} accounts built in {:.1}s, file {} MB", t.elapsed().as_secs_f64(), std::fs::metadata(&db.0).unwrap().len() / 1_000_000);

    let t = Instant::now();
    let store = store::Store::open(&db.0).unwrap();
    println!("open (WAL switch, seed tabs, reader): {:.2}s", t.elapsed().as_secs_f64());

    // The query plan: the per-account walk must use the ordered index and need no sort.
    {
        let c = Connection::open_with_flags(&db.0, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut st = c.prepare(&format!("EXPLAIN QUERY PLAN {}", store::read::inbox_walk_sql())).unwrap();
        let plan: Vec<String> = st.query_map(rusqlite::params!["primary", "primary", "acc-0", Option::<String>::None, 50], |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
        println!("plan: {plan:#?}");
        assert!(plan.iter().any(|p| p.contains("message_by_folder")), "the walk uses message_by_folder: {plan:?}");
        assert!(!plan.iter().any(|p| p.contains("TEMP B-TREE FOR ORDER BY")), "the walk needs no sort: {plan:?}");
    }

    {
        let c = Connection::open_with_flags(&db.0, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut st = c.prepare(&format!("EXPLAIN QUERY PLAN {}", store::read::inbox_pinned_sql())).unwrap();
        let plan: Vec<String> = st.query_map(rusqlite::params!["primary", "primary", Option::<String>::None, Option::<String>::None, 50], |r| r.get::<_, String>(3)).unwrap().map(|r| r.unwrap()).collect();
        println!("pinned plan: {plan:#?}");
        assert!(!plan.iter().any(|p| p == "SCAN m"), "the pinned query must not walk every message: {plan:?}");
        assert!(plan.iter().any(|p| p.starts_with("SCAN p")), "the pinned query drives from pin: {plan:?}");
    }
    let _ = store.inbox(&InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap(); // warm the page cache like a running app
    let primary = sample(&store, "primary", SAMPLES);
    let vips = sample(&store, "vips", RARE_SAMPLES);
    let (pp, vp) = (p95(primary.clone()), p95(vips));
    println!("PRIMARY first-50 over {SAMPLES} runs: p95 = {pp:.2} ms, max = {:.2} ms; VIPS (2% of mail, {RARE_SAMPLES} runs) p95 = {vp:.2} ms; budget = {budget} ms from {source}", primary.iter().cloned().fold(0.0, f64::max));
    assert!(pp < budget as f64, "Primary first-50 p95 {pp:.2} ms is not under {budget} ms");

    // One flag write at 1M: one inbox event, one counts event, and the counts value equals a recount.
    let t = Instant::now();
    let first = store.counts().unwrap();
    println!("first counts() (tally built from scratch on the writer): {:.2}s", t.elapsed().as_secs_f64());
    let sink = Arc::new(Collector::default());
    store.subscribe(StreamKind::Inbox, sink.clone());
    store.subscribe(StreamKind::Counts, sink.clone());
    // m10: unread (i % 5 == 0), not archived, in the inbox folder, not snoozed.
    let target = "m10".to_string();
    let t = Instant::now();
    store.set_read(vec![target.clone()], true).unwrap();
    println!("one flag write incl. both events: {:.2} ms", t.elapsed().as_secs_f64() * 1000.0);
    let events = sink.take();
    let inbox = events.iter().filter(|e| matches!(e, CoreEvent::InboxChanged { .. })).count();
    let counts = events.iter().filter(|e| matches!(e, CoreEvent::CountsChanged { .. })).count();
    assert_eq!((inbox, counts), (2, 1), "events: {} total (the tab and the All view, delta 16)", events.len());
    let after = store.counts().unwrap();
    assert_ne!(first, after, "the write moved the counts");
    let t = Instant::now();
    assert_eq!(after, store.recount().unwrap(), "the tally equals a recount at 1M");
    println!("recount from scratch at 1M: {:.2}s", t.elapsed().as_secs_f64());
}
