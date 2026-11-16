//! TCP + TLS. TLS is the rule: implicit TLS (ssl) or a STARTTLS upgrade. Plain TCP is accepted for
//! loopback addresses only, so a typo cannot send a password in clear to a real host.
use std::{
    fmt, io,
    sync::atomic::{AtomicBool, Ordering},
    net::IpAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;

use crate::transcript::{Tapped, Transcript};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    /// TLS from the first byte (port 993 style).
    Tls,
    /// A plain greeting, then STARTTLS (RFC 3501 section 6.2.1), then TLS (port 143 style).
    StartTls,
    PlainLoopbackOnly,
}

trait Duplex: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Duplex for T {}

/// The stream, and the clock of its last received bytes (command_limit.rs: the limit runs on silence, not on a command's length).
pub struct Conn(Pin<Box<dyn Duplex>>, pub(crate) Arc<crate::command_limit::Activity>, Arc<AtomicBool>, Option<Arc<Transcript>>);

impl Conn {
    /// imap-compress: the connection after `COMPRESS DEFLATE` (RFC 4978): the library's deflate stream over this one. The tap under
    /// the compression goes quiet (it would record compressed bytes) and a new tap above it records the plain protocol lines.
    pub(crate) fn deflated(d: async_imap::DeflateStream<Conn>) -> Conn {
        let (activity, tap, under) = { let c = d.get_ref(); (c.1.clone(), c.3.clone(), c.2.clone()) };
        under.store(true, Ordering::SeqCst);
        let above = Tapped::new(d, tap.clone());
        let mute = above.mute.clone();
        Conn(Box::pin(above), activity, mute, tap)
    }
}

impl fmt::Debug for Conn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("Conn") }
}

impl AsyncRead for Conn {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let r = self.0.as_mut().poll_read(cx, buf);
        if matches!(r, Poll::Ready(Ok(()))) && buf.filled().len() > before { self.1.touch(); }
        r
    }
}
impl AsyncWrite for Conn {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let r = self.0.as_mut().poll_write(cx, buf);
        if matches!(r, Poll::Ready(Ok(n)) if n > 0) { self.1.touch(); }   // a send that is making progress is not silence either
        r
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> { self.0.as_mut().poll_flush(cx) }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> { self.0.as_mut().poll_shutdown(cx) }
}

/// Why a connection could not be opened; each case has its own words in a validation report.
#[derive(Debug)]
pub(crate) enum Fail {
    Unreachable(io::Error),
    Tls(io::Error),
    StartTls(String),
    Policy(String),
}

pub(crate) struct Opened {
    pub conn: Conn,
    /// True when the greeting was already read (the STARTTLS exchange reads it).
    pub greeted: bool,
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost") || host.parse::<IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
}

fn connector(extra_roots_der: &[Vec<u8>]) -> io::Result<tokio_rustls::TlsConnector> {
    let mut roots = rustls::RootCertStore::empty();
    let native = rustls_native_certs::load_native_certs();
    for cert in native.certs {
        roots.add(cert).map_err(|e| io::Error::other(e.to_string()))?;
    }
    for der in extra_roots_der {
        roots.add(rustls::pki_types::CertificateDer::from(der.clone())).map_err(|e| io::Error::other(format!("a trusted certificate was refused: {e}")))?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| io::Error::other(e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

fn server_name(host: &str) -> io::Result<rustls::pki_types::ServerName<'static>> {
    rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))
}

fn boxed<T: AsyncRead + AsyncWrite + Send + Unpin + 'static>(inner: T, tap: &Option<Arc<Transcript>>) -> Conn {
    let under = Tapped::new(inner, tap.clone());
    let mute = under.mute.clone();
    Conn(Box::pin(under), Arc::new(crate::command_limit::Activity::new()), mute, tap.clone())
}

/// Reads one CRLF line a byte at a time, so nothing past the line is consumed before TLS starts.
async fn plain_line(s: &mut Tapped<TcpStream>) -> io::Result<String> {
    let mut out = Vec::new();
    loop {
        let b = s.read_u8().await?;
        if b == b'\n' { break; }
        out.push(b);
    }
    Ok(String::from_utf8_lossy(&out).trim_end_matches('\r').to_string())
}

/// Greeting, STARTTLS, tagged answer. The server's own words come back on a refusal.
async fn start_tls_exchange(s: &mut Tapped<TcpStream>) -> Result<(), String> {
    let greeting = plain_line(s).await.map_err(|e| format!("no greeting: {e}"))?;
    if !greeting.starts_with("* OK") && !greeting.starts_with("* PREAUTH") {
        return Err(format!("the server greeted with: {greeting}"));
    }
    s.write_all(b"s1 STARTTLS\r\n").await.map_err(|e| e.to_string())?;
    loop {
        let l = plain_line(s).await.map_err(|e| format!("no answer to STARTTLS: {e}"))?;
        if let Some(rest) = l.strip_prefix("s1 ") {
            return if rest.to_ascii_uppercase().starts_with("OK") { Ok(()) } else { Err(rest.to_string()) };
        }
    }
}

/// The TCP dial: the guard's resolver when a test set one (netguard::set_resolver), else tokio's own lookup of the name.
async fn dial(host: &str, port: u16) -> io::Result<TcpStream> {
    match netguard::test_resolver() {
        Some(r) => { let addrs = r.lookup(host, port)?; TcpStream::connect(&addrs[..]).await }
        None => TcpStream::connect((host, port)).await,
    }
}

/// The guard's refusal in the words a failed open carries.
fn refused(e: contracts::CoreError) -> Fail {
    Fail::Policy(match e { contracts::CoreError::Action { failure } => failure.reason_words, other => other.to_string() })
}

pub(crate) async fn connect(host: &str, port: u16, security: Security, extra_roots_der: &[Vec<u8>], tap: &Option<Arc<Transcript>>) -> Result<Opened, Fail> {
    netguard::outbound_allowed(host).map_err(refused)?;   // engine-act-r6 round 5: a rig copy resolves and dials this Mac only
    match security {
        Security::PlainLoopbackOnly => {
            if !is_loopback(host) {
                return Err(Fail::Policy("plain IMAP is refused for a host that is not this Mac".into()));
            }
            let tcp = dial(host, port).await.map_err(Fail::Unreachable)?;
            Ok(Opened { conn: boxed(tcp, tap), greeted: false })
        }
        Security::Tls => {
            let tcp = dial(host, port).await.map_err(Fail::Unreachable)?;
            let name = server_name(host).map_err(Fail::Tls)?;
            let tls = connector(extra_roots_der).map_err(Fail::Tls)?.connect(name, tcp).await.map_err(Fail::Tls)?;
            Ok(Opened { conn: boxed(tls, tap), greeted: false })
        }
        Security::StartTls => {
            let tcp = dial(host, port).await.map_err(Fail::Unreachable)?;
            let mut plain = Tapped::new(tcp, tap.clone());
            start_tls_exchange(&mut plain).await.map_err(Fail::StartTls)?;
            let name = server_name(host).map_err(Fail::Tls)?;
            // The handshake bytes are not protocol text: the tap is put back on the TLS stream.
            let tls = connector(extra_roots_der).map_err(Fail::Tls)?.connect(name, plain.inner).await.map_err(Fail::Tls)?;
            Ok(Opened { conn: boxed(tls, tap), greeted: true })
        }
    }
}
