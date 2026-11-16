//! EventSource push delivers a new message to the core, and a dropped stream is caught up.
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use contracts::SyncCursor;
use sync_jmap::{Delivery, Discovery, FetchOptions, JmapAdapter, LinkStatus, JmapMailAdapter, PushConfig, WatchEnd};
use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

const OPTS: FetchOptions = FetchOptions { preview_chars: 40 };
const WAIT: Duration = Duration::from_secs(10);

fn cfg() -> PushConfig {
    PushConfig { ping_seconds: 1, reconnect_delay: Duration::from_millis(50), opts: OPTS }
}

async fn connect(fake: &Fake) -> JmapAdapter {
    match JmapAdapter::connect(fake.config()).await.unwrap() {
        Discovery::Jmap(a) => Arc::try_unwrap(Arc::new(a)).ok().unwrap(),
        Discovery::FallbackToImap { reason_words } => panic!("{reason_words}"),
    }
}

async fn next_status(rx: &mut mpsc::UnboundedReceiver<LinkStatus>, want: LinkStatus) {
    loop {
        let s = timeout(WAIT, rx.recv()).await.expect("status in time").expect("channel open");
        if s == want {
            return;
        }
    }
}

async fn next_with(rx: &mut mpsc::UnboundedReceiver<Delivery>, id: &str) -> Delivery {
    loop {
        let d = timeout(WAIT, rx.recv()).await.expect("a delivery in time").expect("channel open");
        if d.changed.iter().any(|m| m.server_id == id) {
            return d;
        }
    }
}

#[tokio::test]
async fn a_new_message_pushed_by_the_server_reaches_the_core() {
    let fake = Arc::new(Fake::start(Mode::Jmap, (1..=3).map(FEmail::inbox).collect(), 500).await);
    let a = Arc::new(connect(&fake).await);
    let cursor = a.first_screen("m-inbox", 3, OPTS).await.unwrap().cursor.unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let (status_tx, mut status_rx) = mpsc::unbounded_channel();
    let (stop_tx, stop_rx) = watch::channel(false);
    let adapter = a.clone();
    let watcher = tokio::spawn(async move {
        let mut sink = RecSink::new(None);
        sink.tx = Some(tx);
        sink.status_tx = Some(status_tx);
        adapter.watch(cursor, &mut sink, stop_rx, cfg()).await
    });
    next_status(&mut status_rx, LinkStatus::Connected).await;

    let mut fresh = FEmail::inbox(9);
    fresh.subject = "Supplier quote arrived".into();
    fake.add_email(fresh);
    fake.push_state_change();

    let d = next_with(&mut rx, "e09").await;
    let m = d.changed.iter().find(|m| m.server_id == "e09").unwrap();
    assert_eq!(m.message.subject, "Supplier quote arrived");
    assert_eq!(d.cursor, Some(SyncCursor::Jmap { state: "s4".into() }), "the cursor travels with the message");

    stop_tx.send(true).unwrap();
    assert_eq!(timeout(WAIT, watcher).await.unwrap().unwrap().unwrap(), WatchEnd::Stopped);
    fake.shutdown().await;
}

#[tokio::test]
async fn mail_that_arrived_while_the_stream_was_down_is_caught_up_on_reconnect() {
    let fake = Arc::new(Fake::start(Mode::Jmap, (1..=3).map(FEmail::inbox).collect(), 500).await);
    let a = Arc::new(connect(&fake).await);
    let cursor = a.first_screen("m-inbox", 3, OPTS).await.unwrap().cursor.unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let (status_tx, mut status_rx) = mpsc::unbounded_channel();
    let (stop_tx, stop_rx) = watch::channel(false);
    let adapter = a.clone();
    let watcher = tokio::spawn(async move {
        let mut sink = RecSink::new(None);
        sink.tx = Some(tx);
        sink.status_tx = Some(status_tx);
        adapter.watch(cursor, &mut sink, stop_rx, cfg()).await
    });
    next_status(&mut status_rx, LinkStatus::Connected).await;

    // The message lands while the stream is cut; no event ever announces it.
    fake.add_email(FEmail::inbox(7));
    fake.drop_streams();

    let d = next_with(&mut rx, "e07").await;
    assert_eq!(d.cursor, Some(SyncCursor::Jmap { state: "s4".into() }));
    next_status(&mut status_rx, LinkStatus::Connected).await;

    stop_tx.send(true).unwrap();
    assert_eq!(timeout(WAIT, watcher).await.unwrap().unwrap().unwrap(), WatchEnd::Stopped);
    let port = fake.shutdown().await;
    assert!(tokio::net::TcpStream::connect(port).await.is_err(), "the test server released its port");
}

#[tokio::test]
async fn a_cursor_of_another_provider_ends_the_watch_before_any_network_call() {
    let fake = Arc::new(Fake::start(Mode::Jmap, vec![FEmail::inbox(1)], 500).await);
    let a = Arc::new(connect(&fake).await);
    let (stop_tx, stop_rx) = watch::channel(false);
    let mut sink = RecSink::new(None);
    // A cursor of another account type is refused before any network call.
    let out = a.watch(SyncCursor::GmailHistory { history_id: "1".into() }, &mut sink, stop_rx, cfg()).await;
    drop(stop_tx);
    assert!(out.is_err(), "{out:?}");
    fake.shutdown().await;
}
