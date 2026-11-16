//! imap-compress: COMPRESS=DEFLATE (RFC 4978). Three rules the adapter relies on:
//!   1. The client may send `COMPRESS DEFLATE` only when the server lists COMPRESS=DEFLATE, and only once per connection.
//!   2. The tagged OK of the command is the last uncompressed server byte; from the next byte each way is raw DEFLATE (no zlib header)
//!      with a sync flush at the end of each message.
//!   3. A NO or BAD leaves the connection uncompressed (the server's side) and the client carries on without compression.
//! Everything runs against the in-process fake (tests/support), which inflates what it reads and deflates what it writes once it said
//! OK, and keeps the raw bytes it read so a test can see what travelled. Every wait has a time limit: a fake that never answers turns
//! a test red in seconds.
mod support;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use support::*;
use sync_imap::*;
use tokio::sync::Notify;

/// The longest any step of these tests may wait.
const ANSWER_WITHIN: Duration = Duration::from_secs(10);

async fn bounded<T>(what: &str, f: impl Future<Output = T>) -> T {
    match tokio::time::timeout(ANSWER_WITHIN, f).await {
        Ok(v) => v,
        Err(_) => panic!("{what} did not answer within {ANSWER_WITHIN:?}: the connection is still waiting on the server (a stream switched too early or too late)"),
    }
}

async fn server(answer: Option<CompressAnswer>) -> FakeImap {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 5);
    if let Some(a) = answer { srv.with(|s| { s.cap_compress = true; s.compress_answer = a; }); }
    srv
}

fn count(log: &[String], what: &str) -> usize { log.iter().filter(|l| l.contains(what)).count() }

/// The raw bytes the server read after its first `COMPRESS DEFLATE` line.
fn raw_after_compress(srv: &FakeImap) -> Vec<u8> {
    let raw = srv.with(|s| s.raw_in.clone());
    let at = raw.windows(18).position(|w| w == b"COMPRESS DEFLATE\r\n").expect("the server read a COMPRESS DEFLATE line") + 18;
    raw[at..].to_vec()
}

