//! gap-engine-tests (item 44d21658): the engine's end of the plain resync. On a server with NO change lists (no CONDSTORE, no QRESYNC;
//! Microsoft 365 / Exchange Online) the adapter compares the UIDs the store knows with what `UID SEARCH UID 1:*` lists
//! (sync-imap/tests/a144_plain_resync.rs judges those lists); these tests judge what the ENGINE does with them, through the real sync over the
//! in-process IMAP fake (sync-imap's test support), woken by IDLE as a live server wakes it:
//!   (a) a mail moved out of INBOX on the server leaves INBOX in the store at the next sync;
//!   (b) a mail the owner archived with E stays (archived, in the store) when its INBOX UID vanishes;
//!   (c) a flag set on the server follows, and a second resync with no change writes nothing to the message table.
//! RED-FIRST broken copies (one edit each, one at a time, the lead runs them):
//!   (a) core/crates/sync-imap/src/adapter.rs `.partition(|u| live.contains(u))` -> `.partition(|_| true)` (nothing is ever reported vanished)
//!   (b) core/crates/engine/src/sync.rs `if row.is_some_and(|m| m.archived &&` -> `if false && row.is_some_and(|m| m.archived &&` (the owner's row is deleted)
//!   (c) core/crates/sync-imap/src/adapter.rs `if chunk.binary_search(&uid).is_ok() {` -> `if false {` (no flag change is ever reported)
//!   (c, second half) core/crates/engine/src/sync.rs `if was_read != Some(seen) {` -> `if true {` (every repeated flag is written again)
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::{Store, TableChange};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(20) {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within 20 s: {what}");
}

fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

struct Synced { core: RustCore, account: String, imap: FakeImap, _smtp: FakeSmtp, _rt: tokio::runtime::Runtime, _dir: tempfile::TempDir }

/// An account with 3 INBOX mails synced over a fake that has NO change lists, listening by IDLE.
fn synced_without_change_lists() -> Synced {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| { s.qresync = false; s.cap_condstore = false; });
    imap.seed("INBOX", 3);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(engine::SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    wait_until("the three mails stored", || core.store().uids_in_folder(&account, "INBOX").unwrap().len() == 3);
    // the fake counts an IDLE as started before it waits for a change, and drops a change announced in between: let it settle
    std::thread::sleep(Duration::from_millis(300));
    assert!(!imap.log().iter().any(|l| l.contains("CHANGEDSINCE") || l.contains("QRESYNC (")), "the fake really has no change lists: {:?}", imap.log());
    Synced { core, account, imap, _smtp: smtp, _rt: rt, _dir: dir }
}

/// Wakes the IDLE of INBOX with no change of any mail (the fake announces a changed mailbox, the engine resyncs).
fn wake(imap: &FakeImap) { imap.change_flags("INBOX", &[], "\\Seen"); }

/// The server moves one INBOX mail to Archive (what a rule or another client does): the UID leaves INBOX, a copy gets a UID of Archive.
fn move_out_of_inbox(imap: &FakeImap, uid: u32) {
    let msg = imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        let at = b.msgs.iter().position(|m| m.uid == uid).expect("the mail is in INBOX on the server");
        let msg = b.msgs.remove(at);
        b.highest_modseq += 1;
        b.expunged.push((uid, b.highest_modseq));
        b.version += 1;
        msg
    });
    imap.with(|s| {
        let a = s.boxes.iter_mut().find(|b| b.name == "Archive").unwrap();
        let new_uid = a.next_uid;
        a.next_uid += 1;
        a.highest_modseq += 1;
        a.msgs.push(imap_fakes::Msg { uid: new_uid, flags: msg.flags, modseq: a.highest_modseq, raw: msg.raw, gm: None });
        a.version += 1;
    });
    wake(imap);
}

fn inbox_ids(s: &Synced) -> Vec<(u32, String)> {
    let mut v = s.core.store().uids_in_folder(&s.account, "INBOX").unwrap();
    v.sort();
    v
}

