mod support;
use serde_json::json;
use support::*;
use sync_gmail_api::limits::*;
use sync_gmail_api::{GmailError, QuotaGate};

/// A full sync of `n` messages in two list pages, bodies from a template with the shape of the message fixtures.
fn big_mailbox(replay: &Replay, n: usize) {
    replay.push(json!({"path": "profile", "body": {"emailAddress": "owner@example.test", "historyId": "9000"}}));
    let ids: Vec<String> = (0..n).map(|i| format!("q-{i:04}")).collect();
    let half = n / 2;
    let page = |slice: &[String]| slice.iter().map(|id| json!({"id": id, "threadId": format!("t-{id}")})).collect::<Vec<_>>();
    replay.push(json!({"path": "messages", "body": {"messages": page(&ids[..half]), "nextPageToken": "second"}}));
    replay.push(json!({"path": "messages", "query": [["pageToken", "second"]], "body": {"messages": page(&ids[half..])}}));
    for id in &ids {
        // base64url of "From: bulk@example.test\r\nSubject: bulk\r\n\r\nbody\r\n"
        let raw = "RnJvbTogYnVsa0BleGFtcGxlLnRlc3QNClN1YmplY3Q6IGJ1bGsNCg0KYm9keQ0K";
        replay.push(json!({"path": format!("messages/{id}"), "query": [["format", "raw"]],
            "body": {"id": id, "threadId": format!("t-{id}"), "labelIds": ["INBOX"], "historyId": "8000", "internalDate": "1793000000000", "sizeEstimate": 40, "raw": raw}}));
    }
}

#[test]
fn quota_per_minute_never_exceeds_the_ledger_constant_in_a_big_full_sync() {
    let cap = ledger_number("GMAIL_UNITS_PER_MIN");
    assert_eq!(GMAIL_UNITS_PER_MIN, cap, "the crate must use the ledger number");

    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    big_mailbox(&replay, 260);
    let api = open(&replay, &clock);
    let mut sink = VecSink::default();
    let summary = api.sync(None, &mut sink).expect("sync");
    assert_eq!(summary.messages, 260);

    let log = replay.requests();
    let total: u64 = log.iter().map(|l| cost_of(&l.request)).sum();
    assert!(total > 2 * cap, "the fixture run must be big enough to need pacing: {total} units against {cap}");
    assert!(clock.slept() > 0, "the gate never waited, so the run proved nothing");
    let peak = peak_units_per_minute(&log);
    assert!(peak <= cap, "peak {peak} units in one minute is above the ledger cap {cap}");
    assert!(peak > cap / 2, "the gate is far below the cap ({peak} of {cap}): it is waiting for nothing");
}

#[test]
fn a_429_waits_for_retry_after_and_every_try_still_counts() {
    let cap = ledger_number("GMAIL_UNITS_PER_MIN");
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    replay.push(json!({"path": "labels", "status": 429, "retry_after_ms": 2000, "body": {"error": {"code": 429, "message": "Too many concurrent requests for user.", "errors": [{"reason": "rateLimitExceeded"}]}}}));
    replay.push(json!({"path": "labels", "body": {"labels": []}}));
    let api = open(&replay, &clock);
    api.labels().expect("second try succeeds");
    let log = replay.requests();
    assert_eq!(log.len(), 2);
    assert!(log[1].at_ms - log[0].at_ms >= 2000, "did not wait for Retry-After");
    assert!(peak_units_per_minute(&log) <= cap);
}

#[test]
fn a_403_with_a_rate_limit_reason_is_retried_and_gives_up_with_googles_words() {
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    replay.push(json!({"path": "labels", "status": 403, "repeat": true, "body": {"error": {"code": 403, "message": "User-rate limit exceeded.", "errors": [{"reason": "userRateLimitExceeded"}]}}}));
    let api = open(&replay, &clock);
    let err = api.labels().unwrap_err();
    assert_eq!(err, GmailError::RateLimited { message: "User-rate limit exceeded.".into(), attempts: GMAIL_RETRY_MAX as u32 });
    assert_eq!(replay.count() as u64, GMAIL_RETRY_MAX);
    assert!(err.retryable());
}

#[test]
fn a_403_without_a_rate_limit_reason_is_not_retried() {
    let clock = Clock::new(T0);
    let replay = Replay::new(&clock);
    replay.push(json!({"path": "labels", "status": 403, "repeat": true, "body": {"error": {"code": 403, "message": "Insufficient Permission", "errors": [{"reason": "insufficientPermissions"}]}}}));
    let err = open(&replay, &clock).labels().unwrap_err();
    assert_eq!(err, GmailError::Http { status: 403, message: "Insufficient Permission".into() });
    assert_eq!(replay.count(), 1);
}

#[test]
fn the_gate_alone_holds_the_cap_over_a_long_run() {
    let cap = ledger_number("GMAIL_UNITS_PER_MIN");
    let mut gate = QuotaGate::new(cap, 60_000);
    let mut now = 0i64;
    let mut log: Vec<(i64, u64)> = vec![];
    for _ in 0..1000 {
        loop {
            let wait = gate.wait_needed(now, GMAIL_COST_MESSAGES_GET_RAW);
            if wait == 0 {
                break;
            }
            now += wait;
        }
        gate.record(now, GMAIL_COST_MESSAGES_GET_RAW);
        log.push((now, GMAIL_COST_MESSAGES_GET_RAW));
        now += 10;
    }
    let peak = log.iter().map(|&(t, _)| log.iter().filter(|&&(u, _)| u <= t && u > t - 60_000).map(|&(_, c)| c).sum::<u64>()).max().unwrap();
    assert!(peak <= cap, "peak {peak} above {cap}");
    // Every constant in the cost table is within the cap, so nothing is clamped.
    assert!(GMAIL_COST_MESSAGES_GET_RAW < cap);
}
