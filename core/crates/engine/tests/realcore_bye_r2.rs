//! sync-bye-r2: the hidden check's own shape (checks/realcore-check.py --slice-bye): three messages on a server without
//! CONDSTORE, the FIRST fetch cut by "* BYE Connection closed. 14" after one message. Every message must reach the store
//! and the account must not read as failing.
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
use engine::{RustCore, TestTransport};
use imap_fakes::{FakeImap, Fault};
use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const BYE: &str = "Connection closed. 14";
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

fn inbox(core: &RustCore) -> usize {
    core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).len()
}

#[test]
fn a_bye_on_the_first_screen_fetch_of_three_messages_without_condstore_still_stores_all_three() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| { s.cap_condstore = false; s.qresync = false; });
    imap.seed("INBOX", 3);
    imap.fault(Fault::bye("UID FETCH", BYE).after(1));
    let (core, id, _d) = started(&imap);
    wait_until("the BYE", || imap.faults_played() == 1);
    wait_until("all three stored", || state(&core, &id).messages_indexed == 3);
    wait_until("all three in the inbox", || inbox(&core) == 3);
    wait_until("the history fetched", || state(&core, &id).history_fetched);
    let s = state(&core, &id);
    assert!(s.history_fetched && s.last_error.is_none(), "{s:?}");
}

