//! fn-sync-engine with 30 fake-adapter accounts (docs/plan.json acceptance).
mod support;
use std::collections::HashMap;
use support::*;
use sync_engine::{parse_failure, words, worker_cap, Outcome, Transport};

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn every_first_screen_reaches_the_inbox_stream_before_any_backfill_page() {
    let f = fixture(false);
    let store = open_store(&f);
    run(&f, store).await;
    let snap = f.log.first_backfill.lock().unwrap().clone().expect("a backfill page was requested");
    assert!(snap.inbox_events > 0, "no InboxChanged reached the stream before backfill started");
    let first_screen = config().first_screen as i64;
    for (i, a) in f.accounts.iter().enumerate() {
        let stored = snap.messages_per_account.get(&a.id).copied().unwrap_or(0);
        let want = if i == REFUSING { 0 } else { first_screen };
        assert_eq!(stored, want, "{}: {stored} messages stored when the first backfill page was asked (want its first screen, {want})", a.id);
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn one_refused_login_keeps_the_servers_words_and_the_other_29_complete() {
    let f = fixture(false);
    let store = open_store(&f);
    let out = run(&f, store).await;
    let c = db(&f.log.db_path);
    for (i, a) in f.accounts.iter().enumerate() {
        let (fetched, err): (i64, Option<String>) =
            c.query_row("SELECT history_fetched, last_error_json FROM account_sync_state WHERE account_id = ?1", [&a.id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        let count: i64 = c.query_row("SELECT count(*) FROM message WHERE account_id = ?1", [&a.id], |r| r.get(0)).unwrap();
        if i == REFUSING {
            let failure = parse_failure(&err.expect("the refused account has a last_error")).unwrap();
            assert_eq!(failure.reason_words, REFUSAL_WORDS, "the error is not in the server's words");
            assert_eq!(failure.account_name, a.display_name);
            assert!(!failure.retryable);
            assert_eq!((fetched, count), (0, 0));
            assert!(matches!(out[&a.id], Outcome::Failed(_)));
        } else {
            assert_eq!(out[&a.id], Outcome::Complete, "{}", a.id);
            assert_eq!((fetched, count, err), (1, PER_ACCOUNT, None), "{} did not complete", a.id);
        }
    }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_throttled_account_says_slowed_by_its_provider_and_no_other_account_does() {
    let f = fixture(true);
    let store = open_store(&f);
    let out = run(&f, store).await;
    let throttled = &f.accounts[THROTTLED].id;
    let states = f.events.states.lock().unwrap().clone();
    // every status is coded; only the throttled account is ever `sync.slowed`, and it names its provider
    let slowed: Vec<_> = states.iter().filter(|s| s.provider_status_words.as_deref().and_then(|w| words::split_status(w).1).is_some_and(|c| c.name == words::SLOWED)).collect();
    assert!(!slowed.is_empty(), "no SyncStateChanged carried the slowed status");
    for s in &slowed {
        assert_eq!(&s.account_id, throttled, "{} was marked slowed", s.account_id);
        assert_eq!(s.provider_status_words.as_deref().map(|w| words::split_status(w).0), Some("slowed by Microsoft"));
        assert_eq!(s.provider_status_words.as_deref().and_then(|w| words::split_status(w).1).unwrap().param("provider"), Some("Microsoft"));
    }
    assert_eq!(out[throttled], Outcome::Complete, "the throttled account recovered and completed");
    let words_left: Option<String> =
        db(&f.log.db_path).query_row("SELECT provider_status_words FROM account_sync_state WHERE account_id = ?1", [throttled], |r| r.get(0)).unwrap();
    let left = words_left.as_deref().and_then(|w| words::split_status(w).1).map(|c| c.name);
    assert_eq!(left.as_deref(), Some(words::MESSAGES), "the slowed words are replaced once the provider answers again: {words_left:?}");
    assert_eq!(out.values().filter(|o| **o == Outcome::Complete).count(), 29);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn backfill_goes_newest_to_oldest_per_folder_within_the_caps_and_keeps_its_rows() {
    let f = fixture(false);
    let store = open_store(&f);
    run(&f, store).await;
    let mut seq: HashMap<(String, String), Vec<Option<i64>>> = HashMap::new();
    for c in f.log.calls.lock().unwrap().iter() {
        seq.entry((c.account.clone(), c.folder.clone())).or_default().push(c.before);
    }
    for ((acct, folder), befores) in &seq {
        assert_eq!(befores[0], None, "{acct}/{folder} did not start at its newest");
        let anchors: Vec<i64> = befores[1..].iter().map(|b| b.expect("only the first page has no anchor")).collect();
        assert!(anchors.windows(2).all(|w| w[1] < w[0]), "{acct}/{folder} not newest to oldest: {befores:?}");
    }
    let max = f.log.max_inflight.lock().unwrap().clone();
    for a in &f.accounts {
        let m = max.get(&a.id).copied().unwrap_or(0);
        assert!(m <= worker_cap(a.provider, Transport::Imap), "{}: {m} calls at once, cap {}", a.id, worker_cap(a.provider, Transport::Imap));
    }
    let accounts_at_once = f.log.max_backfilling_accounts.load(std::sync::atomic::Ordering::SeqCst);
    assert!(accounts_at_once <= config().backfill_accounts, "{accounts_at_once} accounts backfilled at once");
    let c = db(&f.log.db_path);
    let rows: i64 = c
        .query_row(
            "SELECT count(*) FROM folder_sync_state WHERE last_full_pass_ms IS NOT NULL AND cursor_kind = 'imap' AND last_uid = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 29 * FOLDERS.len() as i64, "every folder of the 29 accounts keeps its cursor and full pass");
    let indexed: i64 = c.query_row("SELECT sum(messages_indexed) FROM account_sync_state", [], |r| r.get(0)).unwrap();
    assert_eq!(indexed, 29 * PER_ACCOUNT);
}
