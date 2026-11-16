//! fn-sync-imap against a local IMAP server that the test starts and stops (tests/support).
//! Nothing here connects to a real host or reads a real credential.
mod support;

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use contracts::{CoreError, SyncCursor};
use support::*;
use sync_imap::*;
use tokio::sync::Notify;

/// The brief's "newest 50"; docs/CONSTANTS.md has no row for it (finding in docs/lanes/core-sync-imap.md).
const FIRST_SCREEN_PER_BRIEF: u32 = 50;
const OPTS: FetchOptions = FetchOptions { preview_bytes: 64 };

fn cfg(srv: &FakeImap, name: &str, auth: Auth) -> ImapConfig {
    ImapConfig {
        account_id: name.into(),
        account_name: name.into(),
        host: "127.0.0.1".into(),
        port: srv.port,
        security: Security::PlainLoopbackOnly,
        username: "owner@example.test".into(),
        auth,
        extra_roots_der: Vec::new(),
        limits: limits(),
    }
}
fn adapter(srv: &FakeImap, name: &str) -> Arc<ImapAdapter> {
    Arc::new(ImapAdapter::new(cfg(srv, name, Auth::Password("app-password".into()))))
}

struct Collect { state: Arc<Mutex<State>>, batches: Vec<Vec<FetchedMessage>>, log_at: Vec<Vec<String>> }
impl Collect {
    fn new(srv: &FakeImap) -> Self { Collect { state: srv.state.clone(), batches: Vec::new(), log_at: Vec::new() } }
    fn all(&self) -> Vec<&FetchedMessage> { self.batches.iter().flatten().collect() }
}
#[async_trait]
impl MessageSink for Collect {
    async fn deliver(&mut self, batch: Vec<FetchedMessage>) -> Result<(), CoreError> {
        self.log_at.push(self.state.lock().unwrap().log.clone());
        self.batches.push(batch);
        Ok(())
    }
}

async fn sync_all(a: &ImapAdapter, srv: &FakeImap, first: u32, page: u32) -> (FolderSnapshot, Collect) {
    let mut sink = Collect::new(srv);
    let snap = a.initial_sync("INBOX", first, page, OPTS, &mut sink).await.expect("initial sync");
    (snap, sink)
}

// ------------------------------------------------------------ first screen before history

#[tokio::test(flavor = "multi_thread")]
async fn newest_fifty_reach_the_sink_before_any_history_is_requested() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 500);
    let a = adapter(&srv, "work");
    let (snap, sink) = sync_all(&a, &srv, FIRST_SCREEN_PER_BRIEF, 100).await;

    let first: Vec<u32> = sink.batches[0].iter().map(|m| m.uid).collect();
    assert_eq!(first, (451..=500).rev().collect::<Vec<u32>>(), "the first batch is the newest 50, newest first");
    let at_first = &sink.log_at[0];
    assert!(at_first.iter().any(|l| l.contains("FETCH 451:*")), "first screen was fetched: {at_first:?}");
    assert!(!at_first.iter().any(|l| l.contains("SEARCH")), "no history lookup before the sink holds the first screen: {at_first:?}");
    assert_eq!(at_first.iter().filter(|l| l.contains("FETCH")).count(), 1, "exactly one fetch before the first delivery: {at_first:?}");

    let seen: BTreeSet<u32> = sink.all().iter().map(|m| m.uid).collect();
    assert_eq!(seen.len(), 500, "history completes the folder");
    let order: Vec<u32> = sink.all().iter().map(|m| m.uid).collect();
    assert!(order.windows(2).all(|w| w[0] > w[1]), "newest to oldest across batches");
    assert_eq!(snap.exists, 500);
    assert_eq!(snap.cursor, SyncCursor::Imap { uid_validity: 1000, highest_mod_seq: 501 });

    let m = &sink.batches[0][0];
    assert_eq!(m.message.subject, "Message 500");
    assert_eq!(m.message.sender_name, "Sender 500");
    assert_eq!(m.message.from_address, "sender500@example.test");
    assert_eq!(m.message.to, vec!["owner@example.test".to_string()]);
    assert_eq!(m.message.preview, "Body of message 500 is here.");
    assert!(!m.message.read);
    assert_eq!(m.message.account_id, "work");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_folder_syncs_to_an_empty_snapshot() {
    let srv = FakeImap::start().await;
    let a = adapter(&srv, "work");
    let (snap, sink) = sync_all(&a, &srv, FIRST_SCREEN_PER_BRIEF, 100).await;
    assert_eq!(snap.exists, 0);
    assert!(sink.all().is_empty());
}

