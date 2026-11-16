//! Kill and relaunch: the backfill resumes from the stored anchors and no message is fetched twice.
mod support;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::time::Duration;
use support::*;
use sync_engine::{Engine, Outcome};

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_killed_run_resumes_from_the_stored_cursor_and_fetches_no_message_twice() {
    let f = fixture(false);
    *f.log.kill_after.lock().unwrap() = Some(60);
    let store = open_store(&f);
    let engine = Engine::new(store.clone(), f.events.clone(), Arc::new(TestClock), config());
    let sources = sources(&f);
    let first = tokio::spawn(async move { engine.run(sources).await });
    tokio::time::timeout(Duration::from_secs(60), f.log.killed.notified()).await.expect("the run never reached the kill point");
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    // A killed process writes nothing after the relaunch: wait until every handle of the first store is gone
    // (its in-flight writes finished), then close it, which joins its writer thread.
    tokio::time::timeout(Duration::from_secs(60), async {
        while Arc::strong_count(&store) > 1 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the first run still held the store 60 s after the kill");
    drop(store);
    let before_restart: i64 = db(&f.log.db_path).query_row("SELECT count(*) FROM message", [], |r| r.get(0)).unwrap();
    assert!(before_restart > 0 && before_restart < 29 * PER_ACCOUNT, "the kill landed mid-backfill ({before_restart} stored)");

    *f.log.kill_after.lock().unwrap() = None;
    let store = open_store(&f);
    let out = run(&f, store).await;
    assert_eq!(out.values().filter(|o| **o == Outcome::Complete).count(), 29);
    let fetched = f.log.fetched.lock().unwrap().clone();
    let twice: Vec<_> = fetched.iter().filter(|(_, n)| **n > 1).map(|(id, n)| format!("{id} x{n}")).collect();
    assert!(twice.is_empty(), "{} messages fetched more than once, e.g. {:?}", twice.len(), &twice[..twice.len().min(5)]);
    assert_eq!(fetched.len() as i64, 29 * PER_ACCOUNT, "every message fetched exactly once across the two runs");
    let stored: i64 = db(&f.log.db_path).query_row("SELECT count(*) FROM message", [], |r| r.get(0)).unwrap();
    assert_eq!(stored, 29 * PER_ACCOUNT);
    assert!(f.log.backfill_pages.load(SeqCst) > 60);
}
