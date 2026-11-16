//! fix-realcore round 2 (main95 s1-001.log `--slice-accountmark`: "icon@notch NOT JUDGED delivered True; the notch listed 0 row(s) after 20.12s"): the
//! app's ArrivalListener (Sources/Features/PushClient/ArrivalRouter.swift) feeds the notification and the notch from `CoreEvent::ArrivalNotice` on the
//! Inbox stream, and the real core never sent it: a mail arriving while the app listens reached the list but never the notch. Here the account is
//! added over the in-process IMAP fake, the history is stored (no notice for it), then one mail arrives on INBOX while the IDLE connection listens:
//! exactly one ArrivalNotice comes, naming that mail, stored in INBOX before the notice.
//! Broken copy (one edit, red first):
//!   junk_filter.rs `store_arrivals`: `for message_id in announce {` -> `for message_id in announce.into_iter().take(0) {` (the old code: no notice, red)
#![allow(dead_code, unused_imports)]
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
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(20)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 25,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// The notices the Inbox stream carried, in order.
struct Notices(Mutex<Vec<String>>);
impl EventSink for Notices {
    fn on_event(&self, event: CoreEvent) {
        if let CoreEvent::ArrivalNotice { message_id } = event { self.0.lock().unwrap().push(message_id); }
    }
}

#[test]
fn a_mail_arriving_while_the_account_listens_is_announced_once_and_the_history_is_not() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let notices = Arc::new(Notices(Mutex::new(vec![])));
    core.subscribe(StreamKind::Inbox, notices.clone());
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port), password: SecretText { value: PW.into() } })
        .expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    std::thread::sleep(Duration::from_millis(500));
    assert!(notices.0.lock().unwrap().is_empty(), "the history is not announced: {:?}", notices.0.lock().unwrap());
    let uid = imap.add_message("INBOX");
    wait_until("the arrival is announced", || !notices.0.lock().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(500));
    let got = notices.0.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "one notice for one arrival: {got:?}");
    let m = core.store().message(&got[0]).unwrap().expect("the announced mail is stored before its notice");
    assert_eq!(m.folder, "INBOX");
    assert_eq!(core.store().uids_in_folder(&id, "INBOX").unwrap().iter().find(|(_, mid)| *mid == got[0]).map(|(u, _)| *u), Some(uid), "the notice names the mail that arrived");
    println!("PASS arrival-notice: one ArrivalNotice for UID {uid}, none for the 3 mails of the history");
}
