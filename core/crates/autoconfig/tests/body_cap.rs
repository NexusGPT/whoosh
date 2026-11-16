//! Provider-limits lane, item 1: an autoconfig or Autodiscover answer is read up to AUTOCONFIG_BODY_MAX_BYTES (256 KiB), never
//! further. The reader takes a real socket from a fake server on 127.0.0.1 (no other host is reached, no TLS needed).
//! Each test is red on a mutant in mutants.sh (names in the comments below).
use autoconfig::adapters::body_cap::{read_body, read_capped};
use autoconfig::consts::AUTOCONFIG_BODY_MAX_BYTES;
use autoconfig::NetError;
use std::io::{Cursor, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// A fake server on 127.0.0.1 that writes `total` bytes of 'a' (None: until the client hangs up) and closes. Returns the client
/// socket and the count of bytes the server managed to write.
fn serve(total: Option<u64>) -> (TcpStream, Arc<AtomicU64>, std::thread::JoinHandle<()>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = l.local_addr().expect("addr");
    let sent = Arc::new(AtomicU64::new(0));
    let sent2 = sent.clone();
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().expect("accept");
        s.set_write_timeout(Some(Duration::from_secs(10))).expect("timeout");
        let chunk = vec![b'a'; 8192];
        loop {
            let want = match total { Some(t) => (t - sent2.load(Ordering::SeqCst)).min(chunk.len() as u64) as usize, None => chunk.len() };
            if want == 0 { break; }
            match s.write(&chunk[..want]) { Ok(n) => { sent2.fetch_add(n as u64, Ordering::SeqCst); } Err(_) => break }
            // an endless server is stopped by the client hanging up; a safety stop far past any sane read keeps a mutant from running away
            if total.is_none() && sent2.load(Ordering::SeqCst) > 64 * 1024 * 1024 { break; }
        }
    });
    (TcpStream::connect(addr).expect("connect"), sent, h)
}

#[test]
fn the_limit_is_the_ledger_value_256_kib() {
    assert_eq!(AUTOCONFIG_BODY_MAX_BYTES, 256 * 1024);
}

#[test]
fn a_body_of_exactly_the_limit_is_read_whole_and_one_byte_over_is_refused_in_words() {
    // mutants (mutants.sh): cap-off-by-one (`> limit` -> `>= limit`: the exact-limit body is refused, red here),
    //          cap-truncates-silently (`take(limit.saturating_add(1))` -> `take(limit)`: the over-limit body comes back cut, red here),
    //          cap-removed (`if out.len() as u64 > limit {` -> `if false {`: the over-limit body is returned, red here),
    //          cap-constant-ignored (read_body's limit x4: the one-byte-over body passes, red here)
    let (client, _, h) = serve(Some(AUTOCONFIG_BODY_MAX_BYTES));
    let body = read_body(client).expect("exactly the limit passes");
    assert_eq!(body.len() as u64, AUTOCONFIG_BODY_MAX_BYTES);
    h.join().expect("server");
    let (client, _, h) = serve(Some(AUTOCONFIG_BODY_MAX_BYTES + 1));
    let Err(NetError::Other(words)) = read_body(client) else { panic!("one byte over the limit must be refused") };
    assert!(words.contains("larger than 256 KiB") && words.contains("Manual setup"), "{words}");
    h.join().expect("server");
}

#[test]
fn a_server_that_never_stops_is_cut_off_near_the_limit_not_read_to_the_end() {
    // mutant (mutants.sh): cap-unbounded (`take(limit.saturating_add(1))` -> `take(u64::MAX)`: reads until the
    // server's own safety stop at 64 MiB, so the server's count passes 16 MiB, red here)
    let (client, sent, h) = serve(None);
    let r = read_body(client);
    assert!(matches!(r, Err(NetError::Other(ref w)) if w.contains("larger than")), "{r:?}");
    h.join().expect("server");
    // the client hung up after limit + 1 bytes: what the server could still push is kernel socket buffering, far below 64 MiB
    assert!(sent.load(Ordering::SeqCst) < 16 * 1024 * 1024, "the server wrote {} bytes", sent.load(Ordering::SeqCst));
}

#[test]
fn the_cap_follows_the_limit_it_is_given_and_a_failing_read_is_an_error_in_words() {
    assert_eq!(read_capped(Cursor::new(vec![1u8; 10]), 10).expect("10 of 10").len(), 10);
    assert!(read_capped(Cursor::new(vec![1u8; 11]), 10).is_err());
    assert_eq!(read_capped(Cursor::new(Vec::<u8>::new()), 10).expect("empty").len(), 0);
    struct Broken;
    impl std::io::Read for Broken { fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> { Err(std::io::Error::other("the socket broke")) } }
    let Err(NetError::Other(w)) = read_capped(Broken, 10) else { panic!("a failing read is an error") };
    assert!(w.contains("could not be read") && w.contains("the socket broke"), "{w}");
}
