//! rig-net-rust (audit r77, gap 6): the real transport (`live`) of a rig copy reaches this Mac only. Every name the client would look
//! up goes to a counting resolver that answers a listener on 127.0.0.1, so a guard removed by a mutant is counted there, never sent away.
//! mutant "ai-send-unguarded": live.rs `send`, the `netguard::outbound_allowed_url(&request.url)...?;` line deleted (red: the resolver
//! is asked, the listener counts one).
//! mutant "ai-redirect-unguarded": live.rs `guarded_redirects`, the guard's arm `Err(e) => a.error(e.to_string())` made `Err(_) => a.follow()` (red: the redirect to
//! the away host is followed, the resolver is asked).
#![cfg(feature = "live")]
#[path = "../../netguard/tests/stubs.rs"]
mod stubs;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::Arc;
use std::time::Duration;

use ai_providers::live::ReqwestTransport;
use ai_providers::transport::{Body, Method, Request, Transport};

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

fn request(url: String) -> Request { Request { method: Method::Post, url, headers: Vec::new(), body: Body::Json("{}".into()) } }

#[test]
fn a_rig_transport_reaches_this_mac_only() {
    let l = stubs::listener();
    let asked = stubs::resolver_to(&l);
    let (port, hits) = redirect_server("http://provider.away.example/next");
    netguard::close_to_this_mac();
    let t = ReqwestTransport::new(Duration::from_secs(5)).expect("transport");
    for away in ["http://provider.away.example/v1/x", "https://provider.away.example\\@127.0.0.1/v1/x"] {
        let e = t.send(&request(away.into())).expect_err(away);
        assert!(e.contains("not this Mac"), "{away}: {e}");
    }
    assert!(t.send(&request(format!("http://127.0.0.1:{port}/start"))).is_err(), "the redirect away is not followed");
    assert_eq!(hits.load(SeqCst), 1, "this Mac's own server was asked once");
    assert_eq!(asked.load(SeqCst), 0, "no name looked up");
    assert_eq!(l.count(), 0, "no socket opened to the away host");
}
