//! perf-archive round 2 (the undo twin of engine_unarchive_after_resync.rs): after E the server moved the copy to Archive (COPYUID recorded
//! its place), the sync heard the INBOX UID vanish and dropped it from the archived row (sync.rs `keep_archived`, `clear_remote_ids`), and Z
//! then built `SetArchived { archived: false }` from a row with no UID (actions usecases/undo.rs -> archive::apply): writeback.rs `rows_for`
//! queued no row, so the mail came back on this Mac and stayed in Archive on the server.
//! Here the row is in the state the sync leaves it in (the same store write `keep_archived` makes), then Z must move the Archive copy back
//! to the server's INBOX.
//! Broken copy (one edit, red first):
//!   writeback.rs `Ledger::execute`: `let placed = with_recorded_places(ctx, request)?;` -> `let placed: Option<WriteBack> = None;`
//!   (today's code: no row queued, the wait for the server's INBOX times out)
#![allow(dead_code, unused_imports)]
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
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(25);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn subjects_of(rows: &[InboxRow]) -> Vec<String> { rows.iter().map(|r| r.subject.clone()).collect() }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: String::new(), smtp_port: 25,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn box_subjects(imap: &FakeImap, name: &str) -> Vec<String> {
    imap.with(|s| s.boxes.iter().find(|b| b.name == name).unwrap().msgs.iter().map(|m| String::from_utf8_lossy(&m.raw).lines().find(|l| l.starts_with("Subject:")).unwrap_or("").trim_start_matches("Subject: ").to_string()).collect())
}
fn settled(core: &RustCore) -> bool { core.store().pending_actions().unwrap().is_empty() }

#[test]
fn z_after_archive_moves_the_archive_copy_back_when_the_sync_already_dropped_the_inbox_uid() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port), password: SecretText { value: PW.into() } }).expect("added");
    let account = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(SyncPhase::Listening));
    let m3 = core.inbox(tab("primary")).into_iter().find(|r| r.subject == "Message 3").expect("Message 3 in the inbox");
    core.run(Command::Archive { message_ids: vec![m3.message_id.clone()] }).expect("archive");
    wait_until("the server holds it in Archive", || box_subjects(&imap, "Archive") == vec!["Message 3".to_string()] && settled(&core));
    let place = core.store().message_place(&m3.message_id).unwrap().expect("the move recorded where the server put the copy (COPYUID)");
    assert_eq!(place.folder, "Archive");
    // the sync heard INBOX's UID 3 vanish: the archived row with a recorded place loses its stale INBOX UID (sync.rs `keep_archived`, this same write)
    let id = m3.message_id.clone();
    core.store().write(move |ctx| ctx.clear_remote_ids(&[id])).unwrap();
    assert!(core.store().message(&m3.message_id).unwrap().is_some(), "the archived row stays");
    // Z: the newest undo entry is the archive
    core.run(Command::Undo).expect("undo");
    assert!(subjects_of(&core.inbox(tab("primary"))).contains(&"Message 3".to_string()), "back in the inbox the moment Z returns");
    // (the worker may apply the row before a read here, so the server's boxes are the judge, never the pending rows)
    wait_until("the server holds it in INBOX again", || box_subjects(&imap, "Archive").is_empty() && box_subjects(&imap, "INBOX").contains(&"Message 3".to_string()) && settled(&core));
    assert_eq!(box_subjects(&imap, "INBOX").len(), 3, "INBOX {:?}", box_subjects(&imap, "INBOX"));
    println!("PASS undo-after-archive-moved-the-message-back-to-the-servers-inbox: Archive empty, INBOX holds 3");
}