// ------------------------------------------------------------ login

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_password_is_reported_in_the_servers_words_naming_the_account() {
    let srv = FakeImap::start().await;
    let a = ImapAdapter::new(cfg(&srv, "iCloud", Auth::Password("wrong".into())));
    match a.list_folders().await {
        Err(CoreError::Action { failure }) => {
            assert_eq!(failure.account_name, "iCloud");
            assert!(failure.reason_words.contains("Invalid credentials"), "{}", failure.reason_words);
            assert!(!failure.retryable);
        }
        other => panic!("expected a refused login, got {other:?}"),
    }
}

struct Tok(&'static str);
#[async_trait]
impl TokenSource for Tok {
    async fn access_token(&self) -> Result<String, String> { Ok(self.0.to_string()) }
}

#[tokio::test(flavor = "multi_thread")]
async fn xoauth2_logs_in_with_the_token_and_a_wrong_token_is_refused() {
    let srv = FakeImap::start().await;
    let good = ImapAdapter::new(cfg(&srv, "ms", Auth::XOAuth2(Arc::new(Tok("token-1")))));
    assert!(good.list_folders().await.is_ok());
    let bad = ImapAdapter::new(cfg(&srv, "ms", Auth::XOAuth2(Arc::new(Tok("stale")))));
    match bad.list_folders().await {
        Err(CoreError::Action { failure }) => assert!(failure.reason_words.contains("token refused"), "{}", failure.reason_words),
        other => panic!("expected a refused token, got {other:?}"),
    }
    // The wire format Microsoft documents: user=<u>^Aauth=Bearer <token>^A^A, base64.
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let raw = STANDARD.decode(xoauth2_initial_response("a@b.test", "T")).unwrap();
    assert_eq!(raw, b"user=a@b.test\x01auth=Bearer T\x01\x01");
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_imap_is_refused_for_a_host_that_is_not_this_mac() {
    let srv = FakeImap::start().await;
    let mut c = cfg(&srv, "x", Auth::Password("p".into()));
    c.host = "192.0.2.1".into(); // documentation address, never contacted: refused before connecting
    match ImapAdapter::new(c).list_folders().await {
        Err(CoreError::Action { failure }) => assert!(failure.reason_words.contains("not this Mac"), "{}", failure.reason_words),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// ------------------------------------------------------------ IDLE re-issue on a clock fake

async fn until(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..500 {
        if f() { return; }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never reached: {what}");
}

#[tokio::test(flavor = "multi_thread")]
async fn idle_is_ended_and_issued_again_exactly_at_the_ledger_interval() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 3);
    let a = adapter(&srv, "work");
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let events: Arc<Mutex<Vec<IdleEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let (a2, c2, s2, e2) = (a.clone(), clock.clone(), stop.clone(), events.clone());
    let task = tokio::spawn(async move {
        let mut cb = move |e| e2.lock().unwrap().push(e);
        a2.idle("INBOX", &*c2, &s2, &mut cb).await
    });

    srv.wait_for("first IDLE", |s| s.idle_starts == 1).await;
    until("the timer is armed", || clock.sleepers() == 1).await;

    clock.advance(Duration::from_secs((IMAP_IDLE_REISSUE_MIN - 1) * 60));
    tokio::time::sleep(Duration::from_millis(150)).await;
    srv.with(|s| { assert_eq!(s.idle_starts, 1, "one minute early: still the same IDLE"); assert_eq!(s.idle_dones, 0); });

    clock.advance(Duration::from_secs(60));
    srv.wait_for("IDLE issued again", |s| s.idle_starts == 2).await;
    let shape: Vec<String> = srv.log().into_iter().filter(|l| l.ends_with(" IDLE") || l.ends_with(" DONE")).map(|l| l.split(' ').last().unwrap().to_string()).collect();
    assert_eq!(shape, vec!["IDLE", "DONE", "IDLE"], "ended with DONE, then issued again");

    until("the timer is armed again", || clock.sleepers() == 1).await;
    srv.add_message("INBOX");
    until("a Changed event", || !events.lock().unwrap().is_empty()).await;
    assert_eq!(events.lock().unwrap()[0], IdleEvent::Changed);

    stop.notify_one();
    task.await.unwrap().expect("idle ends cleanly when stopped");
}

// ------------------------------------------------------------ CONDSTORE / QRESYNC resync

#[tokio::test(flavor = "multi_thread")]
async fn after_a_thousand_flag_changes_a_resync_transfers_only_the_changed_uids() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 3000);
    let a = adapter(&srv, "work");
    let (snap, _) = sync_all(&a, &srv, FIRST_SCREEN_PER_BRIEF, 1000).await;
    let known = srv.uids("INBOX");

    let changed: Vec<u32> = known.iter().copied().filter(|u| u % 3 == 0).collect();
    assert_eq!(changed.len(), 1000);
    srv.change_flags("INBOX", &changed, "\\Seen");
    let gone = [1u32, 2, 4, 5, 7];
    srv.expunge("INBOX", &gone);
    let fresh = [srv.add_message("INBOX"), srv.add_message("INBOX")];
    srv.with(|s| s.fetch_items = 0);

    let Resync::Delta(d) = a.resync("INBOX", &snap.cursor, &known, OPTS).await.unwrap() else { panic!("expected a delta") };
    let flag_uids: BTreeSet<u32> = d.flag_changes.iter().map(|c| c.uid).collect();
    assert_eq!(flag_uids, changed.iter().copied().collect::<BTreeSet<u32>>(), "exactly the changed UIDs");
    assert!(d.flag_changes.iter().all(|c| c.flags.contains(&"\\Seen".to_string())));
    assert_eq!(d.vanished, gone.to_vec());
    assert_eq!(d.new.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![fresh[1], fresh[0]]);
    // 1,000 changed + 2 new in the change list, then the 2 new fetched whole: nothing else moved.
    assert_eq!(srv.with(|s| s.fetch_items), changed.len() + 2 + 2, "the server sent only changed messages, not the 3,000");

    // Nothing changed since: the resync issues no FETCH at all.
    srv.clear_log();
    let Resync::Delta(again) = a.resync("INBOX", &d.cursor, &srv.uids("INBOX"), OPTS).await.unwrap() else { panic!("expected a delta") };
    assert!(again.new.is_empty() && again.flag_changes.is_empty() && again.vanished.is_empty());
    assert!(!srv.log().iter().any(|l| l.contains("FETCH")), "{:?}", srv.log());
}

#[tokio::test(flavor = "multi_thread")]
async fn without_qresync_vanished_uids_come_from_a_search_and_a_rebuilt_folder_resets() {
    let srv = FakeImap::start().await;
    srv.with(|s| s.qresync = false);
    srv.seed("INBOX", 10);
    let a = adapter(&srv, "work");
    let (snap, _) = sync_all(&a, &srv, FIRST_SCREEN_PER_BRIEF, 100).await;
    let known = srv.uids("INBOX");
    srv.expunge("INBOX", &[4]);
    srv.change_flags("INBOX", &[2], "\\Flagged");
    let Resync::Delta(d) = a.resync("INBOX", &snap.cursor, &known, OPTS).await.unwrap() else { panic!("expected a delta") };
    assert_eq!(d.vanished, vec![4]);
    assert_eq!(d.flag_changes.iter().map(|c| c.uid).collect::<Vec<_>>(), vec![2]);
    assert!(!srv.log().iter().any(|l| l.contains("VANISHED")));

    srv.set_uid_validity("INBOX", 2000);
    match a.resync("INBOX", &d.cursor, &srv.uids("INBOX"), OPTS).await.unwrap() {
        Resync::Reset { reason_words } => assert!(reason_words.contains("UIDVALIDITY"), "{reason_words}"),
        other => panic!("expected a reset, got {other:?}"),
    }
}

// ------------------------------------------------------------ folders, flags, move, append

#[tokio::test(flavor = "multi_thread")]
async fn folder_roles_come_from_special_use_and_from_names_when_the_server_has_none() {
    for (special, sent, trash) in [(true, "Sent", "Trash"), (false, "Sent Messages", "Deleted Messages")] {
        let srv = FakeImap::start_with(special).await;
        let folders = adapter(&srv, "a").list_folders().await.unwrap();
        let role = |n: &str| folders.iter().find(|f| f.name == n).unwrap().role;
        assert_eq!(role("INBOX"), FolderRole::Inbox);
        assert_eq!(role(sent), FolderRole::Sent, "special_use={special}");
        assert_eq!(role(trash), FolderRole::Trash);
        assert_eq!(role("Drafts"), FolderRole::Drafts);
        assert_eq!(role("Junk"), FolderRole::Junk);
        assert_eq!(role("Archive"), FolderRole::Archive);
    }
}

/// engine-act-r5: a move answers where the server put each message (RFC 4315 COPYUID), on the MOVE path (untagged answer) and on the
/// COPY + delete + expunge path (tagged answer); a server that names no place answers an empty list, and the move still happens.
#[tokio::test(flavor = "multi_thread")]
async fn a_move_answers_where_the_server_put_each_message() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 6);
    let a = adapter(&srv, "work");
    let uv = srv.with(|s| s.boxes.iter().find(|b| b.name == "Archive").unwrap().uid_validity);
    let placed = a.move_placed("INBOX", &[5, 2], "Archive").await.unwrap();
    assert_eq!(placed, vec![MovedUid { from_uid: 2, uid_validity: uv, to_uid: 1 }, MovedUid { from_uid: 5, uid_validity: uv, to_uid: 2 }], "UID MOVE");
    assert_eq!(srv.uids("Archive"), vec![1, 2]);

    srv.with(|s| s.move_cap = false);
    let b = adapter(&srv, "no-move");
    let placed = b.move_placed("INBOX", &[6], "Archive").await.unwrap();
    assert_eq!(placed, vec![MovedUid { from_uid: 6, uid_validity: uv, to_uid: 3 }], "UID COPY + delete + expunge");
    assert_eq!(srv.uids("INBOX"), vec![1, 3, 4]);

    srv.with(|s| s.copyuid = false);
    let placed = b.move_placed("INBOX", &[1], "Archive").await.unwrap();
    assert!(placed.is_empty(), "a server that names no place");
    assert_eq!(srv.uids("INBOX"), vec![3, 4], "the message moved all the same");
    assert_eq!(srv.uids("Archive").len(), 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn flags_move_and_append_to_sent() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 6);
    let a = adapter(&srv, "work");
    a.set_flags("INBOX", &[1, 2, 3], FlagOp::Add, &["\\Seen", "\\Flagged"]).await.unwrap();
    assert_eq!(srv.flags_of("INBOX", 2), vec!["\\Flagged", "\\Seen"]);
    a.set_flags("INBOX", &[2], FlagOp::Remove, &["\\Seen"]).await.unwrap();
    assert_eq!(srv.flags_of("INBOX", 2), vec!["\\Flagged"]);
    assert!(a.set_flags("INBOX", &[1], FlagOp::Add, &["\\Seen) (X"]).await.is_err(), "a flag cannot carry command syntax");

    a.move_within("INBOX", &[4, 5], "Archive").await.unwrap();
    assert_eq!(srv.uids("INBOX"), vec![1, 2, 3, 6]);
    assert_eq!(srv.uids("Archive").len(), 2);

    srv.with(|s| s.move_cap = false);
    let b = adapter(&srv, "no-move"); // a new connection learns the new capabilities
    b.move_within("INBOX", &[6], "Archive").await.unwrap();
    assert_eq!(srv.uids("INBOX"), vec![1, 2, 3], "COPY + delete + UID EXPUNGE path");
    assert_eq!(srv.uids("Archive").len(), 3);

    a.append_sent(&raw_message(900)).await.unwrap();
    let sent = srv.uids("Sent");
    assert_eq!(sent.len(), 1);
    assert_eq!(srv.flags_of("Sent", sent[0]), vec!["\\Seen"]);
}

