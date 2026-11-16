//! The guess step's probe: TCP connect, TLS handshake, read the greeting. STARTTLS on the plain ports.
//! The TLS set-up is the one in sync-imap's transport.rs (native roots, the ring provider, safe defaults);
//! that function is crate-private, so the same crates are used here. Synchronous: the probe runs on the
//! detection thread, bounded by PROBE_TIMEOUT_MS per read, write and connect.
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use crate::consts::PROBE_TIMEOUT_MS;
use crate::domain::config::Security;
use crate::ports::{NetError, ProbeOk, ProbePort, ProbeRequest, Protocol};

const MAX_LINE: usize = 4096;

pub struct TlsProbe {
    config: Arc<rustls::ClientConfig>,
    timeout: Duration,
}

impl TlsProbe {
    pub fn new() -> Result<Self, String> {
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            roots.add(cert).map_err(|e| e.to_string())?;
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(Self { config: Arc::new(config), timeout: Duration::from_millis(PROBE_TIMEOUT_MS) })
    }

    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, NetError> {
        netguard::outbound_allowed(host).map_err(|_| NetError::Refused)?;   // engine-act-r6 round 5: before the name is resolved
        let addr = netguard::lookup(host, port).map_err(|_| NetError::HostNotFound)?.into_iter().next().ok_or(NetError::HostNotFound)?;
        let tcp = TcpStream::connect_timeout(&addr, self.timeout).map_err(io_error)?;
        tcp.set_read_timeout(Some(self.timeout)).map_err(io_error)?;
        tcp.set_write_timeout(Some(self.timeout)).map_err(io_error)?;
        Ok(tcp)
    }

    fn tls(&self, host: &str, tcp: TcpStream) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, NetError> {
        let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| NetError::Tls(e.to_string()))?;
        let conn = rustls::ClientConnection::new(self.config.clone(), name).map_err(|e| NetError::Tls(e.to_string()))?;
        let mut stream = rustls::StreamOwned::new(conn, tcp);
        while stream.conn.is_handshaking() {
            stream.conn.complete_io(&mut stream.sock).map_err(|e| NetError::Tls(e.to_string()))?;
        }
        Ok(stream)
    }
}

impl ProbePort for TlsProbe {
    fn execute(&self, req: &ProbeRequest) -> Result<ProbeOk, NetError> {
        let tcp = self.connect(&req.host, req.port)?;
        match req.security {
            Security::Ssl => {
                let mut tls = self.tls(&req.host, tcp)?;
                let greeting = read_greeting(&mut tls, req.protocol)?;
                Ok(ProbeOk { greeting })
            }
            Security::Starttls => {
                let mut tcp = tcp;
                let greeting = read_greeting(&mut tcp, req.protocol)?;
                negotiate_starttls(&mut tcp, req.protocol)?;
                self.tls(&req.host, tcp)?;
                Ok(ProbeOk { greeting })
            }
        }
    }
}

fn io_error(e: io::Error) -> NetError {
    match e.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => NetError::Timeout,
        io::ErrorKind::ConnectionRefused => NetError::Refused,
        io::ErrorKind::NetworkUnreachable | io::ErrorKind::HostUnreachable => NetError::Offline,
        _ => NetError::Other(e.to_string()),
    }
}

/// Reads one CRLF line byte by byte, so nothing after it (the TLS handshake) is consumed.
fn read_line(s: &mut impl Read) -> Result<String, NetError> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while line.len() < MAX_LINE {
        if s.read(&mut byte).map_err(io_error)? == 0 {
            return Err(NetError::Other("the server closed the connection".into()));
        }
        if byte[0] == b'\n' {
            return Ok(String::from_utf8_lossy(&line).trim_end_matches('\r').to_string());
        }
        line.push(byte[0]);
    }
    Err(NetError::Other("a server line is longer than 4096 bytes".into()))
}

/// SMTP replies may span lines ("250-…" … "250 …"); returns the last line.
fn read_reply(s: &mut impl Read, protocol: Protocol) -> Result<String, NetError> {
    let mut line = read_line(s)?;
    if protocol == Protocol::Smtp {
        while line.as_bytes().get(3) == Some(&b'-') { line = read_line(s)?; }
    }
    Ok(line)
}