/// (a) Red-first: adapter.rs `.partition(|u| live.contains(u))` -> `.partition(|_| true)`.
#[test]
fn a_mail_moved_out_of_inbox_on_a_server_without_change_lists_leaves_inbox_in_the_store() {
    let s = synced_without_change_lists();
    let before = inbox_ids(&s);
    assert_eq!(before.len(), 3, "three INBOX mails stored: {before:?}");
    let (moved_uid, moved_id) = before[1].clone();
    move_out_of_inbox(&s.imap, moved_uid);
    wait_until("INBOX holds two rows in the store", || inbox_ids(&s).len() == 2);
    let after = inbox_ids(&s);
    assert!(!after.iter().any(|(_, id)| *id == moved_id), "the moved mail is no longer an INBOX row: {after:?}");
    assert_eq!(after.iter().map(|(u, _)| *u).collect::<Vec<_>>(), vec![before[0].0, before[2].0], "the other two stay");
    let store = s.core.store();
    let in_inbox = store.message(&moved_id).unwrap().is_some_and(|m| m.folder.eq_ignore_ascii_case("INBOX"));
    assert!(!in_inbox, "the moved mail's INBOX row is gone (it is deleted, or filed elsewhere), never duplicated in INBOX");
    assert!(!s.imap.log().iter().any(|l| l.contains("CHANGEDSINCE") || l.contains("QRESYNC (")), "no change list was asked of a server without one");
}

/// (b) Red-first: sync.rs `if row.is_some_and(|m| m.archived &&` -> `if false && row.is_some_and(|m| m.archived &&`.
#[test]
fn a_mail_archived_with_e_stays_on_a_server_without_change_lists() {
    let s = synced_without_change_lists();
    let store = s.core.store();
    let (uid3, m3) = store.uids_in_folder(&s.account, "INBOX").unwrap().into_iter().max().unwrap();
    s.core.run(Command::Archive { message_ids: vec![m3.clone()] }).expect("archived");
    wait_until("the server moved its copy to Archive", || { s.core.run_write_back(); !s.imap.uids("INBOX").contains(&uid3) && store.message_place(&m3).unwrap().is_some() });
    let place = store.message_place(&m3).unwrap().unwrap();
    // a new arrival wakes the IDLE connection: the plain resync lists INBOX, and uid3 is no longer in it
    let new_uid = s.imap.add_message("INBOX");
    wait_until("the arrival is stored", || store.uids_in_folder(&s.account, "INBOX").unwrap().iter().any(|(u, _)| *u == new_uid));
    wait_until("the resync applied the vanish", || !store.uids_in_folder(&s.account, "INBOX").unwrap().iter().any(|(u, _)| *u == uid3));
    let kept = store.message(&m3).unwrap().expect("the archived message is still stored");
    assert!(kept.archived && kept.folder == "INBOX", "{kept:?}");
    assert_eq!(store.message_place(&m3).unwrap(), Some(place), "its server place is the Archive copy");
    let archive = InboxQuery { mailbox: Some(Mailbox::Archive), tab_id: "primary".into(), account_id: None, label_id: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    assert!(s.core.inbox(archive).iter().any(|r| r.message_id == m3), "the Archive view still lists it");
    assert!(!s.imap.log().iter().any(|l| l.contains("CHANGEDSINCE") || l.contains("QRESYNC (")), "no change list was asked of a server without one");
}

/// (c) Red-first: adapter.rs `if chunk.binary_search(&uid).is_ok() {` -> `if false {` (first half), sync.rs `if was_read != Some(seen) {` -> `if true {` (second half).
#[test]
fn a_flag_set_on_the_server_follows_on_a_server_without_change_lists() {
    let s = synced_without_change_lists();
    let store = s.core.store();
    let messages_written = Arc::new(AtomicU32::new(0));
    let counter = messages_written.clone();
    store.subscribe_tables(Arc::new(move |c: &TableChange| { if c.table == "message" { counter.fetch_add(c.inserts + c.updates + c.deletes, Ordering::SeqCst); } }));
    let (uid2, m2) = inbox_ids(&s)[1].clone();
    assert!(!store.message(&m2).unwrap().unwrap().read, "unread to begin with");
    s.imap.change_flags("INBOX", &[uid2], "\\Seen");
    wait_until("the row reads read", || store.message(&m2).unwrap().is_some_and(|m| m.read));
    let others_read = inbox_ids(&s).iter().filter(|(u, _)| *u != uid2).filter(|(_, id)| store.message(id).unwrap().is_some_and(|m| m.read)).count();
    assert_eq!(others_read, 0, "only the mail whose flag was set is read");
    // a second resync with no change: the server repeats every known UID's flags, the engine writes none of them
    std::thread::sleep(Duration::from_millis(300));
    let written = messages_written.load(Ordering::SeqCst);
    s.imap.clear_log();
    wake(&s.imap);
    wait_until("the second resync read the flags again", || s.imap.log().iter().any(|l| l.contains("UID FETCH") && l.contains("(UID FLAGS)")));
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(messages_written.load(Ordering::SeqCst), written, "a resync that found nothing new wrote nothing to the message table");
    assert!(store.message(&m2).unwrap().unwrap().read, "and the row still reads read");
}