// ------------------------------------------------------------ across accounts

#[tokio::test(flavor = "multi_thread")]
async fn move_and_copy_across_accounts_never_lose_a_message() {
    let (s1, s2) = (FakeImap::start().await, FakeImap::start().await);
    s1.seed("INBOX", 3);
    s1.change_flags("INBOX", &[2], "\\Flagged");
    let (a, b) = (adapter(&s1, "work"), adapter(&s2, "home"));

    transfer(&a, "INBOX", &[2], &b, "Archive", TransferMode::Copy).await.unwrap();
    assert_eq!(s1.uids("INBOX"), vec![1, 2, 3], "a copy leaves the source");
    assert_eq!(s2.uids("Archive").len(), 1);
    assert_eq!(s2.flags_of("Archive", 1), vec!["\\Flagged"], "flags travel with the message");

    transfer(&a, "INBOX", &[1, 3], &b, "Archive", TransferMode::Move).await.unwrap();
    assert_eq!(s1.uids("INBOX"), vec![2]);
    assert_eq!(s2.uids("Archive").len(), 3);

    s2.with(|s| s.refuse_append = true);
    match transfer(&a, "INBOX", &[2], &b, "Archive", TransferMode::Move).await {
        Err(CoreError::Action { failure }) => {
            assert_eq!(failure.account_name, "home", "the refusing account is named");
            assert!(failure.reason_words.contains("Quota exceeded"), "{}", failure.reason_words);
        }
        other => panic!("expected the target's refusal, got {other:?}"),
    }
    assert_eq!(s1.uids("INBOX"), vec![2], "the source is untouched when the target refuses");
}

