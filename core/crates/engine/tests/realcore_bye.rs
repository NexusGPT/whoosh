//! engine-sync-m365: the owner's first Microsoft 365 account listed its folders and stored 0 messages, its history marked
//! fetched, push stopped on "Connection closed. 14". These run the real core against sync-imap's in-process fake playing
//! the server's BYE or BAD on a chosen command (clear text on 127.0.0.1, no outside network). Each was seen red on a named
//! mutant (docs/lanes/engine-sync-m365.md).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::Arc;
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
/// Long enough for the first reconnect backoff (SYNC_BACKOFF_START_MS) and a few, against a fake that answers in milliseconds.
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

/// A stored account on the fake (no add-account check runs, so every command the fake sees is the sync's), synced at once.
fn started(imap: &FakeImap) -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let keychain = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let r = StoreSecret::execute(keychain.as_ref(), SecretKind::AccountCredential, USER, &Secret::new(PW.as_bytes().to_vec())).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, &format!("{}\u{1f}{}", r.service, r.account), false, config(imap.port), 1).unwrap();
    let core = RustCore::with_parts(store, keychain, detector(), TestTransport::PlainLoopback);
    core.start_stored_accounts();
    (core, id, dir)
}

fn state(core: &RustCore, id: &str) -> SyncState {
    core.sync_states().into_iter().find(|s| s.account_id == id).unwrap_or(SyncState { account_id: id.into(), history_fetched: false, messages_indexed: 0, last_error: None, provider_status_words: None, last_synced_ms: None })
}

#[test]
fn a_bye_in_the_middle_of_the_history_fetch_is_retried_and_the_folder_finishes_from_its_last_uid() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 80);
    // the first FETCH is the first screen (the newest 50); the second, the history page below it, is cut after 10 messages
    imap.fault(Fault::bye("UID FETCH", BYE).skip(1).after(10));
    let (core, id, _d) = started(&imap);
    wait_until("the BYE", || imap.faults_played() == 1);
    // inside the reconnect backoff: the first screen is stored, the cut page is not, and nothing says the history is in
    let mid = state(&core, &id);
    assert!(!mid.history_fetched, "history marked fetched while a page was lost: {mid:?}");
    assert_eq!(mid.messages_indexed, 50, "{mid:?}");
    wait_until("history fetched after the reconnect", || state(&core, &id).history_fetched);
    let done = state(&core, &id);
    assert_eq!(done.messages_indexed, 80, "every message once: {done:?}");
    assert!(done.last_error.is_none(), "{done:?}");
    let log = imap.log();
    let after_bye: Vec<&String> = log.iter().skip_while(|l| !l.contains("UID FETCH")).skip(1).skip_while(|l| !l.contains("UID FETCH")).skip(1).collect();
    assert!(after_bye.iter().any(|l| l.contains("UID SEARCH UID 1:30")), "the retry resumes below the last stored UID (31): {after_bye:?}");
}

#[test]
fn a_refused_fetch_ends_the_sync_with_the_step_and_the_servers_words_on_the_sync_row() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 5);
    imap.fault(Fault::bad("UID SEARCH", "Command Argument Error. 11").times(usize::MAX));
    let (core, id, _d) = started(&imap);
    wait_until("the failure on the sync row", || state(&core, &id).last_error.is_some());
    let s = state(&core, &id);
    let f = s.last_error.clone().unwrap();
    assert_eq!(f.reason_words, "could not fetch INBOX: Command Argument Error. 11", "{f:?}");
    assert!(!s.history_fetched, "{s:?}");
    assert_eq!(s.messages_indexed, 0);
    assert!(matches!(core.sync_phase(&id), Some(SyncPhase::Failed(_))), "{:?}", core.sync_phase(&id));
    assert_eq!(core.accounts()[0].health.as_ref().map(|h| h.reason_words.clone()), Some(f.reason_words));
}

#[test]
fn a_bye_while_idling_puts_the_servers_words_on_the_status_then_reconnects_and_new_mail_arrives() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    imap.fault(Fault::bye("IDLE", BYE));
    let (core, id, _d) = started(&imap);
    wait_until("the paused words", || state(&core, &id).provider_status_words.is_some_and(|w| w.contains(BYE)));
    let words = state(&core, &id).provider_status_words.unwrap();
    assert!(words.starts_with("New mail is paused: the server closed the connection (Connection closed. 14). Reconnecting in "), "{words}");
    assert!(state(&core, &id).history_fetched && state(&core, &id).last_error.is_none(), "the mail stays fetched: {:?}", state(&core, &id));
    wait_until("the IDLE on the new connection", || imap.with(|s| s.idle_starts >= 1));
    wait_until("the words cleared", || state(&core, &id).provider_status_words.is_none());
    imap.add_message("INBOX");
    wait_until("the fourth message", || state(&core, &id).messages_indexed == 4 || core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).len() == 4);
}

#[test]
fn a_bye_on_the_select_of_a_reconnect_keeps_push_alive() {
    // the owner's words: "New mail is not pushed: io: status: Bye ... Connection closed. 14", push gone for good
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 2);
    let (core, id, _d) = started(&imap);
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening) && imap.with(|s| s.idle_starts == 1));
    imap.fault(Fault::bye("SELECT", BYE));
    imap.drop_idle();
    wait_until("the BYE on the reconnect's SELECT", || imap.faults_played() == 1);
    wait_until("IDLE again", || imap.with(|s| s.idle_starts >= 2));
    let words = state(&core, &id).provider_status_words;
    assert!(!words.as_deref().unwrap_or("").contains("not pushed"), "push ended: {words:?}");
}
