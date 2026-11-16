//! Round 4 (engine-sync-m365): a server that ends a connection with `* BYE` (Microsoft 365 answered "Connection closed. 14")
//! or refuses a command with BAD. Each answer was once read as an empty or short SUCCESS, so the folder was marked fully
//! fetched with nothing stored. Here each is an error: a lost connection is retryable and a refusal carries the server's
//! words. Everything runs against the in-process fake (tests/support), which plays the fault on a chosen command.
mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::CoreError;
use support::*;
use sync_imap::*;
use tokio::sync::Notify;

const BYE: &str = "Connection closed. 14";

fn failure(r: Result<impl std::fmt::Debug, CoreError>) -> contracts::ActionFailed {
    match r {
        Err(CoreError::Action { failure }) => failure,
        other => panic!("expected an Action failure, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bye_in_the_middle_of_a_fetch_is_a_lost_connection_never_a_short_page() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 10);
    let a = adapter(&srv, "m365");
    srv.fault(Fault::bye("UID FETCH", BYE).after(3));
    let f = failure(a.history_page("INBOX", u32::MAX, 10, OPTS).await);
    assert!(f.retryable, "a closed connection is retried: {f:?}");
    assert!(f.reason_words.contains(BYE), "the server's words are kept: {}", f.reason_words);
    assert_eq!(srv.faults_played(), 1);
    // the next call opens a new connection and gets the whole page
    let page = a.history_page("INBOX", u32::MAX, 10, OPTS).await.expect("the retry");
    assert_eq!(page.messages.len(), 10);
    assert!(page.done);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bye_on_the_search_is_retryable_never_an_empty_finished_folder() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 4);
    let a = adapter(&srv, "m365");
    srv.fault(Fault::bye("UID SEARCH", BYE));
    let f = failure(a.history_page("INBOX", u32::MAX, 10, OPTS).await);
    assert!(f.retryable && f.reason_words.contains(BYE), "{f:?}");
    assert_eq!(a.history_page("INBOX", u32::MAX, 10, OPTS).await.unwrap().messages.len(), 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_answer_to_the_search_is_the_servers_refusal_in_its_words() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 4);
    let a = adapter(&srv, "m365");
    srv.fault(Fault::bad("UID SEARCH", "Command Argument Error. 11"));
    let f = failure(a.history_page("INBOX", u32::MAX, 10, OPTS).await);
    assert!(!f.retryable, "a refusal is not retried blindly: {f:?}");
    assert!(f.reason_words.contains("Command Argument Error. 11"), "{}", f.reason_words);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_newest_page_asks_for_one_to_star_and_never_the_largest_uid() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 3);
    let a = adapter(&srv, "m365");
    srv.clear_log();
    a.history_page("INBOX", u32::MAX, 10, OPTS).await.unwrap();
    let log = srv.log();
    assert!(log.iter().any(|l| l.ends_with("UID SEARCH UID 1:*")), "{log:?}");
    assert!(!log.iter().any(|l| l.contains("4294967294")), "{log:?}");
    // an older page still names its bound
    srv.clear_log();
    a.history_page("INBOX", 3, 10, OPTS).await.unwrap();
    assert!(srv.log().iter().any(|l| l.ends_with("UID SEARCH UID 1:2")), "{:?}", srv.log());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bye_on_the_select_or_the_list_is_retryable() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 2);
    let a = adapter(&srv, "m365");
    srv.fault(Fault::bye("SELECT", BYE));
    let f = failure(a.folder_state("INBOX").await);
    assert!(f.retryable && f.reason_words.contains(BYE), "{f:?}");
    assert_eq!(a.folder_state("INBOX").await.unwrap().exists, 2);
    srv.fault(Fault::bye("LIST", BYE));
    let f = failure(a.list_folders().await);
    assert!(f.retryable && f.reason_words.contains(BYE), "the folder list is never read as empty: {f:?}");
    assert!(!a.list_folders().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bye_while_idling_reconnects_after_the_backoff_and_says_why() {
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 1);
    let a = adapter(&srv, "m365");
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let events: Arc<Mutex<Vec<IdleEvent>>> = Arc::new(Mutex::new(Vec::new()));
    srv.fault(Fault::bye("IDLE", BYE));
    let h = tokio::spawn({
        let (a, clock, stop, events) = (a.clone(), clock.clone(), stop.clone(), events.clone());
        async move { a.idle_supervised("INBOX", &*clock, &stop, &mut move |e| events.lock().unwrap().push(e)).await }
    });
    let lost = || events.lock().unwrap().iter().find_map(|e| match e { IdleEvent::Lost { reason_words, retry_in_ms } => Some((reason_words.clone(), *retry_in_ms)), _ => None });
    for _ in 0..500 { if lost().is_some() { break } tokio::time::sleep(Duration::from_millis(10)).await; }
    let (words, wait) = lost().expect("a Lost event");
    assert!(words.contains(BYE), "the BYE's words, not a bare 'connection lost': {words}");
    assert_eq!(wait, a.connection_table().reconnect_delay(0).as_millis() as u64);
    assert!(!events.lock().unwrap().contains(&IdleEvent::Changed), "a BYE is not new mail: {:?}", events.lock().unwrap());
    for _ in 0..500 { if clock.sleepers() == 1 { break } tokio::time::sleep(Duration::from_millis(10)).await; }
    clock.advance(Duration::from_millis(wait));
    srv.wait_for("the IDLE on the new connection", |s| s.idle_starts == 1).await;
    for _ in 0..500 { if events.lock().unwrap().contains(&IdleEvent::Listening) { break } tokio::time::sleep(Duration::from_millis(10)).await; }
    assert!(events.lock().unwrap().contains(&IdleEvent::Listening), "{:?}", events.lock().unwrap());
    stop.notify_one();
    h.await.unwrap().expect("stops cleanly");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bye_on_the_select_before_idle_reconnects_instead_of_ending_push() {
    // the owner's case: "New mail is not pushed: io: status: Bye ... Connection closed. 14" and push never came back
    let srv = FakeImap::start().await;
    srv.seed("INBOX", 1);
    let a = adapter(&srv, "m365");
    let clock = Arc::new(ManualClock::new());
    let stop = Arc::new(Notify::new());
    let events: Arc<Mutex<Vec<IdleEvent>>> = Arc::new(Mutex::new(Vec::new()));
    srv.fault(Fault::bye("SELECT", BYE));
    let h = tokio::spawn({
        let (a, clock, stop, events) = (a.clone(), clock.clone(), stop.clone(), events.clone());
        async move { a.idle_supervised("INBOX", &*clock, &stop, &mut move |e| events.lock().unwrap().push(e)).await }
    });
    for _ in 0..500 {
        if h.is_finished() || events.lock().unwrap().iter().any(|e| matches!(e, IdleEvent::Lost { .. })) { break }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(!h.is_finished(), "the supervisor gave up on a BYE");
    for _ in 0..500 { if clock.sleepers() == 1 { break } tokio::time::sleep(Duration::from_millis(10)).await; }
    clock.advance(a.connection_table().reconnect_delay(0));
    srv.wait_for("IDLE after the reconnect", |s| s.idle_starts == 1).await;
    stop.notify_one();
    h.await.unwrap().expect("stops cleanly");
}
