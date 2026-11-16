//! Round 2: Gmail's message id and labels, the per-provider connection table, IDLE reconnect with
//! backoff, and the redacted protocol transcript.
mod support;

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use contracts::CoreError;
use support::*;
use sync_imap::*;
use tokio::sync::Notify;

const ALL_MAIL: &str = "[Gmail]/All Mail";

async fn gmail_server() -> FakeImap {
    let srv = FakeImap::start().await;
    srv.with(|s| {
        s.gmail = true;
        s.qresync = false; // Gmail offers CONDSTORE but not QRESYNC
        s.boxes[5].name = ALL_MAIL.into();
        s.boxes[5].special = Some("\\All");
    });
    srv
}

async fn sync(a: &ImapAdapter, folder: &str) -> Vec<FetchedMessage> {
    let mut sink = Sink::default();
    a.initial_sync(folder, 50, 50, OPTS, &mut sink).await.expect("sync");
    sink.msgs
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gmail_message_under_two_label_folders_is_stored_once_with_its_labels() {
    let srv = gmail_server().await;
    srv.add_gmail_message("INBOX", 77, &["\\Inbox", "Work", "Big Client"]);
    srv.add_gmail_message(ALL_MAIL, 77, &["\\Inbox", "Work", "Big Client"]);
    srv.add_gmail_message(ALL_MAIL, 78, &[]);
    let a = adapter(&srv, "gm");
    let mut all = sync(&a, "INBOX").await;
    all.extend(sync(&a, ALL_MAIL).await);

    assert_eq!(all.len(), 3, "three copies on the server");
    let ids: BTreeSet<&str> = all.iter().map(|m| m.message.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "two messages: {ids:?}");
    assert!(ids.contains("gm:gm:4d") && ids.contains("gm:gm:4e"), "{ids:?}");
    let seventy_seven: Vec<_> = all.iter().filter(|m| m.message.id == "gm:gm:4d").collect();
    assert_eq!(seventy_seven.len(), 2);
    for m in &seventy_seven {
        let g = m.gmail.as_ref().expect("gmail identity");
        assert_eq!(g.msg_id, 77);
        assert_eq!(g.labels, vec!["\\Inbox".to_string(), "Work".into(), "Big Client".into()]);
    }
    // The two copies stay distinguishable by where they sit, for the store's location rows.
    let places: BTreeSet<(String, u32)> = seventy_seven.iter().map(|m| (m.message.folder.clone(), m.uid)).collect();
    assert_eq!(places.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_gmail_extension_the_same_text_in_two_folders_stays_two_messages() {
    let srv = FakeImap::start().await;
    srv.add_gmail_message("INBOX", 77, &["Work"]);
    srv.add_gmail_message("Archive", 77, &["Work"]);
    let a = adapter(&srv, "plain");
    let mut all = sync(&a, "INBOX").await;
    all.extend(sync(&a, "Archive").await);
    let ids: BTreeSet<&str> = all.iter().map(|m| m.message.id.as_str()).collect();
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(all.iter().all(|m| m.gmail.is_none()));
    assert!(!srv.log().iter().any(|l| l.contains("X-GM")), "the extension must not be asked for");
}

#[tokio::test(flavor = "multi_thread")]
async fn gmail_flag_changes_name_the_message_by_its_gmail_id() {
    let srv = gmail_server().await;
    srv.add_gmail_message("INBOX", 90, &["\\Inbox"]);
    let a = adapter(&srv, "gm2");
    let mut sink = Sink::default();
    let snap = a.initial_sync("INBOX", 50, 50, OPTS, &mut sink).await.unwrap();
    srv.change_flags("INBOX", &[1], "\\Seen");
    srv.clear_log();
    let Resync::Delta(d) = a.resync("INBOX", &snap.cursor, &[1], OPTS).await.unwrap() else { panic!("expected a delta") };
    assert_eq!(d.flag_changes.len(), 1);
    assert_eq!(d.flag_changes[0].gmail_msg_id, Some(90));
    assert!(srv.log().iter().any(|l| l.contains("CHANGEDSINCE") && l.contains("X-GM-MSGID")));
}

#[test]
fn the_table_gives_gmail_its_own_cap_and_every_other_host_the_default() {
    let l = limits();
    assert_ne!(l.max_gmail, l.max_default, "the test needs two different numbers");
    for host in ["imap.gmail.com", "IMAP.GMAIL.COM", "imap.googlemail.com"] {
        let t = l.table_for(host);
        assert_eq!((t.provider, t.max_connections), (Provider::Gmail, l.max_gmail), "{host}");
    }
    for host in ["imap.example.test", "mail.example.org", "127.0.0.1"] {
        let t = l.table_for(host);
        assert_eq!((t.provider, t.max_connections), (Provider::Other, l.max_default), "{host}");
    }
    let t = l.table_for("imap.example.test");
    assert_eq!(t.idle_connections, 1, "one IDLE connection per account");
    // The reconnect wait doubles from the ledger's start value and stops at its maximum.
    assert_eq!(t.reconnect_delay(0), Duration::from_millis(l.backoff_start_ms));
    assert_eq!(t.reconnect_delay(1), Duration::from_millis(l.backoff_start_ms * 2));
    assert_eq!(t.reconnect_delay(60), Duration::from_millis(l.backoff_max_ms));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_idle_on_the_same_account_is_refused() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 1);
    let a = adapter(&srv, "idle1");
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let first = tokio::spawn({
        let (a, clock, stop) = (a.clone(), clock.clone(), stop.clone());
        async move { a.idle("INBOX", &*clock, &stop, &mut |_| {}).await }
    });
    srv.wait_for("the first IDLE", |s| s.idle_starts == 1).await;
    let second = tokio::time::timeout(Duration::from_secs(5), a.idle("INBOX", &*clock, &stop, &mut |_| {})).await.expect("the second IDLE must be refused, not wait");
    match second {
        Err(CoreError::Action { failure }) => assert!(failure.reason_words.contains("already open"), "{}", failure.reason_words),
        other => panic!("expected a refusal, got {other:?}"),
    }
    stop.notify_one();
    first.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_idle_connection_is_reopened_after_the_doubling_backoff_within_the_cap() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 1);
    let a = adapter(&srv, "idle2");
    let table = a.connection_table().clone();
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let h = tokio::spawn({
        let (a, clock, stop) = (a.clone(), clock.clone(), stop.clone());
        async move { a.idle_supervised("INBOX", &*clock, &stop, &mut |_| {}).await }
    });
    srv.wait_for("the first IDLE", |s| s.idle_starts == 1).await;

    let start = table.reconnect_delay(0);
    let almost = |d: Duration| d - Duration::from_millis(1);
    srv.drop_idle();
    // The first reconnect waits the start value: one millisecond short is not enough. The 29-minute
    // IDLE timer is gone once the connection is, so the one sleeper left is the backoff.
    srv.wait_for("the dropped connection to close", |s| s.open_now == 0).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(clock.sleepers(), 1, "exactly the backoff timer should be waiting");
    clock.advance(almost(start));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(srv.with(|s| s.idle_starts), 1, "reconnected before the backoff ended");
    clock.advance(Duration::from_millis(1));
    srv.wait_for("the second IDLE", |s| s.idle_starts == 2).await;

    // The second drop waits twice as long.
    let second = table.reconnect_delay(1);
    assert_eq!(second, start * 2);
    srv.drop_idle();
    srv.wait_for("the second connection to close", |s| s.open_now == 0).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(clock.sleepers(), 1, "exactly the backoff timer should be waiting");
    clock.advance(almost(second));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(srv.with(|s| s.idle_starts), 2, "the second wait was not doubled");
    clock.advance(Duration::from_millis(1));
    srv.wait_for("the third IDLE", |s| s.idle_starts == 3).await;

    assert!(srv.with(|s| s.max_open) <= table.max_connections, "more connections than the table allows");
    stop.notify_one();
    h.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_transcript_keeps_commands_and_hides_passwords_tokens_and_message_text() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 3);
    let pwd = adapter(&srv, "log1");
    pwd.list_folders().await.unwrap();
    sync(&pwd, "INBOX").await;
    let text = pwd.transcript().text();
    assert!(text.contains("C: ") && text.contains("S: "), "{text}");
    assert!(text.contains("LOGIN \"owner@example.test\" [password redacted]") || text.contains("LOGIN owner@example.test [password redacted]"), "{text}");
    assert!(text.contains("FETCH") && text.contains("LIST"), "{text}");
    assert!(!text.contains("app-password"), "the password is in the log:\n{text}");
    assert!(!text.contains("Body of message") && !text.contains("Subject: Message"), "message text is in the log:\n{text}");
    assert!(text.contains("bytes of message data not logged"), "{text}");

    let tok = ImapAdapter::new(config(&srv, "log2", Auth::XOAuth2(Arc::new(FixedTok("token-1")))));
    tok.list_folders().await.unwrap();
    let text = tok.transcript().text();
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let encoded = STANDARD.encode(format!("user=owner@example.test\x01auth=Bearer token-1\x01\x01"));
    assert!(text.contains("AUTHENTICATE XOAUTH2") && text.contains("[credential redacted]"), "{text}");
    assert!(!text.contains("token-1") && !text.contains(&encoded), "the token is in the log:\n{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_transcript_is_a_ring_of_the_configured_size() {
    let srv = FakeImap::start().await;
    let mut cfg = config(&srv, "ring", Auth::Password("app-password".into()));
    cfg.limits.transcript_lines = 20;
    let a = ImapAdapter::new(cfg);
    for _ in 0..10 { a.list_folders().await.unwrap(); }
    let lines = a.transcript().lines();
    assert_eq!(lines.len(), 20);
    assert!(!lines.iter().any(|l| l.contains("fake ready")), "the oldest line (the greeting) must have been pushed out");
    assert!(lines.last().unwrap().starts_with("S: "), "the newest line is the last answer");
}
