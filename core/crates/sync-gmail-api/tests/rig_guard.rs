//! rig-net-rust (audit r77, gap 6): the real Gmail transport (`live`) of a rig copy reaches this Mac only. The name it would look up goes
//! to a counting resolver that answers a listener on 127.0.0.1, so a guard removed by a mutant is counted there, never sent to Google.
//! mutant "gmail-send-unguarded": live.rs `send`, the `netguard::outbound_allowed_url(&url)...?;` line deleted (red: the resolver is
//! asked, the listener counts one).
//! mutant "gmail-redirect-unguarded" (fix-core, item c02dad3b): live.rs `guarded_redirects`, the guard's arm `Err(e) => a.error(e.to_string()),` made
//! `Err(_) => a.follow(),` (red: the redirect to the away host is followed, the resolver is asked). The transport's `with_base` seam points it at a
//! redirect server on 127.0.0.1, so no TLS server is needed. The same policy is proven in ai-providers and sync-graph (docs/lanes/rig-net-rust.md).
#![cfg(feature = "live")]
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sync_gmail_api::live::ReqwestTransport;
use sync_gmail_api::transport::{Method, Request, Transport};

/// The two tests set the process's one resolver and count against it: one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A server on 127.0.0.1 that answers every request with a 302 to `to`, counting them. Each read is bounded.
fn redirect_server(to: &'static str) -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = l.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || for s in l.incoming() {
        let Ok(mut s) = s else { continue };
        h.fetch_add(1, SeqCst);
        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 4096];
        let _ = s.read(&mut buf);
        let _ = s.write_all(format!("HTTP/1.1 302 Found\r\nlocation: {to}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").as_bytes());
    });
    (port, hits)
}

#[test]
fn a_rig_gmail_transport_does_not_follow_a_redirect_away() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let l = stubs::listener();
    let asked = stubs::resolver_to(&l);
    let (port, hits) = redirect_server("http://gmail.away.example/x");
    netguard::close_to_this_mac();
    let t = ReqwestTransport::with_base(Duration::from_secs(5), format!("http://127.0.0.1:{port}/gmail/v1/users/me/")).expect("transport");
    let r = Request { method: Method::Get, path: "labels".into(), query: Vec::new(), bearer: "test-token-example".into(), json_body: None };
    assert!(t.send(&r).is_err(), "the redirect away is not followed");
    assert_eq!(hits.load(SeqCst), 1, "this Mac's own server was asked once");
    assert_eq!(asked.load(SeqCst), 0, "no name looked up");
    assert_eq!(l.count(), 0, "no socket opened to the away host");
}

#[test]
fn a_rig_gmail_transport_reaches_this_mac_only() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let l = stubs::listener();
    let asked = stubs::resolver_to(&l);
    netguard::close_to_this_mac();
    let t = ReqwestTransport::new(Duration::from_secs(5)).expect("transport");
    let r = Request { method: Method::Get, path: "labels".into(), query: Vec::new(), bearer: "test-token-example".into(), json_body: None };
    let e = t.send(&r).expect_err("refused on a rig");
    assert!(e.contains("not this Mac") && e.contains("gmail.googleapis.com"), "{e}");
    assert_eq!(asked.load(SeqCst), 0, "no name looked up");
    assert_eq!(l.count(), 0, "no socket opened");
}
