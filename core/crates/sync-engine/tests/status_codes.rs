//! sync-words: every status the engine publishes carries a code and its parameters beside the English words; a failed account
//! is tried again; a Microsoft account reached over IMAP takes the IMAP worker cap. The first four read only what main's
//! engine already exposes (the published `provider_status_words`, the account's `last_error`, the in-flight log), so each runs
//! against main's sources and fails there on its assertion (docs/lanes/sync-words.md).
mod support;
use async_trait::async_trait;
use contracts::{Account, ActionFailed, CoreError, MailProvider};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use support::*;
use sync_engine::{AccountSource, Engine, EngineConfig, Outcome, Page, Resume, SourceError, Transport};
use tokio::sync::Notify;
use tokio::time::Instant;

/// A status's code as published (`<English words>\u{1f}<name>?k=v&k=v`): its name and parameters, never by the crate's own parser.
fn code_of(status: &str) -> Option<(String, Vec<(String, String)>)> {
    let tail = status.split_once('\u{1f}')?.1;
    let (name, rest) = tail.split_once('?').unwrap_or((tail, ""));
    let params = rest.split('&').filter(|p| !p.is_empty()).filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), unescape(v))).collect();
    Some((name.to_string(), params))
}

fn unescape(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            out.push(u8::from_str_radix(&v[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn failure(words: &str, retryable: bool) -> SourceError {
    SourceError::from(CoreError::Action { failure: ActionFailed { account_name: "Scripted".into(), reason_words: words.into(), retryable, code: None } })
}

/// One INBOX of `total` messages; each `page` call first takes the next scripted answer (None: serve the page).
struct Scripted {
    script: Mutex<Vec<Option<SourceError>>>,
    transport: Transport,
    calls: AtomicUsize,
    times: Mutex<Vec<Instant>>,
    total: i64,
}

impl Scripted {
    fn new(script: Vec<Option<SourceError>>) -> Arc<Scripted> {
        Arc::new(Scripted { script: Mutex::new(script), transport: Transport::Imap, calls: AtomicUsize::new(0), times: Mutex::new(vec![]), total: 5 })
    }
}

#[async_trait]
impl AccountSource for Scripted {
    fn transport(&self) -> Transport { self.transport }
    async fn list_folders(&self) -> Result<Vec<String>, SourceError> { Ok(vec!["INBOX".into()]) }
    async fn page(&self, folder: &str, before: Option<i64>, limit: u32) -> Result<Page, SourceError> {
        self.calls.fetch_add(1, SeqCst);
        self.times.lock().unwrap().push(Instant::now());
        let next = { let mut s = self.script.lock().unwrap(); if s.is_empty() { None } else { s.remove(0) } };
        if let Some(e) = next { return Err(e); }
        let acct = account(0);
        let top = before.map(|b| b - 1).unwrap_or(self.total);
        let low = (top - limit as i64 + 1).max(1);
        let messages = (low..=top).rev().map(|k| message_for(&acct, folder, k)).collect::<Vec<_>>();
        let oldest = if messages.is_empty() { None } else { Some(low) };
        Ok(Page { messages, oldest, done: oldest.is_none_or(|o| o <= 1), cursor: None })
    }
}

fn message_for(acct: &Account, folder: &str, k: i64) -> store::NewMessage { message(acct, folder, 0, k) }

/// Names of the codes one account published, in order, consecutive repeats folded.
fn code_names(f: &Fixture, account_id: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in f.events.states.lock().unwrap().iter().filter(|s| s.account_id == account_id) {
        if let Some((name, _)) = s.provider_status_words.as_deref().and_then(code_of) {
            if out.last() != Some(&name) { out.push(name); }
        }
    }
    out
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn every_status_of_a_sync_carries_its_code_in_order_connecting_fetching_messages() {
    let mut f = fixture(false);
    f.accounts.truncate(1);
    let store = open_store(&f);
    let out = run(&f, store).await;
    let id = &f.accounts[0].id;
    assert_eq!(out[id], Outcome::Complete);
    assert_eq!(code_names(&f, id), ["sync.connecting", "sync.fetching", "sync.messages"]);
    let states = f.events.states.lock().unwrap().clone();
    let published: Vec<&str> = states.iter().filter_map(|s| s.provider_status_words.as_deref()).collect();
    let first = published.first().unwrap();
    assert_eq!(first.split('\u{1f}').next(), Some("Connecting…"), "the engine's own English words stay whole: {first:?}");
    let fetching: Vec<_> = published.iter().filter_map(|w| code_of(w)).filter(|(n, _)| n == "sync.fetching").collect();
    assert!(!fetching.is_empty());
    for (_, params) in &fetching {
        let folder = param(params, "folder").expect("sync.fetching names its folder");
        assert!(FOLDERS.iter().any(|(f, _)| *f == folder), "{folder}");
        assert!(param(params, "count").unwrap().parse::<i64>().unwrap() >= 1);
    }
    let last = published.last().unwrap();
    let (name, params) = code_of(last).unwrap();
    assert_eq!((name.as_str(), param(&params, "count")), ("sync.messages", Some(&*PER_ACCOUNT.to_string())));
    assert_eq!(last.split('\u{1f}').next(), Some("280 messages"));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_lost_connection_that_is_retried_says_sync_paused_with_the_seconds_and_the_servers_words() {
    const BYE: &str = "the server closed the connection (Connection closed. 14)";
    let mut f = fixture(false);
    f.accounts.truncate(1);
    let store = open_store(&f);
    let src = Scripted::new(vec![Some(failure(BYE, true)), Some(failure(BYE, true))]);
    let engine = Engine::new(store, f.events.clone(), Arc::new(TestClock), config());
    let out = engine.run(vec![(f.accounts[0].clone(), src.clone() as Arc<dyn AccountSource>)]).await;
    assert_eq!(out[0].1, Outcome::Complete, "two lost connections are inside the retry count (3)");
    let id = &f.accounts[0].id;
    let paused: Vec<_> = f.events.states.lock().unwrap().iter().filter(|s| &s.account_id == id).filter_map(|s| s.provider_status_words.as_deref().and_then(code_of)).filter(|(n, _)| n == "sync.paused").collect();
    assert!(!paused.is_empty(), "no sync.paused was published while the connection was down: {:?}", code_names(&f, id));
    for (_, params) in &paused {
        assert_eq!(param(params, "retry_s"), Some("1"), "the 2 ms backoff rounds up to 1 s");
        assert_eq!(param(params, "server_words"), Some(BYE));
    }
    assert_eq!(code_names(&f, id).last().map(String::as_str), Some("sync.messages"), "{:?}", code_names(&f, id));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_failure_that_stops_the_sync_carries_sync_failed_with_the_words_it_always_had() {
    const BAD: &str = "could not fetch INBOX: Command Argument Error. 11";
    let mut f = fixture(false);
    f.accounts.truncate(1);
    let store = open_store(&f);
    let src = Scripted::new(vec![Some(failure(BAD, false))]);
    let engine = Engine::new(store, f.events.clone(), Arc::new(TestClock), config());
    let out = engine.run(vec![(f.accounts[0].clone(), src as Arc<dyn AccountSource>)]).await;
    let Outcome::Failed(failed) = &out[0].1 else { panic!("{:?}", out[0].1) };
    assert_eq!(failed.reason_words, BAD, "the words stay as they were");
    let code = failed.code.as_deref().expect("a failure carries a code");
    let (name, params) = code_of(&format!("x\u{1f}{code}")).unwrap();
    assert_eq!(name, "sync.failed");
    assert_eq!(param(&params, "step"), Some("sync"), "a source that names no step is a failed sync");
    assert_eq!(param(&params, "server_words"), Some(BAD));
    // the same code is on the stored row the app reads
    let stored: Option<String> = db(&f.log.db_path).query_row("SELECT last_error_json FROM account_sync_state WHERE account_id = ?1", [&f.accounts[0].id], |r| r.get(0)).unwrap();
    assert!(stored.unwrap().contains("sync.failed"));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_microsoft_account_reached_over_imap_has_at_most_the_imap_worker_cap_in_flight() {
    let cap = sync_engine::consts::IMAP_CONN_PER_ACCOUNT;
    let mut f = fixture(false);
    f.accounts = vec![account(THROTTLED)]; // the fixture's Microsoft account; its FakeSource is an IMAP source
    assert_eq!(f.accounts[0].provider, MailProvider::Microsoft);
    let store = open_store(&f);
    run(&f, store).await;
    let max = f.log.max_inflight.lock().unwrap().get(&f.accounts[0].id).copied().unwrap();
    assert_eq!(max, cap, "three folders backfill at once; a Microsoft IMAP account may have {cap} calls in flight, not Graph's 4");
}

/// A Graph source: the same fake, reached through Graph.
struct GraphFake(FakeSource);

#[async_trait]
impl AccountSource for GraphFake {
    fn transport(&self) -> Transport { Transport::Graph }
    async fn list_folders(&self) -> Result<Vec<String>, SourceError> { self.0.list_folders().await }
    async fn page(&self, folder: &str, before: Option<i64>, limit: u32) -> Result<Page, SourceError> { self.0.page(folder, before, limit).await }
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_graph_account_keeps_the_graph_cap() {
    use std::sync::atomic::AtomicUsize;
    assert_eq!(sync_engine::worker_cap(MailProvider::Microsoft, Transport::Graph), sync_engine::consts::GRAPH_CONCURRENT);
    assert_eq!(sync_engine::worker_cap(MailProvider::Microsoft, Transport::Imap), sync_engine::consts::IMAP_CONN_PER_ACCOUNT);
    let mut f = fixture(false);
    f.accounts = vec![account(THROTTLED)];
    let store = open_store(&f);
    let a = f.accounts[0].clone();
    let src = GraphFake(FakeSource { account: a.clone(), index: 0, refuse: None, throttle_left: AtomicUsize::new(0), log: f.log.clone() });
    let engine = Engine::new(store, f.events.clone(), Arc::new(TestClock), config());
    let out = engine.run(vec![(a.clone(), Arc::new(src) as Arc<dyn AccountSource>)]).await;
    assert_eq!(out[0].1, Outcome::Complete);
    let max = f.log.max_inflight.lock().unwrap().get(&a.id).copied().unwrap();
    assert_eq!(max, FOLDERS.len(), "a Graph account runs its {} folders at once: its cap (4) is above that", FOLDERS.len());
}

/// A ledger number, read from docs/CONSTANTS.md (the one place it lives), as the engine's config takes it.
fn ledger(name: &str) -> u64 {
    let table = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/CONSTANTS.md")).unwrap();
    let row = table.lines().find(|l| l.trim_start().starts_with(&format!("| {name} "))).expect("the ledger row");
    row.split('|').nth(2).unwrap().trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap()
}

fn ledger_backoff_max_ms() -> u64 { ledger("SYNC_BACKOFF_MAX_MS") }

/// The ledger's retry count and longest wait; the first wait is a test input (2 ms on the paused clock).
fn resuming_config(max_ms: u64) -> EngineConfig { EngineConfig::from_ledger(40, 2, max_ms, ledger("SYNC_RETRY_COUNT") as u32) }

async fn settle() { for _ in 0..200 { tokio::task::yield_now().await; tokio::time::sleep(Duration::from_millis(1)).await; } }

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_sixth_failure_is_tried_again_after_the_ledgers_longest_wait_and_says_so() {
    let max = ledger_backoff_max_ms();
    assert_eq!(max, 300_000, "the ledger's SYNC_BACKOFF_MAX_MS");
    let mut f = fixture(false);
    f.accounts.truncate(1);
    let store = open_store(&f);
    let down = || Some(failure("the server closed the connection (Connection closed. 14)", true));
    // the first try and SYNC_RETRY_COUNT (5) retries fail: the sixth failure ends the run, and is the one the owner reads
    let src = Scripted::new((0..6).map(|_| down()).collect());
    let engine = Arc::new(Engine::new(store, f.events.clone(), Arc::new(TestClock), resuming_config(max)));
    let retry = Arc::new(Notify::new());
    let phases = Arc::new(Mutex::new(Vec::new()));
    let task = {
        let (engine, retry, phases, acct, src) = (engine.clone(), retry.clone(), phases.clone(), f.accounts[0].clone(), src.clone());
        tokio::spawn(async move { engine.run_resuming(acct, src as Arc<dyn AccountSource>, &retry, |r| phases.lock().unwrap().push(r)).await })
    };
    settle().await;
    let calls_at_failure = src.calls.load(SeqCst);
    assert_eq!(calls_at_failure, 6, "the sixth failure");
    assert!(matches!(phases.lock().unwrap().last(), Some(Resume::Failed(_))));
    let row: Option<String> = db(&f.log.db_path).query_row("SELECT provider_status_words FROM account_sync_state WHERE account_id = ?1", [&f.accounts[0].id], |r| r.get(0)).unwrap();
    let (name, params) = code_of(&row.expect("the failed account says what it waits for")).unwrap();
    assert_eq!((name.as_str(), param(&params, "retry_s")), ("sync.paused", Some("300")));
    // just before the ledger's longest wait: nothing runs (the clock moves by itself while every task waits); after it: the account
    // runs again and completes
    let mut task = task;
    let early = tokio::time::timeout(Duration::from_millis(max - 1_000), &mut task).await;
    assert!(early.is_err(), "the account ended before the longest wait");
    assert_eq!(src.calls.load(SeqCst), calls_at_failure, "tried again before the longest wait");
    let out = tokio::time::timeout(Duration::from_secs(60), task).await.expect("the account was not tried again after the longest wait").unwrap();
    assert_eq!(out, Outcome::Complete);
    assert!(src.calls.load(SeqCst) > calls_at_failure);
    let times = src.times.lock().unwrap().clone();
    let gap = times[calls_at_failure].duration_since(times[calls_at_failure - 1]);
    assert!(gap >= Duration::from_millis(max) && gap < Duration::from_millis(max + 50), "the gap before the retry was {gap:?}");
    assert!(matches!(phases.lock().unwrap().last(), Some(Resume::Again)));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_owners_next_action_tries_a_failed_account_again_at_once_and_a_refusal_waits_for_it() {
    let max = ledger_backoff_max_ms();
    let mut f = fixture(false);
    f.accounts.truncate(1);
    let store = open_store(&f);
    // a refused command is not retryable: the clock never brings it back; the owner's action does
    let src = Scripted::new(vec![Some(failure("could not fetch INBOX: Command Argument Error. 11", false))]);
    let engine = Arc::new(Engine::new(store, f.events.clone(), Arc::new(TestClock), resuming_config(max)));
    let retry = Arc::new(Notify::new());
    let task = {
        let (engine, retry, acct, src) = (engine.clone(), retry.clone(), f.accounts[0].clone(), src.clone());
        tokio::spawn(async move { engine.run_resuming(acct, src as Arc<dyn AccountSource>, &retry, |_| {}).await })
    };
    settle().await;
    assert_eq!(src.calls.load(SeqCst), 1);
    tokio::time::advance(Duration::from_millis(max * 3)).await;
    settle().await;
    assert_eq!(src.calls.load(SeqCst), 1, "a refusal was retried by the clock");
    let before = Instant::now();
    retry.notify_waiters();
    let out = tokio::time::timeout(Duration::from_secs(60), task).await.expect("the owner's action did not retry the account").unwrap();
    assert_eq!(out, Outcome::Complete);
    assert!(Instant::now().duration_since(before) < Duration::from_secs(1), "the retry waited instead of running at once");
    assert_eq!(src.calls.load(SeqCst), 2);
}
