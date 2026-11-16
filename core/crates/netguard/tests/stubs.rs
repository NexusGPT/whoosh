//! Shared by the rig-guard tests of every crate (included by `#[path]`): a counting TCP listener on 127.0.0.1, a counting UDP name
//! server that answers NXDOMAIN, and a resolver that counts and answers that listener. With them a guard removed by a mutant reaches this
//! Mac (and is counted), never the network.
#![allow(dead_code)]
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Accepts and drops every connection, counting them.
pub struct Listener { pub port: u16, pub accepted: Arc<AtomicUsize> }
pub fn listener() -> Listener {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = l.local_addr().unwrap().port();
    let accepted = Arc::new(AtomicUsize::new(0));
    let a = accepted.clone();
    std::thread::spawn(move || for s in l.incoming() { if s.is_ok() { a.fetch_add(1, Ordering::SeqCst); } });
    Listener { port, accepted }
}
impl Listener { pub fn count(&self) -> usize { std::thread::sleep(std::time::Duration::from_millis(150)); self.accepted.load(Ordering::SeqCst) } }

/// A name server on 127.0.0.1 that counts each query and answers it NXDOMAIN.
pub struct NameServer { pub addr: SocketAddr, pub asked: Arc<AtomicUsize> }
pub fn name_server() -> NameServer {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind udp");
    let addr = sock.local_addr().unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let a = asked.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1500];
        while let Ok((n, peer)) = sock.recv_from(&mut buf) {
            a.fetch_add(1, Ordering::SeqCst);
            if n >= 12 { buf[2] |= 0x80; buf[3] = (buf[3] & 0xF0) | 3; let _ = sock.send_to(&buf[..n], peer); }
        }
    });
    NameServer { addr, asked }
}
impl NameServer { pub fn count(&self) -> usize { std::thread::sleep(std::time::Duration::from_millis(150)); self.asked.load(Ordering::SeqCst) } }

/// Every name resolves to 127.0.0.1:<port of the listener>; each lookup counted.
pub struct Resolver { pub port: u16, pub asked: Arc<AtomicUsize> }
impl netguard::Resolve for Resolver {
    fn lookup(&self, _host: &str, _port: u16) -> std::io::Result<Vec<SocketAddr>> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(vec![SocketAddr::from(([127, 0, 0, 1], self.port))])
    }
}
pub fn resolver_to(l: &Listener) -> Arc<AtomicUsize> {
    let asked = Arc::new(AtomicUsize::new(0));
    netguard::set_resolver(Arc::new(Resolver { port: l.port, asked: asked.clone() }));
    asked
}