fn has(hay: &[u8], needle: &[u8]) -> bool { hay.windows(needle.len()).any(|w| w == needle) }

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_lists_compress_and_says_ok_gets_the_next_commands_deflated_and_its_answers_are_read() {
    let srv = server(Some(CompressAnswer::Ok)).await;
    let a = adapter(&srv, "z");
    let folders = bounded("LIST over a compressed connection", a.list_folders()).await.expect("LIST over deflate");
    assert!(folders.iter().any(|f| f.name == "INBOX"), "the compressed LIST answer was read: {folders:?}");
    let page = bounded("SELECT and FETCH over a compressed connection", a.history_page("INBOX", u32::MAX, 10, OPTS)).await.expect("FETCH over deflate");
    assert_eq!(page.messages.len(), 5, "five messages came back through the inflater");

    let log = srv.log();
    assert_eq!(count(&log, "COMPRESS DEFLATE"), 1, "asked once: {log:?}");
    let pos = |what: &str| log.iter().position(|l| l.contains(what)).unwrap_or_else(|| panic!("no {what} in {log:?}"));
    assert!(pos("LOGIN") < pos("COMPRESS DEFLATE") && pos("COMPRESS DEFLATE") < pos("LIST"), "after the sign-in, before the first mailbox command: {log:?}");
    assert_eq!(srv.with(|s| s.compressed_conns), 1);
    let tail = raw_after_compress(&srv);
    assert!(!tail.is_empty(), "the client sent bytes after the OK");
    for word in [&b"LIST"[..], b"SELECT", b"UID FETCH", b"ENABLE"] {
        assert!(!has(&tail, word), "the command text {:?} travelled in clear after the OK", String::from_utf8_lossy(word));
    }
    assert_eq!(a.connections_compressed(), vec![true], "the fact kept for the connection");
    let t = a.transcript().text();
    assert!(t.contains("COMPRESS DEFLATE") && t.contains("OK DEFLATE active") && t.lines().any(|l| l.starts_with("C: ") && l.ends_with("LIST \"\" \"*\"")) && t.lines().any(|l| l.starts_with("S: * LIST")), "the transcript keeps the plain lines on both sides of the switch:\n{t}");
    assert!(t.is_ascii(), "the transcript holds protocol text, never compressed bytes");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_compress_no_or_bad_leaves_a_working_clear_connection_and_is_not_asked_again() {
    for answer in [CompressAnswer::No, CompressAnswer::Bad] {
        let srv = server(Some(answer)).await;
        let a = adapter(&srv, "z");
        let folders = bounded("LIST after a refused COMPRESS", a.list_folders()).await.unwrap_or_else(|e| panic!("a {answer:?} to COMPRESS must not end the connection: {e:?}"));
        assert!(folders.iter().any(|f| f.name == "INBOX"), "{folders:?}");
        assert_eq!(bounded("a second call", a.history_page("INBOX", u32::MAX, 10, OPTS)).await.expect("the clear connection works").messages.len(), 5);
        let log = srv.log();
        assert_eq!(count(&log, "COMPRESS DEFLATE"), 1, "{answer:?}: asked once, never again on the next connection or call: {log:?}");
        assert_eq!(srv.with(|s| s.compressed_conns), 0);
        let raw = srv.with(|s| s.raw_in.clone());
        assert!(has(&raw, b"LIST \"\" \"*\"\r\n") || has(&raw, b"LIST"), "{answer:?}: the LIST arrived in clear");
        assert!(has(&raw, b"SELECT") && has(&raw, b"FETCH"), "{answer:?}: the later commands arrived in clear");
        assert_eq!(a.connections_compressed(), vec![false], "{answer:?}: the connection that carries on is recorded as clear");
        assert!(a.transcript().text().contains("NO") || a.transcript().text().contains("BAD"), "the refusal is in the transcript");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_does_not_list_compress_never_gets_a_compress_command() {
    let srv = server(None).await;
    let a = adapter(&srv, "z");
    bounded("LIST on a server without COMPRESS", a.list_folders()).await.expect("LIST");
    bounded("FETCH on a server without COMPRESS", a.history_page("INBOX", u32::MAX, 10, OPTS)).await.expect("FETCH");
    let log = srv.log();
    assert_eq!(count(&log, "COMPRESS"), 0, "COMPRESS was sent to a server that does not list it: {log:?}");
    assert_eq!(a.connections_compressed(), vec![false]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_xoauth2_sign_in_is_compressed_too() {
    let srv = server(Some(CompressAnswer::Ok)).await;
    let a = Arc::new(ImapAdapter::new(config(&srv, "z", Auth::XOAuth2(Arc::new(FixedTok("token-1"))))));
    bounded("LIST after XOAUTH2", a.list_folders()).await.expect("LIST");
    let log = srv.log();
    let pos = |what: &str| log.iter().position(|l| l.contains(what)).unwrap_or_else(|| panic!("no {what} in {log:?}"));
    assert!(pos("AUTHENTICATE") < pos("COMPRESS DEFLATE") && pos("COMPRESS DEFLATE") < pos("LIST"), "{log:?}");
    assert_eq!(a.connections_compressed(), vec![true]);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_idle_connection_is_compressed_and_hears_new_mail_through_the_inflater() {
    let srv = server(Some(CompressAnswer::Ok)).await;
    let a = adapter(&srv, "z");
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let events: Arc<Mutex<Vec<IdleEvent>>> = Arc::new(Mutex::new(Vec::new()));
    let h = tokio::spawn({
        let (a, clock, stop, events) = (a.clone(), clock.clone(), stop.clone(), events.clone());
        async move { a.idle("INBOX", &*clock, &stop, &mut move |e| events.lock().unwrap().push(e)).await }
    });
    bounded("the server seeing IDLE", srv.wait_for("IDLE", |s| s.idle_starts == 1)).await;
    assert_eq!(a.connections_compressed(), vec![true], "the IDLE connection took the same step");
    srv.add_message("INBOX");
    bounded("the change reaching the client", async {
        while !events.lock().unwrap().contains(&IdleEvent::Changed) { tokio::time::sleep(Duration::from_millis(10)).await; }
    }).await;
    stop.notify_one();
    bounded("IDLE stopping", h).await.expect("join").expect("IDLE ends cleanly over deflate");
    assert!(!has(&raw_after_compress(&srv), b"IDLE"), "IDLE travelled deflated");
}

/// The measurement for the lead (not run by `cargo test`): one initial sync of a 2,000-message folder with compression and one
/// without, against the loopback fake, printing both times and the bytes the fake read from the client.
///   cd core && cargo test -p sync-imap --offline --test compress -- --ignored --nocapture sync_2000
/// Loopback has no bandwidth limit, so this shows the CPU the deflate costs; a link's gain needs a real slow link.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "timed measurement, run by hand"]
async fn sync_2000_messages_with_and_without_compression() {
    for (label, answer) in [("clear", None), ("deflate", Some(CompressAnswer::Ok))] {
        let srv = FakeImap::start().await;
        srv.seed("INBOX", 2000);
        if let Some(a) = answer { srv.with(|s| { s.cap_compress = true; s.compress_answer = a; }); }
        let a = adapter(&srv, "z");
        let mut sink = Sink::default();
        let t0 = std::time::Instant::now();
        let _snap = bounded("the 2,000-message sync", a.initial_sync("INBOX", 200, 500, OPTS, &mut sink)).await.expect("sync");
        let took = t0.elapsed();
        println!("{label}: {} messages in {took:?}; compressed connections {:?}", sink.msgs.len(), a.connections_compressed());
    }
}
