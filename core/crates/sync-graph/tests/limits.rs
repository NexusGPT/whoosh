mod support;
use std::sync::{atomic::{AtomicUsize, Ordering::SeqCst}, Arc};

use contracts::{Clock, CoreError};
use support::*;
use sync_graph::{limits, Limits, Throttle};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_requests_per_mailbox_never_exceed_the_documented_cap() {
    let stub = start(150, |_| Resp::ok(fixture("delta_empty.json"))).await;
    let r = rig(&stub.base, 50, 3);
    let handles: Vec<_> = (0..limits::GRAPH_CONCURRENT * 3)
        .map(|i| {
            let mb = r.mailbox.clone();
            tokio::spawn(async move { mb.delta_round(&format!("folder-{i}"), "F", None).await })
        })
        .collect();
    for h in handles {
        h.await.expect("task").expect("round");
    }
    let seen = stub.max_inflight.load(SeqCst);
    assert!(seen <= limits::GRAPH_CONCURRENT, "{seen} requests were in flight at once; the cap is {}", limits::GRAPH_CONCURRENT);
    assert_eq!(seen, limits::GRAPH_CONCURRENT, "the load must actually reach the cap, or the bound proves nothing");
    assert_eq!(stub.requests().len(), limits::GRAPH_CONCURRENT * 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn each_mailbox_has_its_own_cap() {
    let stub = start(150, |_| Resp::ok(fixture("delta_empty.json"))).await;
    let (a, b) = (rig(&stub.base, 50, 3), rig(&stub.base, 50, 3));
    let handles: Vec<_> = (0..limits::GRAPH_CONCURRENT * 2)
        .map(|i| {
            let mb = if i % 2 == 0 { a.mailbox.clone() } else { b.mailbox.clone() };
            tokio::spawn(async move { mb.delta_round(&format!("folder-{i}"), "F", None).await })
        })
        .collect();
    for h in handles {
        h.await.expect("task").expect("round");
    }
    let seen = stub.max_inflight.load(SeqCst);
    assert!(seen <= 2 * limits::GRAPH_CONCURRENT && seen > limits::GRAPH_CONCURRENT, "two mailboxes may overlap beyond one cap: saw {seen}");
}

#[test]
fn the_default_throttle_uses_the_numbers_from_constants_md() {
    let clock = Arc::new(FakeClock(Default::default()));
    let sleeper = Arc::new(RecSleeper { clock: clock.clone(), slept: Default::default() });
    let t = Throttle::new(clock, sleeper);
    assert_eq!(t.limits(), Limits::documented());
    assert_eq!(t.limits().concurrent, limits::GRAPH_CONCURRENT);
    assert_eq!(t.limits().per_window, limits::GRAPH_REQ_PER_10MIN);
    assert_eq!(t.limits().window_ms, limits::GRAPH_WINDOW_MS);
}

#[tokio::test]
async fn the_window_makes_the_next_request_wait_for_the_oldest_to_age_out() {
    let clock = Arc::new(FakeClock(Default::default()));
    let sleeper = Arc::new(RecSleeper { clock: clock.clone(), slept: Default::default() });
    let t = Throttle::with_limits(Limits { concurrent: 2, per_window: 3, window_ms: 1_000 }, clock.clone(), sleeper.clone());
    for _ in 0..3 {
        drop(t.acquire().await);
        clock.0.fetch_add(100, SeqCst);
    }
    assert!(sleeper.slept().is_empty(), "three starts fit the window");
    drop(t.acquire().await);
    assert_eq!(sleeper.slept(), vec![700], "the fourth waits until the first (started at 0) is a window old");
    assert_eq!(clock.now_ms(), 1_000);
}

#[tokio::test]
async fn a_pause_holds_every_request_of_the_mailbox() {
    let clock = Arc::new(FakeClock(Default::default()));
    let sleeper = Arc::new(RecSleeper { clock: clock.clone(), slept: Default::default() });
    let t = Throttle::new(clock, sleeper.clone());
    t.pause_for(7_000);
    drop(t.acquire().await);
    assert_eq!(sleeper.slept(), vec![7_000]);
}

#[tokio::test]
async fn a_429_with_retry_after_backs_off_and_says_slowed_by_microsoft() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let stub = start(0, move |_| {
        if c.fetch_add(1, SeqCst) == 0 {
            Resp::status(429, fixture("throttle_429.json")).header("Retry-After", "7")
        } else {
            Resp::ok(fixture("delta_empty.json"))
        }
    })
    .await;
    let r = rig(&stub.base, 50, 3);
    r.mailbox.delta_round("f", "F", None).await.expect("succeeds after the pause");
    let slowed = r.slowed.lock().unwrap().clone();
    assert_eq!(slowed.len(), 1);
    assert!(slowed[0].reason_words.contains("slowed by Microsoft"), "{}", slowed[0].reason_words);
    assert!(slowed[0].reason_words.contains("7 s"));
    assert!(slowed[0].retryable);
    assert_eq!(slowed[0].account_name, ACCOUNT_NAME);
    assert_eq!(r.sleeper.slept(), vec![7_000]);
    assert_eq!(stub.requests().len(), 2);
}

#[tokio::test]
async fn a_429_without_retry_after_backs_off_exponentially_then_fails_in_words() {
    let stub = start(0, |_| Resp::status(429, fixture("throttle_429.json"))).await;
    let r = rig(&stub.base, 50, 3);
    let err = r.mailbox.delta_round("f", "F", None).await.unwrap_err();
    match err {
        CoreError::Action { failure } => {
            assert!(failure.reason_words.contains("slowed by Microsoft"), "{}", failure.reason_words);
            assert!(failure.retryable);
            assert_eq!(failure.account_name, ACCOUNT_NAME);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(stub.requests().len(), 3);
    assert_eq!(r.sleeper.slept(), vec![1_000, 2_000]);
    assert_eq!(r.slowed.lock().unwrap().len(), 2, "the owner was told before each retry, not after the last");
}

#[tokio::test]
async fn a_503_is_retried_and_other_errors_carry_the_real_body() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let stub = start(0, move |req| {
        if req.target.contains("folder-401") {
            Resp::status(401, fixture("error_401.json"))
        } else if req.target.contains("folder-400") {
            Resp::status(400, fixture("error_400.json"))
        } else if c.fetch_add(1, SeqCst) < 2 {
            Resp::status(503, fixture("error_503.json"))
        } else {
            Resp::ok(fixture("delta_empty.json"))
        }
    })
    .await;
    let r = rig(&stub.base, 50, 3);
    r.mailbox.delta_round("folder-ok", "F", None).await.expect("recovers");
    assert_eq!(r.sleeper.slept(), vec![1_000, 2_000]);
    assert!(r.slowed.lock().unwrap().is_empty(), "a 503 without Retry-After is an outage, not a slow-down");

    let before = stub.requests().len();
    let CoreError::Action { failure } = r.mailbox.delta_round("folder-401", "F", None).await.unwrap_err() else { panic!("not an action error") };
    assert!(!failure.retryable && failure.reason_words.contains("sign-in expired") && failure.reason_words.contains("InvalidAuthenticationToken"), "{}", failure.reason_words);
    let CoreError::Action { failure } = r.mailbox.delta_round("folder-400", "F", None).await.unwrap_err() else { panic!("not an action error") };
    assert!(!failure.retryable && failure.reason_words.contains("ErrorInvalidIdMalformed") && failure.reason_words.contains("Id is malformed."), "{}", failure.reason_words);
    assert_eq!(stub.requests().len() - before, 2, "neither is retried");
}
