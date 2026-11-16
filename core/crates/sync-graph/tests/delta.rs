mod support;
use std::sync::{atomic::{AtomicUsize, Ordering::SeqCst}, Arc, Mutex};

use contracts::{CoreError, SyncCursor};
use support::*;
use sync_graph::DeltaChange;

fn token_of(c: &SyncCursor) -> &str {
    match c {
        SyncCursor::GraphDelta { token } => token,
        other => panic!("not a graph cursor: {other:?}"),
    }
}

fn upsert(c: &DeltaChange) -> &contracts::Message {
    match c {
        DeltaChange::Upsert(m) => m,
        other => panic!("not an upsert: {other:?}"),
    }
}

#[tokio::test]
async fn first_round_follows_next_links_and_maps_messages() {
    let stub = start(0, |req| {
        if req.target.contains("skiptoken=page2-example") { Resp::ok(fixture("delta_page2.json")) } else { Resp::ok(fixture("delta_page1.json")) }
    })
    .await;
    let r = rig(&stub.base, 50, 3);
    let round = r.mailbox.delta_round("inbox-id", "Inbox", None).await.expect("round");
    assert_eq!(round.pages, 2);
    assert!(!round.reset);
    assert_eq!(round.changes.len(), 3);
    let m = upsert(&round.changes[0]);
    assert_eq!((m.id.as_str(), m.thread_id.as_str(), m.subject.as_str()), ("AAMkExample001=", "AAQkConvExampleA=", "Quarterly numbers"));
    assert_eq!((m.sender_name.as_str(), m.from_address.as_str(), m.account_id.as_str(), m.folder.as_str()), ("Dana Example", "dana@example.net", "acct-work-example", "Inbox"));
    assert_eq!(m.to, vec!["owner@example.net".to_string()]);
    assert!(!m.read);
    assert_eq!(m.received_at_ms, 1_793_607_300_000);
    // an empty display name falls back to the address
    assert_eq!(upsert(&round.changes[2]).sender_name, "lee@example.net");
    assert!(token_of(&round.cursor).contains("$deltatoken=round1-example"));
    // every request carried the token and asked for the page size and immutable ids
    for q in stub.requests() {
        assert_eq!(q.headers.get("authorization").map(String::as_str), Some(format!("Bearer {TOKEN}").as_str()));
        assert_eq!(q.headers.get("prefer").map(String::as_str), Some("odata.maxpagesize=50, IdType=\"ImmutableId\""));
    }
    assert!(stub.requests()[0].target.starts_with("/v1.0/me/mailFolders/inbox-id/messages/delta?"));
}

#[tokio::test]
async fn a_later_round_transfers_only_the_changed_messages() {
    let model = Arc::new(Mutex::new(Model::new(120)));
    let served = Arc::new(AtomicUsize::new(0));
    let stub = start(0, model_handler(model.clone(), served.clone())).await;
    let r = rig(&stub.base, 50, 3);

    let first = r.mailbox.delta_round("inbox-id", "Inbox", None).await.expect("first");
    assert_eq!((first.changes.len(), first.pages), (120, 3));
    assert_eq!(served.load(SeqCst), 120);

    {
        let mut m = model.lock().unwrap();
        m.mark_read("AAMkMsg007=");
        m.remove("AAMkMsg042=");
    }
    served.store(0, SeqCst);
    let before = stub.requests().len();
    let second = r.mailbox.delta_round("inbox-id", "Inbox", Some(&first.cursor)).await.expect("second");
    assert_eq!(second.pages, 1);
    assert_eq!(second.changes.len(), 2);
    assert_eq!(served.load(SeqCst), 2, "the wire carried only the two changed records");
    assert!(upsert(&second.changes[0]).read && upsert(&second.changes[0]).id == "AAMkMsg007=");
    assert_eq!(second.changes[1], DeltaChange::Removed { id: "AAMkMsg042=".into(), reason: "deleted".into() });
    let new_requests = stub.requests().split_off(before);
    assert_eq!(new_requests.len(), 1);
    assert!(new_requests[0].target.contains("$deltatoken=v0"), "round two started from the stored delta link: {}", new_requests[0].target);

    // nothing changed since: an empty round, still one request
    served.store(0, SeqCst);
    let third = r.mailbox.delta_round("inbox-id", "Inbox", Some(&second.cursor)).await.expect("third");
    assert!(third.changes.is_empty());
    assert_eq!(served.load(SeqCst), 0);
}

#[tokio::test]
async fn a_stale_cursor_walks_the_folder_again_and_says_so() {
    let stub = start(0, |req| {
        if req.target.contains("$deltatoken=old-example") { Resp::status(410, fixture("gone_410.json")) } else { Resp::ok(fixture("delta_empty.json")) }
    })
    .await;
    let r = rig(&stub.base, 50, 3);
    let stale = SyncCursor::GraphDelta { token: format!("{}/v1.0/me/mailFolders/folder/messages/delta?$deltatoken=old-example", stub.base) };
    let round = r.mailbox.delta_round("folder", "Folder", Some(&stale)).await.expect("round");
    assert!(round.reset);
    assert_eq!(stub.requests().len(), 2);
    assert!(token_of(&round.cursor).contains("empty-example"));
}

#[tokio::test]
async fn a_link_to_another_host_is_refused_before_any_request_to_it() {
    let stub = start(0, |_| Resp::ok(fixture("delta_offsite.json"))).await;
    let r = rig(&stub.base, 50, 3);
    let err = r.mailbox.delta_round("inbox-id", "Inbox", None).await.unwrap_err();
    assert!(matches!(err, CoreError::Invalid { ref field, .. } if field == "link"), "{err:?}");
    assert_eq!(stub.requests().len(), 1);
}

#[tokio::test]
async fn a_cursor_of_another_provider_is_refused() {
    let stub = start(0, |_| Resp::ok(fixture("delta_empty.json"))).await;
    let r = rig(&stub.base, 50, 3);
    let err = r.mailbox.delta_round("f", "F", Some(&SyncCursor::Jmap { state: "s".into() })).await.unwrap_err();
    assert!(matches!(err, CoreError::Invalid { .. }));
    assert!(stub.requests().is_empty());
}

#[tokio::test]
async fn the_folder_tree_includes_children() {
    let stub = start(0, |req| if req.target.contains("/inbox-id/childFolders") { Resp::ok(fixture("folders_children.json")) } else { Resp::ok(fixture("folders_top.json")) }).await;
    let r = rig(&stub.base, 50, 3);
    let mut folders = r.mailbox.list_folders().await.expect("folders");
    folders.sort_by(|a, b| a.id.cmp(&b.id));
    let names: Vec<(&str, &str, Option<&str>)> = folders.iter().map(|f| (f.id.as_str(), f.display_name.as_str(), f.parent_id.as_deref())).collect();
    assert_eq!(names, vec![("inbox-id", "Inbox", Some("root-id")), ("receipts-id", "Receipts", Some("inbox-id")), ("sent-id", "Sent Items", Some("root-id"))]);
}