pub(crate) fn read_greeting(s: &mut impl Read, protocol: Protocol) -> Result<String, NetError> {
    let line = read_reply(s, protocol)?;
    let ok = match protocol {
        Protocol::Imap => line.starts_with("* OK") || line.starts_with("* PREAUTH"),
        Protocol::Smtp => line.starts_with("220"),
    };
    if ok { Ok(line) } else { Err(NetError::Other(format!("not a mail server greeting: {line}"))) }
}

pub(crate) fn negotiate_starttls(s: &mut (impl Read + Write), protocol: Protocol) -> Result<(), NetError> {
    match protocol {
        Protocol::Imap => {
            s.write_all(b"a1 STARTTLS\r\n").map_err(io_error)?;
            loop {
                let line = read_line(s)?;
                if let Some(rest) = line.strip_prefix("a1 ") {
                    return if rest.starts_with("OK") { Ok(()) } else { Err(NetError::Other(format!("STARTTLS refused: {line}"))) };
                }
            }
        }
        Protocol::Smtp => {
            s.write_all(b"EHLO whoosh.app\r\n").map_err(io_error)?;
            let ehlo = read_reply(s, protocol)?;
            if !ehlo.starts_with("250") { return Err(NetError::Other(format!("EHLO refused: {ehlo}"))); }
            s.write_all(b"STARTTLS\r\n").map_err(io_error)?;
            let line = read_reply(s, protocol)?;
            if line.starts_with("220") { Ok(()) } else { Err(NetError::Other(format!("STARTTLS refused: {line}"))) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;

    /// A loopback server speaking `script`: (line to send, expected line to read before the next send).
    fn serve(script: Vec<(&'static str, Option<&'static str>)>) -> (u16, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().expect("addr").port();
        let h = thread::spawn(move || {
            let (sock, _) = listener.accept().expect("accept");
            let mut w = sock.try_clone().expect("clone");
            let mut r = BufReader::new(sock);
            let mut heard = Vec::new();
            for (send, expect) in script {
                if !send.is_empty() { w.write_all(send.as_bytes()).expect("write"); }
                if expect.is_some() {
                    let mut l = String::new();
                    r.read_line(&mut l).expect("read");
                    heard.push(l.trim_end().to_string());
                }
            }
            heard
        });
        (port, h)
    }

    #[test]
    fn imap_starttls_is_negotiated_after_the_greeting() {
        let (port, h) = serve(vec![("* OK ready\r\n", Some("a1 STARTTLS")), ("a1 OK Begin TLS\r\n", None)]);
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        assert_eq!(read_greeting(&mut s, Protocol::Imap).expect("greeting"), "* OK ready");
        negotiate_starttls(&mut s, Protocol::Imap).expect("STARTTLS accepted");
        assert_eq!(h.join().expect("server"), vec!["a1 STARTTLS".to_string()]);
    }

    #[test]
    fn smtp_starttls_reads_multiline_replies_and_a_refusal_is_an_error() {
        let (port, h) = serve(vec![
            ("220-mx hello\r\n220 ready\r\n", Some("EHLO whoosh.app")),
            ("250-mx\r\n250 STARTTLS\r\n", Some("STARTTLS")),
            ("220 go ahead\r\n", None),
        ]);
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        assert_eq!(read_greeting(&mut s, Protocol::Smtp).expect("greeting"), "220 ready");
        negotiate_starttls(&mut s, Protocol::Smtp).expect("STARTTLS accepted");
        assert_eq!(h.join().expect("server"), vec!["EHLO whoosh.app".to_string(), "STARTTLS".to_string()]);

        let (port, _h) = serve(vec![("* OK ready\r\n", Some("a1 STARTTLS")), ("a1 BAD no\r\n", None)]);
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        read_greeting(&mut s, Protocol::Imap).expect("greeting");
        assert!(negotiate_starttls(&mut s, Protocol::Imap).is_err());
    }

    #[test]
    fn a_non_mail_greeting_is_refused() {
        let (port, _h) = serve(vec![("HTTP/1.1 400 Bad Request\r\n", None)]);
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        assert!(read_greeting(&mut s, Protocol::Imap).is_err());
    }
}
