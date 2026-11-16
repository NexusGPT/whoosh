//! sync-words: the real core's sync statuses carry a code and its parameters beside the English words, a failed account is
//! tried again on the owner's next action, over the real core and sync-imap's in-process fake (clear text on 127.0.0.1, no
//! outside network). The first three read only what main's core already exposes (the SyncStatus stream, the sync row), so each
//! runs against main's sources and fails there on its assertion (docs/lanes/sync-words.md).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::{FakeImap, Fault};
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const BYE: &str = "Connection closed. 14";
const BAD: &str = "Command Argument Error. 11";
const SYNC_WAIT: Duration = Duration::from_secs(30);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < SYNC_WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {SYNC_WAIT:?}: {what}");
}

fn config(imap_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// Every SyncStateChanged of the account, in the order the stream delivered them.
#[derive(Default)]
struct Seen(Mutex<Vec<SyncState>>);
impl EventSink for Seen {
    fn on_event(&self, event: CoreEvent) {
        if let CoreEvent::SyncStateChanged { state } = event { self.0.lock().unwrap().push(state); }
    }
}

/// A stored account on the fake, with the SyncStatus stream watched from before its sync starts.
fn started(imap: &FakeImap) -> (RustCore, String, Arc<Seen>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let keychain = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let r = StoreSecret::execute(keychain.as_ref(), SecretKind::AccountCredential, USER, &Secret::new(PW.as_bytes().to_vec())).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, &format!("{}\u{1f}{}", r.service, r.account), false, config(imap.port), 1).unwrap();
    let core = RustCore::with_parts(store, keychain, detector(), TestTransport::PlainLoopback);
    let seen = Arc::new(Seen::default());
    core.subscribe(StreamKind::SyncStatus, seen.clone());
    core.start_stored_accounts();
    (core, id, seen, dir)
}

fn state(core: &RustCore, id: &str) -> SyncState {
    core.sync_states().into_iter().find(|s| s.account_id == id).unwrap_or(SyncState { account_id: id.into(), history_fetched: false, messages_indexed: 0, last_error: None, provider_status_words: None, last_synced_ms: None })
}

/// A published code: its name and parameters, read from the text as the app reads it (`<words>\u{1f}<name>?k=v&k=v`, or the bare
/// `name?k=v` of an `ActionFailed.code`), values percent-decoded.
fn code_of(text: &str) -> Option<(String, Vec<(String, String)>)> {
    let tail = text.rsplit_once('\u{1f}').map(|(_, t)| t).unwrap_or(text);
    let (name, rest) = tail.split_once('?').unwrap_or((tail, ""));
    if !name.starts_with("sync.") { return None; }
    let params = rest.split('&').filter(|p| !p.is_empty()).filter_map(|p| p.split_once('=')).map(|(k, v)| (k.to_string(), unescape(v))).collect();
    Some((name.to_string(), params))
}

fn unescape(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() { out.push(u8::from_str_radix(&v[i + 1..i + 3], 16).unwrap()); i += 3; } else { out.push(b[i]); i += 1; }
    }
    String::from_utf8(out).unwrap()
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> { params.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str()) }

fn names(seen: &Seen) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in seen.0.lock().unwrap().iter() {
        if let Some((n, _)) = s.provider_status_words.as_deref().and_then(code_of) {
            if out.last() != Some(&n) { out.push(n); }
        }
    }
    out
}

#[test]
fn the_local_sync_publishes_connecting_then_fetching_then_messages_each_with_its_code() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 7);
    let (core, id, seen, _d) = started(&imap);
    wait_until("history fetched", || state(&core, &id).history_fetched);
    wait_until("the messages status", || names(&seen).last().is_some_and(|n| n == "sync.messages"));
    assert_eq!(names(&seen), ["sync.connecting", "sync.fetching", "sync.messages"]);
    let all: Vec<String> = seen.0.lock().unwrap().iter().filter_map(|s| s.provider_status_words.clone()).collect();
    assert_eq!(all.first().unwrap().split('\u{1f}').next(), Some("Connecting…"), "the engine's English words stay whole: {all:?}");
    let (_, fetching) = all.iter().filter_map(|w| code_of(w)).find(|(n, _)| n == "sync.fetching").unwrap();
    assert_eq!(param(&fetching, "folder"), Some("INBOX"));
    assert_eq!(param(&fetching, "count"), Some("7"));
    let (_, last) = code_of(all.last().unwrap()).unwrap();
    assert_eq!(param(&last, "count"), Some("7"));
    // a listening account carries no words beside its row: the Settings list prints them raw when there are any
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    wait_until("the spent status cleared", || state(&core, &id).provider_status_words.is_none());
}

#[test]
fn a_bye_while_idling_publishes_sync_paused_with_the_seconds_and_the_servers_words() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    imap.fault(Fault::bye("IDLE", BYE));
    let (core, id, _seen, _d) = started(&imap);
    wait_until("the paused status", || state(&core, &id).provider_status_words.as_deref().and_then(code_of).is_some_and(|(n, _)| n == "sync.paused"));
    let words = state(&core, &id).provider_status_words.unwrap();
    let (_, params) = code_of(&words).unwrap();
    assert!(param(&params, "retry_s").unwrap().parse::<u64>().unwrap() >= 1, "{words:?}");
    assert!(param(&params, "server_words").unwrap().contains(BYE), "{words:?}");
    assert!(words.starts_with("New mail is paused: the server closed the connection (Connection closed. 14). Reconnecting in "), "the English words stay whole: {words}");
}

#[test]
fn a_refused_command_publishes_sync_failed_with_the_step_the_folder_and_the_servers_words() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 5);
    imap.fault(Fault::bad("UID SEARCH", BAD).times(usize::MAX));
    let (core, id, _seen, _d) = started(&imap);
    wait_until("the failure on the sync row", || state(&core, &id).last_error.is_some());
    let f = state(&core, &id).last_error.unwrap();
    assert_eq!(f.reason_words, format!("could not fetch INBOX: {BAD}"), "the words stay as they were: {f:?}");
    let (name, params) = code_of(f.code.as_deref().expect("the failure carries a code")).expect("a sync code");
    assert_eq!(name, "sync.failed");
    assert_eq!((param(&params, "step"), param(&params, "folder"), param(&params, "server_words")), (Some("fetch"), Some("INBOX"), Some(BAD)));
}

#[test]
fn the_owners_next_action_on_a_failed_account_syncs_it_again_and_clears_the_failure() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 5);
    // one refused SEARCH ends the run (a refusal is not retried by the clock); the next run finds the server well again
    imap.fault(Fault::bad("UID SEARCH", BAD));
    let (core, id, _seen, _d) = started(&imap);
    wait_until("the failure", || state(&core, &id).last_error.is_some() && matches!(core.sync_phase(&id), Some(SyncPhase::Failed(_))));
    assert!(!state(&core, &id).history_fetched);
    engine::sync::retry_now(&id);
    wait_until("history fetched after the owner's action", || state(&core, &id).history_fetched);
    let done = state(&core, &id);
    assert!(done.last_error.is_none(), "{done:?}");
    assert_eq!(done.messages_indexed, 5, "{done:?}");
    assert_eq!(core.sync_phase(&id), Some(SyncPhase::Listening));
}