// ------------------------------------------------------------ connection cap

#[tokio::test(flavor = "multi_thread")]
async fn open_connections_never_exceed_the_ledger_cap_even_with_idle_and_a_burst() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 20);
    srv.with(|s| s.latency_ms = 15);
    let a = adapter(&srv, "work");
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let (a2, c2, s2) = (a.clone(), clock.clone(), stop.clone());
    let idle = tokio::spawn(async move { a2.idle("INBOX", &*c2, &s2, &mut |_| {}).await });
    srv.wait_for("IDLE running", |s| s.idle_starts == 1).await;

    let mut burst = Vec::new();
    for i in 1..=8u32 {
        let a = a.clone();
        burst.push(tokio::spawn(async move { a.set_flags("INBOX", &[i], FlagOp::Add, &["\\Seen"]).await }));
    }
    for h in burst { h.await.unwrap().expect("burst op"); }
    let max = srv.with(|s| s.max_open);
    assert_eq!(max, IMAP_CONN_PER_ACCOUNT, "the burst used every allowed connection and no more");
    stop.notify_one();
    idle.await.unwrap().unwrap();
}

// ------------------------------------------------------------ numbers live once

#[test]
fn no_source_file_retypes_a_ledger_number() {
    let ledger = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/CONSTANTS.md")).unwrap();
    let row = |name: &str| -> u64 { ledger.lines().find(|l| l.contains(&format!("| {name} |"))).unwrap().split('|').nth(2).unwrap().trim().parse().unwrap() };
    assert_eq!(IMAP_IDLE_REISSUE_MIN, row("IMAP_IDLE_REISSUE_MIN"));
    assert_eq!(IMAP_CONN_PER_ACCOUNT as u64, row("IMAP_CONN_PER_ACCOUNT"));
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap() {
        let p = entry.unwrap().path();
        let text = std::fs::read_to_string(&p).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap();
            for lit in [IMAP_IDLE_REISSUE_MIN.to_string(), (IMAP_IDLE_REISSUE_MIN * 60).to_string()] {
                assert!(!code.split(|c: char| !c.is_ascii_digit()).any(|t| t == lit), "{}:{} retypes {lit}", p.display(), n + 1);
            }
        }
    }
}
