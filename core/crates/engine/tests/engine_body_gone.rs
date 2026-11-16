//! fix-realcore round 9 (item 7e1fc235, the owner's core.log: 240 lines "the server sent no body for UID N" for 8 UIDs between 10 and 12 Nov, each
//! "no FETCH for this UID" in all three asks): two cases, on the loopback IMAP fake and on a fake body source (no socket).
//!   1. A UID the server no longer holds: after the three asks get no FETCH line, ONE `UID SEARCH UID <n>` finds it gone; the row is removed from the
//!      list after that one pass and is never asked again, and core.log holds one line for it.
//!   2. A UID the server lists but whose body never comes: asked once in ten passes (the folder's sync no longer clears the failure, the pause runs),
//!      the reader's read `message/<id>/body_unavailable` says why, and core.log holds one line, not one per pass.
//! Broken copies (one edit each, red first):
//!   sync-imap/src/checked.rs `fetch_bodies`: `if silent && !uid_search(s, &format!("UID {uid}")).await?.contains(&uid) {` -> `if false {` (1: the row stays: red)
//!   engine/src/bodies.rs `keep`: `if source.as_ref().is_some_and(|s| s.gone(folder, *uid)) {` -> `if false {` (1: the row stays: red)
//!   engine/src/bodies.rs `folder_synced`: `lock(&self.failed).retain(|_, f| f.no_body || !(f.account == account && f.folder == folder));` ->
//!       `lock(&self.failed).retain(|_, f| !(f.account == account && f.folder == folder));` (2: asked on every pass: red)
//!   engine/src/bodies.rs `no_body`: `} else {\n            return;\n        };` -> `} else {\n            format!("body {id}: {why}")\n        };` (2: one log line per pass: red)
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../send/tests/support/mod.rs"]
mod smtp_fakes;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::bodies::BodySource;
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use smtp_fakes::{FakeSmtp, Mode};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(40);
const PASSES: usize = 10;

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn wait_for<T: std::fmt::Debug>(what: &str, mut read: impl FnMut() -> T, ok: impl Fn(&T) -> bool) -> T {
    let t0 = Instant::now();
    loop {
        let v = read();
        if ok(&v) { return v; }
        if t0.elapsed() >= WAIT { panic!("never reached within {WAIT:?}: {what}; last read: {v:?}"); }
        std::thread::sleep(Duration::from_millis(40));
    }
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
/// A stored row with no body yet, as the sync stores headers.
fn row(account: &str, id: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{account}:<gone{uid}@thread>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "s@example.test".into(), to: vec![USER.into()], subject: format!("Gone {uid}"), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}
fn lines_of(log: &[String], needle: &str) -> usize { log.iter().filter(|l| l.contains(needle)).count() }
fn core_log(dir: &std::path::Path, needle: &str) -> usize {
    std::fs::read_to_string(dir.join("core.log")).unwrap_or_default().lines().filter(|l| l.contains(needle)).count()
}

#[test]
fn a_uid_the_server_no_longer_holds_is_removed_after_one_pass_and_never_asked_again() {
    const GONE: u32 = 99;
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    // round 9 fix (lead's run 13 Nov 00:12): the add validates the sending server too, so a loopback SMTP fake listens (as engine_files_r6.rs)
    let smtp = FakeSmtp::start(Mode::Xoauth2);
    let db = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&db.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } })
        .expect("added").created_id.unwrap();
    wait_for("listening", || core.sync_phase(&account), |p| *p == Some(SyncPhase::Listening));
    // a row the store holds for a UID the server no longer lists (moved or deleted from another device while this Mac was away)
    let id = format!("{account}:INBOX:gone:{GONE}");
    core.store().insert_messages(vec![row(&account, &id, GONE)]).unwrap();
    let _ = core.body(id.clone());
    wait_for("the row left the list", || core.store().message(&id).unwrap().map(|m| m.subject), |m| m.is_none());
    let search = format!("UID SEARCH UID {GONE}");
    let asked = |log: &[String]| (lines_of(log, &search), log.iter().filter(|l| l.contains("UID FETCH") && l.split(' ').any(|w| w == GONE.to_string() || w.starts_with(&format!("{GONE},")) || w.ends_with(&format!(",{GONE}")))).count());
    let first = asked(&imap.log());
    assert_eq!(first.0, 1, "one existence check: {first:?}");
    // more passes: the sync's folder pass, the prefetch, the crawl and the reader again
    for _ in 0..3 {
        core.background().folder_synced(&account, "INBOX");
        core.background().kick(&account);
        core.background().crawl_kick(&account);
        let _ = core.body(id.clone());
        assert!(core.background().settle(WAIT), "the background ended");
    }
    let after = asked(&imap.log());
    assert_eq!(after, first, "never asked again after it was found gone (search lines, fetch lines)");
    assert_eq!(core_log(db.path(), &format!("no longer lists UID {GONE}")), 1, "one log line for the decision");
    println!("PASS body-gone-removed: one UID SEARCH, row removed, {} fetch lines in all, none after", first.1);
}

/// A server that lists the message but never sends its body (the same case words as the owner's log), counting the asks per UID.
struct NoBody { calls: AtomicUsize, asked: Mutex<Vec<Vec<u32>>> }
#[async_trait]
impl BodySource for NoBody {
    async fn fetch_bodies(&self, _folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push(uids.to_vec());
        Ok(Vec::new())
    }
    fn why_missing(&self, _folder: &str, _uid: u32) -> Option<String> { Some("no BODY[] in the answer (keys UID FLAGS)".into()) }
}

#[test]
fn a_uid_the_server_lists_without_a_body_is_asked_once_in_ten_passes_and_logged_once() {
    const STUCK: u32 = 5;
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let id = format!("{account}:INBOX:1:{STUCK}");
    store.insert_messages(vec![row(&account, &id, STUCK)]).unwrap();
    let source = Arc::new(NoBody { calls: AtomicUsize::new(0), asked: Mutex::new(vec![]) });
    core.set_body_source(&account, source.clone());
    for _ in 0..PASSES {
        core.background().folder_synced(&account, "INBOX");
        core.background().kick(&account);
        core.background().crawl_kick(&account);
        let _ = core.body(id.clone());
        assert!(core.background().settle(WAIT), "the background ended");
    }
    let asked: usize = source.asked.lock().unwrap().iter().filter(|u| u.contains(&STUCK)).count();
    assert_eq!(asked, 1, "asked once in {PASSES} passes (the pause runs, the folder's sync does not clear it): {:?}", source.asked.lock().unwrap());
    let why = core.state(format!("message/{id}/body_unavailable")).expect("served");
    let v: serde_json::Value = serde_json::from_str(&why).unwrap();
    assert_eq!((v["code"].as_str(), v["tries"].as_u64(), v["stopped"].as_bool()), (Some("body_not_sent"), Some(1), Some(false)), "{why}");
    assert!(core.store().message(&id).unwrap().is_some(), "a mail the server still lists keeps its row");
    assert_eq!(core_log(dir.path(), &format!("body {id}:")), 1, "one log line for the decision, not one per pass");
    println!("PASS body-not-sent-bounded: asked 1 time in {PASSES} passes, one log line, the reader's reason {why}");
}
