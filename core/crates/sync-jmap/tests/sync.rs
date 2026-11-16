//! First screen before history, the mapping to the contract's Message, batch caps, and resync.
mod common;

use std::sync::Arc;

use common::*;
use contracts::SyncCursor;
use sync_jmap::{Discovery, FetchOptions, JmapAdapter, JmapMailAdapter, Resync};

const OPTS: FetchOptions = FetchOptions { preview_chars: 20 };

fn twelve() -> Vec<FEmail> {
    let mut v: Vec<FEmail> = (1..=12).map(FEmail::inbox).collect();
    v[11].keywords = vec!["$seen".into()];
    v[11].attachments = vec!["quote.pdf".into(), "terms.pdf".into()];
    let mut archived = FEmail::inbox(13);
    archived.mailboxes = vec!["m-archive".into()];
    v.push(archived);
    v
}

async fn adapter(fake: &Fake) -> JmapAdapter {
    match JmapAdapter::connect(fake.config()).await.expect("connect") {
        Discovery::Jmap(a) => a,
        Discovery::FallbackToImap { reason_words } => panic!("expected JMAP, got fallback: {reason_words}"),
    }
}

#[tokio::test]
async fn first_screen_reaches_the_core_before_history_is_requested() {
    let fake = Arc::new(Fake::start(Mode::Jmap, twelve(), 500).await);
    let a = adapter(&fake).await;
    let mut sink = RecSink::new(Some(fake.clone()));
    let cursor = a.initial_sync("m-inbox", 5, 4, OPTS, &mut sink).await.expect("initial sync");

    // Cursor is the Email state at the first screen's moment.
    assert_eq!(cursor, SyncCursor::Jmap { state: "s13".into() });
    // Deliveries: 5 (with the cursor), then 4, then 3; newest to oldest, each message once.
    let sizes: Vec<usize> = sink.deliveries.iter().map(|d| d.changed.len()).collect();
    assert_eq!(sizes, vec![5, 4, 3]);
    assert_eq!(sink.deliveries[0].cursor, Some(SyncCursor::Jmap { state: "s13".into() }));
    assert!(sink.deliveries[1].cursor.is_none() && sink.deliveries[2].cursor.is_none(), "a history page never moves the cursor");
    let all: Vec<String> = sink.deliveries.iter().flat_map(ids_of).collect();
    let want: Vec<String> = (1..=12).rev().map(|n| format!("e{n:02}")).collect();
    assert_eq!(all, want);

    // The ORDER of the story: the first query, then the first delivery, and only then the history query.
    let log: Vec<String> = fake.log().into_iter().filter(|l| l.starts_with("query") || l.starts_with("deliver")).collect();
    assert_eq!(log[0], "query mailbox=m-inbox anchor=- limit=5");
    assert_eq!(log[1], "deliver n=5 vanished=0 cursor=true");
    assert_eq!(log[2], "query mailbox=m-inbox anchor=e08 limit=4");
    assert_eq!(log[3], "deliver n=4 vanished=0 cursor=false");
    fake.shutdown().await;
}

#[tokio::test]
async fn a_message_maps_to_the_contract_record_in_the_servers_words() {
    let fake = Fake::start(Mode::Jmap, twelve(), 500).await;
    let a = adapter(&fake).await;
    let first = a.first_screen("m-inbox", 1, OPTS).await.unwrap();
    let m = &first.changed[0];
    assert_eq!(m.server_id, "e12");
    assert_eq!(m.message.id, "acct-1:e12");
    assert_eq!(m.message.thread_id, "acct-1:t12");
    assert_eq!(m.message.account_id, "acct-1");
    assert_eq!(m.message.sender_name, "Sender 12");
    assert_eq!(m.message.from_address, "sender12@example.test");
    assert_eq!(m.message.to, vec!["owner@example.test".to_string()]);
    assert_eq!(m.message.subject, "Subject 12");
    assert_eq!(m.message.preview, "Preview text of mess", "the preview is cut to the caller's length");
    assert_eq!(m.message.received_at_ms, FEmail::inbox(12).received_ms());
    assert!(m.message.read, "$seen means read");
    assert_eq!(m.message.folder, "Inbox");
    assert!(!m.message.archived);
    assert_eq!(m.message.attachment_names, vec!["quote.pdf".to_string(), "terms.pdf".to_string()]);
    assert_eq!(m.keywords, vec!["$seen".to_string()]);

    let unread = a.first_screen("m-inbox", 2, OPTS).await.unwrap();
    assert!(!unread.changed[1].message.read, "no $seen means unread");

    let archive = a.first_screen("m-archive", 5, OPTS).await.unwrap();
    assert_eq!(archive.changed.len(), 1);
    assert!(archive.changed[0].message.archived);
    assert_eq!(archive.changed[0].message.folder, "Archive");

    let folders = a.list_folders().await.unwrap();
    let inbox = folders.iter().find(|f| f.id == "m-inbox").unwrap();
    assert_eq!((inbox.role.as_deref(), inbox.name.as_str(), inbox.total, inbox.unread), (Some("inbox"), "Inbox", 12, 11));
    fake.shutdown().await;
}

