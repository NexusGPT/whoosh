//! sync-bye-r2: the IDLE reconnect backoff after a connection that worked. Microsoft 365 ends an IDLE connection with
//! "* BYE Connection closed. 14" now and then; on a quiet mailbox no new mail arrives between two of them. The backoff
//! (SYNC_BACKOFF_START_MS doubled up to SYNC_BACKOFF_MAX_MS) exists for a server that takes IDLE and drops it at once. A
//! connection that held IDLE for longer than the longest wait (SYNC_BACKOFF_MAX_MS) was a working one: its loss is retried
//! at the first step again, so the status says "Reconnecting in 2 s", never 32 s after a day of quiet drops.
mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::*;
use sync_imap::*;
use tokio::sync::Notify;

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..500 {
        if ok() { return; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never reached: {what}");
}

fn waits(events: &Mutex<Vec<IdleEvent>>) -> Vec<u64> {
    events.lock().unwrap().iter().filter_map(|e| match e { IdleEvent::Lost { retry_in_ms, .. } => Some(*retry_in_ms), _ => None }).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_loss_after_a_long_healthy_idle_retries_at_the_first_step_while_quick_drops_still_double() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 1);
    let a = adapter(&srv, "m365");
    let table = a.connection_table().clone();
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let events: Arc<Mutex<Vec<IdleEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let h = tokio::spawn({
        let (a, clock, stop, events) = (a.clone(), clock.clone(), stop.clone(), events.clone());
        async move { a.idle_supervised("INBOX", &*clock, &stop, &mut move |e| events.lock().unwrap().push(e)).await }
    });
    // two drops at once: the server is not holding IDLE, the backoff doubles (the behaviour that stays)
    for n in 1..=2usize {
        srv.wait_for("IDLE", |s| s.idle_starts == n).await;
        srv.drop_idle();
        until("the loss", || waits(&events).len() == n).await;
        until("only the backoff timer", || clock.sleepers() == 1).await;
        clock.advance(Duration::from_millis(*waits(&events).last().unwrap()));
    }
    assert_eq!(waits(&events), vec![table.reconnect_delay(0).as_millis() as u64, table.reconnect_delay(1).as_millis() as u64], "quick drops double");
    // the third connection holds IDLE past the longest wait with no new mail, then the server ends it
    srv.wait_for("the third IDLE", |s| s.idle_starts == 3).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    clock.advance(Duration::from_millis(table.backoff_max_ms));
    tokio::time::sleep(Duration::from_millis(200)).await;
    srv.drop_idle();
    until("the third loss", || waits(&events).len() == 3).await;
    assert_eq!(waits(&events)[2], table.reconnect_delay(0).as_millis() as u64,
        "a connection that held IDLE past SYNC_BACKOFF_MAX_MS was a working one: retried at the first step ({:?})", waits(&events));
    assert!(!events.lock().unwrap().contains(&IdleEvent::Changed), "no new mail arrived: the reset is the healthy hold's, not a change's");
    stop.notify_one();
    h.await.unwrap().expect("stops cleanly");
}
