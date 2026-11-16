//! Round 2, sync state: CONDSTORE/QRESYNC resync, the per-folder numbers the store keeps, and the
//! identity (account, folder, uidvalidity, uid). Everything runs against the in-process fake.
mod support;

use std::collections::BTreeSet;

use contracts::SyncCursor;
use support::*;
use sync_imap::*;

async fn synced(srv: &FakeImap, a: &ImapAdapter, folder: &str) -> (FolderSnapshot, Vec<u32>) {
    let mut sink = Sink::default();
    let snap = a.initial_sync(folder, 50, 50, OPTS, &mut sink).await.expect("initial sync");
    let mut known: Vec<u32> = sink.msgs.iter().map(|m| m.uid).collect();
    known.sort_unstable();
    let _ = srv;
    (snap, known)
}

fn delta(r: Resync) -> Delta {
    match r { Resync::Delta(d) => d, Resync::Reset { reason_words } => panic!("expected a delta, got a reset: {reason_words}") }
}

#[tokio::test(flavor = "multi_thread")]
async fn qresync_select_returns_the_changed_and_vanished_uids_in_one_command() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 20);
    let a = adapter(&srv, "qr");
    let (snap, known) = synced(&srv, &a, "INBOX").await;
    srv.change_flags("INBOX", &[3, 4], "\\Seen");
    srv.expunge("INBOX", &[7]);
    let arrived = srv.add_message("INBOX");
    srv.clear_log();

    let d = delta(a.resync("INBOX", &snap.cursor, &known, OPTS).await.expect("resync"));
    let mut changed: Vec<u32> = d.flag_changes.iter().map(|c| c.uid).collect();
    changed.sort_unstable();
    assert_eq!(changed, vec![3, 4]);
    assert!(d.flag_changes.iter().all(|c| c.flags.iter().any(|f| f == "\\Seen")));
    assert_eq!(d.vanished, vec![7]);
    assert_eq!(d.new.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![arrived]);

    let log = srv.log();
    assert!(log.iter().any(|l| l.contains("SELECT") && l.contains("QRESYNC (")), "no SELECT with QRESYNC in {log:?}");
    assert!(!log.iter().any(|l| l.contains("CHANGEDSINCE")), "the older fetch was used: {log:?}");
    assert!(!log.iter().any(|l| l.contains("SEARCH")), "a SEARCH was needed: {log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_qresync_the_condstore_uid_range_path_stays() {
    let srv = FakeImap::start().await;
    srv.with(|s| s.qresync = false);
    srv.seed("INBOX", 20);
    let a = adapter(&srv, "cs");
    let (snap, known) = synced(&srv, &a, "INBOX").await;
    srv.change_flags("INBOX", &[5], "\\Flagged");
    srv.expunge("INBOX", &[9]);
    srv.clear_log();

    let d = delta(a.resync("INBOX", &snap.cursor, &known, OPTS).await.expect("resync"));
    assert_eq!(d.flag_changes.iter().map(|c| c.uid).collect::<Vec<_>>(), vec![5]);
    assert_eq!(d.vanished, vec![9]);
    let log = srv.log();
    assert!(log.iter().any(|l| l.contains("CHANGEDSINCE")), "{log:?}");
    assert!(!log.iter().any(|l| l.contains("QRESYNC (")), "{log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_qresync_known_uids_out_of_order_still_use_the_highest_one() {
    // The engine passes `store.uids_in_folder`, whose query has no ORDER BY: the known UIDs may come in any order.
    // RED (search range): adapter.rs CONDSTORE branch `let gone = match known.iter().max() {` -> `let gone = match known.last() {`
    //   (the search covers UID 1:1 only, so 5, 3 and 2 read as vanished)
    // RED (max_known): adapter.rs `let max_known = known.iter().copied().max().unwrap_or(0);` -> `let max_known = known.last().copied().unwrap_or(0);`
    //   (the flag change on 5 reads as a new mail)
    let srv = FakeImap::start().await;
    srv.with(|s| s.qresync = false);
    srv.seed("INBOX", 5);
    let a = adapter(&srv, "order");
    let (snap, mut known) = synced(&srv, &a, "INBOX").await;
    known.reverse(); // 5, 4, 3, 2, 1: the last one is the lowest
    srv.change_flags("INBOX", &[5], "\\Flagged");
    srv.expunge("INBOX", &[4]);
    srv.clear_log();

    let d = delta(a.resync("INBOX", &snap.cursor, &known, OPTS).await.expect("resync"));
    assert_eq!(d.vanished, vec![4], "only the expunged UID");
    assert_eq!(d.flag_changes.iter().map(|c| c.uid).collect::<Vec<_>>(), vec![5], "a flag change on a known UID");
    assert!(d.new.is_empty(), "no known UID is new: {:?}", d.new.iter().map(|m| m.uid).collect::<Vec<_>>());
    assert!(srv.log().iter().any(|l| l.contains("UID SEARCH UID 1:5")), "{:?}", srv.log());
}

// A144 (docs/SPEC.md:162): "A mail that left a folder on the server (moved, archived or deleted from another device) leaves that folder
// in the app at the next sync, on every server, also one without change lists (Microsoft 365)." This test once asserted `Reset` for a
// server without CONDSTORE that had known UIDs; that case now answers a `Delta` (tests/a144_plain_resync.rs holds the detail). With NO
// known UID there is nothing to compare, and it still answers `Reset`, as before.
#[tokio::test(flavor = "multi_thread")]
async fn a_server_with_no_condstore_and_known_uids_answers_a_delta_by_uid_range() {
    // RED: adapter.rs resync `return resync_plain(s, &f, ctx0, want_uv, &known, opts).await;`
    //   -> `return Ok(Resync::Reset { reason_words: "the server has no CONDSTORE, so changes cannot be listed".into() });`
    let srv = FakeImap::start().await;
    srv.with(|s| { s.qresync = false; s.cap_condstore = false; });
    srv.seed("INBOX", 5);
    let a = adapter(&srv, "plain");
    let (snap, known) = synced(&srv, &a, "INBOX").await;
    srv.clear_log();
    let d = delta(a.resync("INBOX", &snap.cursor, &known, OPTS).await.expect("resync"));
    assert_eq!(d.cursor, SyncCursor::Imap { uid_validity: 1000, highest_mod_seq: 0 }, "UIDVALIDITY kept, no modseq on this server");
    assert!(d.new.is_empty() && d.vanished.is_empty(), "nothing changed on the server");
    let log = srv.log();
    assert!(log.iter().any(|l| l.contains("UID SEARCH UID 1:*")), "{log:?}");
    assert!(!log.iter().any(|l| l.contains("CHANGEDSINCE") || l.contains("QRESYNC (")), "{log:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_with_no_condstore_and_no_known_uid_still_resets_so_the_folder_is_read_again() {
    // RED: adapter.rs resync_plain, inside `let Some(max_known) = .. else {`, the
    //   `return Ok(Resync::Reset { reason_words: "the server has no CONDSTORE, so changes cannot be listed".into() });`
    //   -> `return Ok(Resync::Delta(Delta { new: vec![], flag_changes: vec![], vanished: vec![], cursor: imap_cursor(0, 0) }));`
    let srv = FakeImap::start().await;
    srv.with(|s| { s.qresync = false; s.cap_condstore = false; });
    srv.seed("INBOX", 5);
    let a = adapter(&srv, "plain");
    let (snap, _known) = synced(&srv, &a, "INBOX").await;
    match a.resync("INBOX", &snap.cursor, &[], OPTS).await.expect("resync") {
        Resync::Reset { reason_words } => assert!(reason_words.contains("CONDSTORE"), "{reason_words}"),
        other => panic!("expected a reset, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_sync_state_exposes_modseq_and_uidvalidity_for_each_folder() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 5);
    srv.seed("Sent", 2);
    srv.change_flags("Sent", &[1], "\\Seen");
    let a = adapter(&srv, "st");
    let inbox = a.folder_state("INBOX").await.expect("inbox state");
    let sent = a.folder_state("Sent").await.expect("sent state");
    let (inbox_want, sent_want) = srv.with(|s| {
        let f = |n: &str| { let b = s.boxes.iter().find(|b| b.name == n).unwrap(); (b.uid_validity, b.highest_modseq) };
        (f("INBOX"), f("Sent"))
    });
    assert_eq!((inbox.uid_validity, inbox.highest_modseq), (inbox_want.0, Some(inbox_want.1)));
    assert_eq!((sent.uid_validity, sent.highest_modseq), (sent_want.0, Some(sent_want.1)));
    assert_ne!(inbox.highest_modseq, sent.highest_modseq, "each folder has its own modseq");
    assert!(inbox.condstore && inbox.qresync);
    assert_eq!(inbox.cursor(), SyncCursor::Imap { uid_validity: u64::from(inbox_want.0), highest_mod_seq: inbox_want.1 });

    // The snapshot a first sync returns carries the same two numbers, and a second resync from it
    // finds nothing and fetches nothing.
    let (snap, known) = synced(&srv, &a, "INBOX").await;
    assert_eq!(snap.cursor, inbox.cursor());
    let before = srv.with(|s| s.fetch_items);
    let d = delta(a.resync("INBOX", &snap.cursor, &known, OPTS).await.expect("resync"));
    assert!(d.new.is_empty() && d.flag_changes.is_empty() && d.vanished.is_empty());
    assert_eq!(d.cursor, snap.cursor);
    assert_eq!(srv.with(|s| s.fetch_items), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_uidvalidity_drops_the_old_cache_with_and_without_qresync() {
    for qresync in [true, false] {
        let srv = FakeImap::start().await;
        srv.with(|s| s.qresync = qresync);
        srv.seed("INBOX", 6);
        let a = adapter(&srv, "uv");
        let (snap, known) = synced(&srv, &a, "INBOX").await;
        let mut old = Sink::default();
        a.initial_sync("INBOX", 50, 50, OPTS, &mut old).await.unwrap();
        let old_ids: BTreeSet<String> = old.msgs.iter().map(|m| m.message.id.clone()).collect();

        srv.set_uid_validity("INBOX", 2000);
        match a.resync("INBOX", &snap.cursor, &known, OPTS).await.expect("resync") {
            Resync::Reset { reason_words } => assert!(reason_words.contains("UIDVALIDITY"), "{reason_words}"),
            other => panic!("qresync={qresync}: expected a reset, got {other:?}"),
        }
        // The caller then reads the folder again: every id carries the new UIDVALIDITY, so no
        // old row can be mistaken for a new message.
        let mut fresh = Sink::default();
        let snap2 = a.initial_sync("INBOX", 50, 50, OPTS, &mut fresh).await.unwrap();
        let fresh_ids: BTreeSet<String> = fresh.msgs.iter().map(|m| m.message.id.clone()).collect();
        assert!(fresh_ids.iter().all(|i| i.contains(":2000:")), "{fresh_ids:?}");
        assert!(old_ids.is_disjoint(&fresh_ids));
        assert!(matches!(snap2.cursor, SyncCursor::Imap { uid_validity: 2000, .. }));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn identity_is_account_folder_uidvalidity_and_uid_everywhere() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 1);
    srv.seed("Sent", 1);
    let mut ids = BTreeSet::new();
    for acct in ["a1", "a2"] {
        let a = adapter(&srv, acct);
        for folder in ["INBOX", "Sent"] {
            let mut sink = Sink::default();
            a.initial_sync(folder, 5, 5, OPTS, &mut sink).await.unwrap();
            assert_eq!(sink.msgs.len(), 1);
            let m = &sink.msgs[0];
            assert_eq!(m.message.id, format!("{acct}:{folder}:{}:{}", m.uid_validity, m.uid));
            assert_eq!((m.uid, m.uid_validity), (1, 1000));
            ids.insert(m.message.id.clone());
        }
    }
    // UID 1 exists in both folders of both accounts: four different messages.
    assert_eq!(ids.len(), 4, "{ids:?}");
}