#[tokio::test]
async fn no_request_asks_for_more_than_the_servers_declared_get_limit() {
    let fake = Arc::new(Fake::start(Mode::Jmap, twelve(), 3).await);
    let a = adapter(&fake).await;
    let mut sink = RecSink::new(Some(fake.clone()));
    a.initial_sync("m-inbox", 8, 8, OPTS, &mut sink).await.unwrap();
    let all: Vec<String> = sink.deliveries.iter().flat_map(ids_of).collect();
    assert_eq!(all.len(), 12, "all twelve arrive even though the server caps a request at three");
    for line in fake.log() {
        if let Some(n) = line.strip_prefix("query ").and_then(|l| l.rsplit("limit=").next()).and_then(|n| n.parse::<usize>().ok()) {
            assert!(n <= 3, "a query asked for {n}: {line}");
        }
    }
    fake.shutdown().await;
}

#[tokio::test]
async fn resync_reports_new_changed_and_destroyed_messages_and_moves_the_cursor() {
    let fake = Fake::start(Mode::Jmap, twelve(), 500).await;
    let a = adapter(&fake).await;
    let first = a.first_screen("m-inbox", 3, OPTS).await.unwrap();
    let cursor = first.cursor.clone().unwrap();

    fake.add_email(FEmail::inbox(14));
    fake.mark_seen("e11");
    fake.destroy("e10");
    let Resync::Delta(d) = a.resync(&cursor, OPTS).await.unwrap() else { panic!("expected a delta") };
    let mut ids = ids_of(&d);
    ids.sort();
    assert_eq!(ids, vec!["e11".to_string(), "e14".to_string()]);
    assert_eq!(d.vanished, vec!["e10".to_string()]);
    assert_eq!(d.cursor, Some(SyncCursor::Jmap { state: fake.state() }));
    let seen = d.changed.iter().find(|m| m.server_id == "e11").unwrap();
    assert!(seen.message.read, "a keyword change comes back as the message's new state");

    // Nothing changed since the new cursor: an empty delta that still carries the cursor.
    let Resync::Delta(none) = a.resync(d.cursor.as_ref().unwrap(), OPTS).await.unwrap() else { panic!("expected a delta") };
    assert!(none.changed.is_empty() && none.vanished.is_empty());
    assert_eq!(none.cursor, d.cursor);
    fake.shutdown().await;
}

#[tokio::test]
async fn a_cursor_the_server_cannot_use_resets_instead_of_guessing() {
    let fake = Fake::start(Mode::Jmap, twelve(), 500).await;
    let a = adapter(&fake).await;
    let r = a.resync(&SyncCursor::Jmap { state: "garbage".into() }, OPTS).await.unwrap();
    assert!(matches!(r, Resync::Reset { .. }), "{r:?}");
    let wrong = a.resync(&SyncCursor::GraphDelta { token: "x".into() }, OPTS).await;
    assert!(wrong.is_err(), "a Graph cursor is not a JMAP cursor");
    fake.shutdown().await;
}

#[tokio::test]
async fn resync_fetches_in_chunks_no_bigger_than_the_get_limit() {
    let fake = Fake::start(Mode::Jmap, twelve(), 2).await;
    let a = adapter(&fake).await;
    let cursor = SyncCursor::Jmap { state: fake.state() };
    for n in 20..25 {
        fake.add_email(FEmail::inbox(n));
    }
    let Resync::Delta(d) = a.resync(&cursor, OPTS).await.unwrap() else { panic!("expected a delta") };
    assert_eq!(d.changed.len(), 5);
    let gets: Vec<String> = fake.log().into_iter().filter(|l| l.starts_with("get ids=")).collect();
    assert_eq!(gets, vec!["get ids=2", "get ids=2", "get ids=1"]);
    fake.shutdown().await;
}
