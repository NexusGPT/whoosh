//! Provider-limits lane, item 1, through the real HTTP client (`--features live`): a local fake server on 127.0.0.1 answers over
//! plain http (`ReqwestHttp::new_plain_loopback`, which refuses every URL but 127.0.0.1). No other host is reached.
//! Mutant (mutants.sh, run with --features live): http-body-uncapped (`body_cap::read_body(resp)?` -> `resp.text().map_err(net_error)?`;
//! the chunked 300 KiB answer comes back whole, red here), http-length-check-dropped (the `content_length()` test made false: only
//! the Content-Length cases still refuse through the reader, so this one stays green by design: the reader is the real guard).
#![cfg(feature = "live")]
use autoconfig::adapters::http_reqwest::ReqwestHttp;
use autoconfig::consts::AUTOCONFIG_BODY_MAX_BYTES;
use autoconfig::{HttpPort, HttpRequest, Method, NetError};
use std::io::{Read, Write};
use std::net::TcpListener;

#[derive(Clone, Copy)]
enum Kind { Length, Chunked }

/// One answer per connection: `total` bytes of 'a', with a Content-Length or chunked. Returns the URL.
fn fake(total: usize, kind: Kind) -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().expect("accept");
        s.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok();
        let mut head = Vec::new();
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") { if s.read(&mut b).unwrap_or(0) == 0 { break; } head.push(b[0]); }
        let body = vec![b'a'; total];
        let _ = match kind {
            Kind::Length => { let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n").as_bytes()); s.write_all(&body) }
            Kind::Chunked => {
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
                let mut r = Ok(());
                for c in body.chunks(8192) { r = s.write_all(format!("{:x}\r\n", c.len()).as_bytes()).and_then(|_| s.write_all(c)).and_then(|_| s.write_all(b"\r\n")); if r.is_err() { break; } }
                r.and_then(|_| s.write_all(b"0\r\n\r\n"))
            }
        };
    });
    format!("http://127.0.0.1:{port}/mail/config-v1.1.xml")
}

fn get(url: String) -> Result<autoconfig::HttpResponse, NetError> {
    ReqwestHttp::new_plain_loopback().expect("client").execute(&HttpRequest { method: Method::Get, url, user_agent: "Whoosh".into(), body: None })
}

#[test]
fn an_answer_over_the_limit_is_refused_in_words_whether_it_names_its_length_or_not() {
    let over = AUTOCONFIG_BODY_MAX_BYTES as usize + 1;
    for kind in [Kind::Length, Kind::Chunked] {
        let Err(NetError::Other(w)) = get(fake(over, kind)) else { panic!("an answer one byte over the limit must be refused") };
        assert!(w.contains("larger than 256 KiB"), "{w}");
        let Err(NetError::Other(w)) = get(fake(300 * 1024, kind)) else { panic!("a 300 KiB answer must be refused") };
        assert!(w.contains("larger than 256 KiB"), "{w}");
    }
}

#[test]
fn an_answer_up_to_the_limit_and_a_normal_one_come_back_whole() {
    for kind in [Kind::Length, Kind::Chunked] {
        let r = get(fake(AUTOCONFIG_BODY_MAX_BYTES as usize, kind)).expect("exactly the limit");
        assert_eq!((r.status, r.body.len() as u64), (200, AUTOCONFIG_BODY_MAX_BYTES));
        let r = get(fake(1500, kind)).expect("a normal answer");
        assert_eq!((r.status, r.body.len()), (200, 1500));
    }
}

#[test]
fn the_plain_loopback_client_refuses_any_other_host() {
    assert_eq!(get("http://example.org/x".into()).err(), Some(NetError::Refused));
    assert_eq!(get("https://127.0.0.1.example.org/x".into()).err(), Some(NetError::Refused));
}
