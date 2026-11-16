//! rig-net-rust (audit r77, gap 6): a Graph mailbox of a rig copy reaches this Mac only: refused at the constructor, at each dial and at
//! each redirect. Every name the client would look up goes to a counting resolver that answers a listener on 127.0.0.1, so a guard
//! removed by a mutant is counted there, never sent to Microsoft.
//! mutant "graph-new-unguarded": client.rs `GraphMailbox::new`, the `netguard::outbound_allowed_url(&cfg.base_url)?;` line deleted (red:
//! the mailbox is built).
//! mutant "graph-dial-unguarded": client.rs `attempt_once`, the `netguard::outbound_allowed_url(url)?;` line deleted (red: the resolver
//! is asked, the listener counts one).
//! mutant "graph-redirect-unguarded": client.rs `guarded_redirects`, the guard's arm `Err(e) => a.error(e.to_string())` made `Err(_) => a.follow()` (red: the redirect
//! to the away host is followed, the resolver is asked).
mod support;
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;

use std::sync::atomic::{AtomicI64, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use contracts::CoreError;
use support::{FakeClock, RecSleeper, Resp};
use sync_graph::{BoxFut, GraphConfig, GraphMailbox, Throttle, TokenSource};

struct Token;
impl TokenSource for Token {
    fn access_token(&self) -> BoxFut<'_, Result<String, CoreError>> { Box::pin(async { Ok("test-token-example".to_string()) }) }
}

fn mailbox(base_url: &str) -> Result<GraphMailbox, CoreError> {
    let clock = Arc::new(FakeClock(AtomicI64::new(1_793_000_000_000)));
    let sleeper = Arc::new(RecSleeper { clock: clock.clone(), slept: Mutex::new(Vec::new()) });
    let cfg = GraphConfig { base_url: base_url.into(), mailbox_path: "me".into(), account_id: "acct-rig".into(), account_name: "Rig Example".into(),
        max_attempts: 1, backoff_base_ms: 1, backoff_max_ms: 1, timeout_ms: 5_000, page_size: 10 };
    GraphMailbox::new(cfg, Arc::new(Token), Arc::new(Throttle::new(clock, sleeper)), None)
}

fn refused<T>(r: &Result<T, CoreError>) -> bool {
    matches!(r, Err(CoreError::Action { failure }) if failure.code.as_deref() == Some(netguard::OUTBOUND_REFUSED_CODE))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rig_mailbox_reaches_this_mac_only() {
    let l = stubs::listener();
    let asked = stubs::resolver_to(&l);
    let stub = support::start(0, |_| Resp::status(302, String::new()).header("location", "http://graph.away.example/v1.0/me/mailFolders")).await;
    // built while this process is the owner's copy; then the rig closes it
    let early = mailbox("http://graph.away.example/v1.0").expect("open: any host");
    let near = mailbox(&format!("{}/v1.0", stub.base)).expect("this Mac");
    netguard::close_to_this_mac();

    let built = mailbox("https://graph.microsoft.com/v1.0");
    assert!(refused(&built), "constructor: {:?}", built.as_ref().err());
    let dial = tokio::time::timeout(std::time::Duration::from_secs(10), early.list_folders()).await.expect("bounded");
    assert!(refused(&dial), "dial: {dial:?}");
    let redirect = tokio::time::timeout(std::time::Duration::from_secs(10), near.list_folders()).await.expect("bounded");
    assert!(redirect.is_err(), "the redirect away is not followed: {redirect:?}");
    assert_eq!(stub.requests().len(), 1, "this Mac's own server was asked once");
    assert_eq!(asked.load(SeqCst), 0, "no name looked up");
    assert_eq!(l.count(), 0, "no socket opened to the away host");
}
