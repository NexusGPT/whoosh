//! A small IMAP server for tests: listens on 127.0.0.1 only, lives in this process, and dies
//! when `FakeImap` is dropped. It speaks the subset the adapter uses (LOGIN, XOAUTH2, CAPABILITY,
//! ENABLE, SELECT with CONDSTORE/QRESYNC, LIST with SPECIAL-USE, UID SEARCH/FETCH/STORE/COPY/MOVE/
//! EXPUNGE, APPEND, IDLE, NOOP, LOGOUT) and keeps a log of every command it receives.
#![allow(dead_code)]
use std::{
    collections::BTreeSet,
    sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc, Mutex},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::future::BoxFuture;
use sync_imap::{ImapLimits, Sleeper, IMAP_CONN_PER_ACCOUNT};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::{watch, Notify},
    task::JoinHandle,
};

pub mod smtp;

use async_trait::async_trait;
use contracts::CoreError;
use sync_imap::{Auth, FetchOptions, FetchedMessage, ImapAdapter, ImapConfig, MessageSink, Security, TokenSource};

pub const OPTS: FetchOptions = FetchOptions { preview_bytes: 64 };

pub fn config(srv: &FakeImap, name: &str, auth: Auth) -> ImapConfig {
    ImapConfig {
        account_id: name.into(),
        account_name: name.into(),
        host: "127.0.0.1".into(),
        port: srv.port,
        security: Security::PlainLoopbackOnly,
        username: "owner@example.test".into(),
        auth,
        extra_roots_der: Vec::new(),
        limits: limits(),
    }
}

pub fn adapter(srv: &FakeImap, name: &str) -> Arc<ImapAdapter> {
    Arc::new(ImapAdapter::new(config(srv, name, Auth::Password("app-password".into()))))
}

/// Collects every batch the adapter delivers.
#[derive(Default)]
pub struct Sink { pub msgs: Vec<FetchedMessage> }
#[async_trait]
impl MessageSink for Sink {
    async fn deliver(&mut self, batch: Vec<FetchedMessage>) -> Result<(), CoreError> {
        self.msgs.extend(batch);
        Ok(())
    }
}

pub struct FixedTok(pub &'static str);
#[async_trait]
impl TokenSource for FixedTok {
    async fn access_token(&self) -> Result<String, String> { Ok(self.0.to_string()) }
}

trait Duplex: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Duplex for T {}
type Io = Box<dyn Duplex>;

/// How the fake secures its connections.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TlsMode { Implicit, StartTls }

pub fn test_ca_der() -> Vec<u8> { include_bytes!("tls/test-ca.der").to_vec() }

fn server_config() -> Arc<tokio_rustls::rustls::ServerConfig> {
    use tokio_rustls::rustls::{self, pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer}};
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cert = CertificateDer::from(include_bytes!("tls/test-leaf.der").to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(include_bytes!("tls/test-leaf-key.der").to_vec()));
    Arc::new(rustls::ServerConfig::builder_with_provider(provider).with_safe_default_protocol_versions().expect("tls versions").with_no_client_auth().with_single_cert(vec![cert], key).expect("test certificate"))
}

/// Reads a number from docs/CONSTANTS.md so a test never retypes a ledger value.
pub fn ledger(name: &str) -> u64 {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../docs/CONSTANTS.md")).expect("CONSTANTS.md");
    for line in text.lines() {
        let c: Vec<&str> = line.split('|').map(str::trim).collect();
        if c.len() > 2 && c[1] == name { return c[2].parse().expect("ledger row is an integer"); }
    }
    panic!("no ledger row {name}");
}

/// Limits for tests. The two Gmail/default numbers and the transcript size have no ledger row yet
/// (reported in docs/lanes/core-sync-imap.md "## Round 2"); the test values only prove the
/// table picks the right one.
pub fn limits() -> ImapLimits {
    ImapLimits {
        max_default: IMAP_CONN_PER_ACCOUNT,
        max_gmail: IMAP_CONN_PER_ACCOUNT + 5,
        idle_per_account: 1,
        backoff_start_ms: ledger("SYNC_BACKOFF_START_MS"),
        backoff_max_ms: ledger("SYNC_BACKOFF_MAX_MS"),
        transcript_lines: 300,
    }
}

pub struct Msg { pub uid: u32, pub flags: BTreeSet<String>, pub modseq: u64, pub raw: Vec<u8>, pub gm: Option<(u64, Vec<String>)> }

pub struct Mbox {
    pub name: String,
    pub special: Option<&'static str>,
    pub uid_validity: u32,
    pub next_uid: u32,
    pub highest_modseq: u64,
    pub msgs: Vec<Msg>,
    pub expunged: Vec<(u32, u64)>,
    pub version: u64,
}

pub struct State {
    pub boxes: Vec<Mbox>,
    pub log: Vec<String>,
    pub open_now: usize,
    pub max_open: usize,
    pub idle_starts: usize,
    pub idle_dones: usize,
    pub fetch_items: usize,
    pub qresync: bool,
    pub move_cap: bool,
    /// UIDPLUS's answer to COPY and MOVE (RFC 4315 COPYUID); false = a server that names no place.
    pub copyuid: bool,
    pub special_use: bool,
    pub password: String,
    pub token: String,
    pub refuse_append: bool,
    pub latency_ms: u64,
    pub cap_idle: bool,
    pub cap_condstore: bool,
    pub cap_special_use: bool,
    pub gmail: bool,
    pub login_disabled: bool,
    pub starttls_cap: bool,
    pub refuse_login: Option<String>,
    pub drop_epoch: u64,
    /// Faults the fake plays on chosen commands (a BYE that closes the connection, a BAD), in order of arrival.
    pub faults: Vec<Fault>,
    /// How many faults were played.
    pub faults_played: usize,
    /// body-r3: the body of a numbered part (`BODY.PEEK[2]`, `BODY[1.2]`) of a message, as the test's author wrote it: (UID, section, bytes).
    pub parts: Vec<(u32, String, Vec<u8>)>,
    /// imap-compress: the fake lists COMPRESS=DEFLATE (RFC 4978), and how it answers `COMPRESS DEFLATE`.
    pub cap_compress: bool,
    pub compress_answer: CompressAnswer,
    /// imap-compress: the raw bytes the fake read from its clients, in arrival order, kept only while `cap_compress` is on (at most RAW_KEPT).
    pub raw_in: Vec<u8>,
    /// imap-compress: how many connections switched to deflate after an OK.
    pub compressed_conns: usize,
}

/// What the fake answers to `COMPRESS DEFLATE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressAnswer { Ok, No, Bad }

/// Bound on `State::raw_in`.
pub const RAW_KEPT: usize = 4 * 1024 * 1024;

/// The connection as the fake sees it: straight through until `on` is set, then raw DEFLATE (RFC 4978 section 3: no zlib header) in
/// both directions. Every write is flushed with a sync flush so the client can read it at once.
struct Wire { inner: Io, on: Arc<AtomicBool>, st: Arc<Mutex<State>>, dec: flate2::Decompress, enc: flate2::Compress, pend_in: Vec<u8>, out: Vec<u8>, out_at: usize }

impl Wire {
    fn new(inner: Io, on: Arc<AtomicBool>, st: Arc<Mutex<State>>) -> Wire {
        Wire { inner, on, st, dec: flate2::Decompress::new(false), enc: flate2::Compress::new(flate2::Compression::default(), false), pend_in: Vec::new(), out: Vec::new(), out_at: 0 }
    }
    fn keep(&self, bytes: &[u8]) {
        let mut g = self.st.lock().unwrap();
        if g.cap_compress && g.raw_in.len() + bytes.len() <= RAW_KEPT { g.raw_in.extend_from_slice(bytes); }
    }
    fn drain(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        use std::task::Poll;
        while self.out_at < self.out.len() {
            match std::pin::Pin::new(&mut self.inner).poll_write(cx, &self.out[self.out_at..]) {
                Poll::Ready(Ok(0)) => return Poll::Ready(Err(std::io::ErrorKind::WriteZero.into())),
                Poll::Ready(Ok(n)) => self.out_at += n,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        self.out.clear();
        self.out_at = 0;
        Poll::Ready(Ok(()))
    }
}

impl AsyncRead for Wire {
    fn poll_read(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
        use std::task::Poll;
        let this = self.get_mut();
        if !this.on.load(Ordering::SeqCst) {
            let before = buf.filled().len();
            let r = std::pin::Pin::new(&mut this.inner).poll_read(cx, buf);
            if let Poll::Ready(Ok(())) = &r { let got = buf.filled()[before..].to_vec(); this.keep(&got); }
            return r;
        }
        loop {
            if !this.pend_in.is_empty() {
                let (ti, to) = (this.dec.total_in(), this.dec.total_out());
                let out = buf.initialize_unfilled();
                let status = this.dec.decompress(&this.pend_in, out, flate2::FlushDecompress::Sync);
                let (used, made) = ((this.dec.total_in() - ti) as usize, (this.dec.total_out() - to) as usize);
                if let Err(e) = status { return Poll::Ready(Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))); }
                this.pend_in.drain(..used);
                if made > 0 { buf.advance(made); return Poll::Ready(Ok(())); }
                if used > 0 { continue; }
            }
            let mut tmp = [0u8; 4096];
            let mut rb = tokio::io::ReadBuf::new(&mut tmp);
            match std::pin::Pin::new(&mut this.inner).poll_read(cx, &mut rb) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Ready(Ok(())) => {}
            }
            let got = rb.filled().to_vec();
            if got.is_empty() { return Poll::Ready(Ok(())); }
            this.keep(&got);
            this.pend_in.extend_from_slice(&got);
        }
    }
}

impl AsyncWrite for Wire {
    fn poll_write(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>, data: &[u8]) -> std::task::Poll<std::io::Result<usize>> {
        use std::task::Poll;
        let this = self.get_mut();
        if !this.on.load(Ordering::SeqCst) { return std::pin::Pin::new(&mut this.inner).poll_write(cx, data); }
        match this.drain(cx) { Poll::Ready(Ok(())) => {} Poll::Ready(Err(e)) => return Poll::Ready(Err(e)), Poll::Pending => return Poll::Pending }
        let mut used = 0usize;
        loop {
            this.out.reserve(data.len() + 64);
            let ti = this.enc.total_in();
            if let Err(e) = this.enc.compress_vec(&data[used..], &mut this.out, flate2::FlushCompress::Sync) { return Poll::Ready(Err(std::io::Error::other(e.to_string()))); }
            used += (this.enc.total_in() - ti) as usize;
            if used >= data.len() && this.out.len() < this.out.capacity() { break; }
        }
        match this.drain(cx) { Poll::Ready(Err(e)) => Poll::Ready(Err(e)), _ => Poll::Ready(Ok(data.len())) }
    }
    fn poll_flush(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> {
        use std::task::Poll;
        let this = self.get_mut();
        match this.drain(cx) { Poll::Ready(Ok(())) => {} other => return other }
        std::pin::Pin::new(&mut this.inner).poll_flush(cx)
    }
    fn poll_shutdown(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<std::io::Result<()>> { std::pin::Pin::new(&mut self.get_mut().inner).poll_shutdown(cx) }
}

/// What the fake answers instead of the command's normal answer.
#[derive(Clone, Debug)]
pub enum FaultKind {
    /// `* BYE <words>` and the connection closed. On a FETCH the first `after` messages go out first.
    Bye(String),
    /// `<tag> BAD <words>`; the connection stays open.
    Bad(String),
}

/// A fault on the commands whose text starts with `on` ("UID FETCH", "UID SEARCH", "SELECT", "IDLE", "LIST"): the first
/// `skip` such commands are answered normally, the next `times` get the fault.
#[derive(Clone, Debug)]
pub struct Fault { pub on: &'static str, pub skip: usize, pub times: usize, pub after: usize, pub kind: FaultKind }

impl Fault {
    pub fn bye(on: &'static str, words: &str) -> Fault { Fault { on, skip: 0, times: 1, after: 0, kind: FaultKind::Bye(words.into()) } }
    pub fn bad(on: &'static str, words: &str) -> Fault { Fault { on, skip: 0, times: 1, after: 0, kind: FaultKind::Bad(words.into()) } }
    pub fn skip(self, skip: usize) -> Fault { Fault { skip, ..self } }
    pub fn times(self, times: usize) -> Fault { Fault { times, ..self } }
    pub fn after(self, after: usize) -> Fault { Fault { after, ..self } }
}

/// The fault the command `rest` meets now, if any (and counts it).
fn take_fault(st: &Mutex<State>, rest: &str) -> Option<Fault> {
    let up = rest.to_ascii_uppercase();
    let mut g = st.lock().unwrap();
    let i = g.faults.iter().position(|f| f.times > 0 && up.starts_with(f.on))?;
    if g.faults[i].skip > 0 { g.faults[i].skip -= 1; return None; }
    g.faults[i].times -= 1;
    g.faults_played += 1;
    Some(g.faults[i].clone())
}

pub struct FakeImap {
    pub port: u16,
    pub state: Arc<Mutex<State>>,
    bump: Arc<Notify>,
    task: JoinHandle<()>,
    conns: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Drop for FakeImap {
    fn drop(&mut self) {
        self.task.abort();
        for c in self.conns.lock().unwrap().drain(..) { c.abort(); }
    }
}

pub fn raw_message(n: u32) -> Vec<u8> {
    format!("From: Sender {n} <sender{n}@example.test>\r\nTo: owner@example.test\r\nSubject: Message {n}\r\nMessage-ID: <{n}@example.test>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\nBody of message {n} is here.\r\n").into_bytes()
}

fn new_box(name: &str, special: Option<&'static str>) -> Mbox {
    Mbox { name: name.into(), special, uid_validity: 1000, next_uid: 1, highest_modseq: 1, msgs: Vec::new(), expunged: Vec::new(), version: 0 }
}

impl FakeImap {
    pub async fn start() -> FakeImap { Self::start_with(true).await }

    pub async fn start_with(special_use: bool) -> FakeImap { Self::start_inner(special_use, None).await }

    /// A server that speaks TLS: from the first byte, or after STARTTLS.
    pub async fn start_secure(mode: TlsMode) -> FakeImap { Self::start_inner(true, Some(mode)).await }

    async fn start_inner(special_use: bool, tls: Option<TlsMode>) -> FakeImap {
        let names: [(&str, Option<&'static str>); 6] = if special_use {
            [("INBOX", None), ("Sent", Some("\\Sent")), ("Drafts", Some("\\Drafts")), ("Trash", Some("\\Trash")), ("Junk", Some("\\Junk")), ("Archive", Some("\\Archive"))]
        } else {
            [("INBOX", None), ("Sent Messages", None), ("Drafts", None), ("Deleted Messages", None), ("Junk", None), ("Archive", None)]
        };
        let state = Arc::new(Mutex::new(State {
            boxes: names.iter().map(|(n, s)| new_box(n, *s)).collect(),
            log: Vec::new(), open_now: 0, max_open: 0, idle_starts: 0, idle_dones: 0, fetch_items: 0,
            qresync: true, move_cap: true, copyuid: true, special_use,
            password: "app-password".into(), token: "token-1".into(), refuse_append: false, latency_ms: 0,
            cap_idle: true, cap_condstore: true, cap_special_use: true, gmail: false, login_disabled: false,
            starttls_cap: tls == Some(TlsMode::StartTls), refuse_login: None, drop_epoch: 0,
            faults: Vec::new(), faults_played: 0, parts: Vec::new(),
            cap_compress: false, compress_answer: CompressAnswer::Ok, raw_in: Vec::new(), compressed_conns: 0,
        }));
        let tls_cfg = tls.map(|_| server_config());
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        let bump = Arc::new(Notify::new());
        let conns: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
        let (st, bp, cs) = (state.clone(), bump.clone(), conns.clone());
        let counter = Arc::new(AtomicUsize::new(0));
        let task = tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else { break };
                let id = counter.fetch_add(1, Ordering::SeqCst) + 1;
                {
                    let mut g = st.lock().unwrap();
                    g.open_now += 1;
                    g.max_open = g.max_open.max(g.open_now);
                }
                let (st2, bp2, cfg2) = (st.clone(), bp.clone(), tls_cfg.clone());
                let h = tokio::spawn(async move {
                    let io: Result<Io, std::io::Error> = match (tls, cfg2.clone()) {
                        (Some(TlsMode::Implicit), Some(cfg)) => tokio_rustls::TlsAcceptor::from(cfg).accept(sock).await.map(|s| Box::new(s) as Io),
                        _ => Ok(Box::new(sock) as Io),
                    };
                    // imap-compress: the connection goes through `Wire`, which deflates after a COMPRESS the server accepted
                    let on = Arc::new(AtomicBool::new(false));
                    let io = io.map(|io| Box::new(Wire::new(io, on.clone(), st2.clone())) as Io);
                    if let Ok(io) = io { let _ = serve(io, id, st2.clone(), bp2, cfg2, on).await; }
                    st2.lock().unwrap().open_now -= 1;
                });
                cs.lock().unwrap().push(h);
            }
        });
        FakeImap { port, state, bump, task, conns }
    }

    pub fn seed(&self, mbox: &str, n: u32) {
        let mut g = self.state.lock().unwrap();
        let b = g.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        for _ in 0..n { push_msg(b, raw_message(b.next_uid), &[]); }
    }

    pub fn add_message(&self, mbox: &str) -> u32 {
        let uid = {
            let mut g = self.state.lock().unwrap();
            let b = g.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
            let uid = b.next_uid;
            push_msg(b, raw_message(uid), &[]);
            uid
        };
        self.bump.notify_waiters();
        uid
    }

    pub fn change_flags(&self, mbox: &str, uids: &[u32], flag: &str) {
        {
            let mut g = self.state.lock().unwrap();
            let b = g.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
            for u in uids {
                b.highest_modseq += 1;
                let m = b.highest_modseq;
                if let Some(msg) = b.msgs.iter_mut().find(|m| m.uid == *u) { msg.flags.insert(flag.into()); msg.modseq = m; }
            }
            b.version += 1;
        }
        self.bump.notify_waiters();
    }

    pub fn expunge(&self, mbox: &str, uids: &[u32]) {
        let mut g = self.state.lock().unwrap();
        let b = g.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        for u in uids {
            b.highest_modseq += 1;
            let m = b.highest_modseq;
            b.msgs.retain(|x| x.uid != *u);
            b.expunged.push((*u, m));
        }
    }

    /// Adds a Gmail-style message: the same X-GM-MSGID may sit in several folders.
    pub fn add_gmail_message(&self, mbox: &str, msg_id: u64, labels: &[&str]) -> u32 {
        let mut g = self.state.lock().unwrap();
        let b = g.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        let uid = b.next_uid;
        push_msg(b, raw_message(msg_id as u32), &[]);
        b.msgs.last_mut().unwrap().gm = Some((msg_id, labels.iter().map(|l| l.to_string()).collect()));
        uid
    }

    /// Closes every connection that is idling (the server restarts, a NAT drops the socket).
    pub fn drop_idle(&self) {
        self.state.lock().unwrap().drop_epoch += 1;
        self.bump.notify_waiters();
    }

    pub fn set_uid_validity(&self, mbox: &str, v: u32) {
        self.state.lock().unwrap().boxes.iter_mut().find(|b| b.name == mbox).unwrap().uid_validity = v;
    }

    pub fn log(&self) -> Vec<String> { self.state.lock().unwrap().log.clone() }
    pub fn clear_log(&self) { self.state.lock().unwrap().log.clear(); }
    pub fn uids(&self, mbox: &str) -> Vec<u32> {
        self.state.lock().unwrap().boxes.iter().find(|b| b.name == mbox).unwrap().msgs.iter().map(|m| m.uid).collect()
    }
    pub fn flags_of(&self, mbox: &str, uid: u32) -> Vec<String> {
        let g = self.state.lock().unwrap();
        g.boxes.iter().find(|b| b.name == mbox).unwrap().msgs.iter().find(|m| m.uid == uid).map(|m| m.flags.iter().cloned().collect()).unwrap_or_default()
    }
    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R { f(&mut self.state.lock().unwrap()) }
    /// Plays `fault` on the next matching commands.
    pub fn fault(&self, fault: Fault) { self.state.lock().unwrap().faults.push(fault); }
    pub fn faults_played(&self) -> usize { self.state.lock().unwrap().faults_played }

    /// Waits (real time, bounded) until `pred` holds on the server state.
    pub async fn wait_for(&self, what: &str, pred: impl Fn(&State) -> bool) {
        for _ in 0..500 {
            if pred(&self.state.lock().unwrap()) { return; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the server never reached: {what}");
    }
}

fn push_msg(b: &mut Mbox, raw: Vec<u8>, flags: &[&str]) {
    b.highest_modseq += 1;
    let uid = b.next_uid;
    b.next_uid += 1;
    b.msgs.push(Msg { uid, flags: flags.iter().map(|f| f.to_string()).collect(), modseq: b.highest_modseq, raw, gm: None });
    b.version += 1;
}

/// A clock the test moves by hand.
pub struct ManualClock { now: watch::Sender<u64>, waiting: Arc<AtomicUsize> }

impl ManualClock {
    pub fn new() -> Self { ManualClock { now: watch::channel(0).0, waiting: Arc::new(AtomicUsize::new(0)) } }
    pub fn advance(&self, d: Duration) { self.now.send_modify(|t| *t += d.as_millis() as u64); }
    pub fn sleepers(&self) -> usize { self.waiting.load(Ordering::SeqCst) }
}

struct Waiting(Arc<AtomicUsize>);
impl Drop for Waiting { fn drop(&mut self) { self.0.fetch_sub(1, Ordering::SeqCst); } }

impl Sleeper for ManualClock {
    fn sleep(&self, d: Duration) -> BoxFuture<'static, ()> {
        let mut rx = self.now.subscribe();
        let target = *rx.borrow() + d.as_millis() as u64;
        self.waiting.fetch_add(1, Ordering::SeqCst);
        let guard = Waiting(self.waiting.clone());
        Box::pin(async move {
            let _g = guard;
            loop {
                if *rx.borrow() >= target { break; }
                if rx.changed().await.is_err() { break; }
            }
        })
    }
}

// ---------------------------------------------------------------- the protocol

fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            ' ' => i += 1,
            '"' => { let mut j = i + 1; let mut t = String::new(); while j < b.len() && b[j] != '"' { t.push(b[j]); j += 1; } out.push(t); i = j + 1; }
            '(' => { let mut depth = 0; let mut j = i; let mut t = String::new(); loop { if b[j] == '(' { depth += 1 } if b[j] == ')' { depth -= 1 } t.push(b[j]); j += 1; if depth == 0 || j >= b.len() { break } } out.push(t); i = j; }
            _ => { let mut j = i; let mut t = String::new(); while j < b.len() && b[j] != ' ' { t.push(b[j]); j += 1; } out.push(t); i = j; }
        }
    }
    out
}

fn parse_set(s: &str, max: u32) -> Vec<(u32, u32)> {
    s.split(',').filter(|p| !p.is_empty()).map(|p| {
        let n = |x: &str| if x == "*" { max } else { x.parse().unwrap_or(0) };
        match p.split_once(':') { Some((a, b)) => { let (a, b) = (n(a), n(b)); (a.min(b), a.max(b)) } None => (n(p), n(p)) }
    }).collect()
}
fn in_set(set: &[(u32, u32)], v: u32) -> bool { set.iter().any(|(a, b)| v >= *a && v <= *b) }

fn ranges(mut v: Vec<u32>) -> String {
    v.sort_unstable();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() { let mut j = i; while j + 1 < v.len() && v[j + 1] == v[j] + 1 { j += 1 } out.push(if i == j { v[i].to_string() } else { format!("{}:{}", v[i], v[j]) }); i = j + 1; }
    out.join(",")
}

fn split_raw(raw: &[u8]) -> (&[u8], &[u8]) {
    let p = raw.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4).unwrap_or(raw.len());
    (&raw[..p], &raw[p..])
}

fn fetch_line(seq: u32, m: &Msg, items: &str, by_uid: bool, with_modseq: bool, parts: &[(u32, String, Vec<u8>)]) -> Vec<u8> {
    let up = items.to_ascii_uppercase();
    let mut out: Vec<u8> = format!("* {seq} FETCH (").into_bytes();
    let mut first = true;
    let mut sep = |o: &mut Vec<u8>| { if !first { o.push(b' ') } first = false; };
    if by_uid || up.contains("UID") { sep(&mut out); out.extend(format!("UID {}", m.uid).bytes()); }
    if up.contains("FLAGS") { sep(&mut out); out.extend(format!("FLAGS ({})", m.flags.iter().cloned().collect::<Vec<_>>().join(" ")).bytes()); }
    if with_modseq { sep(&mut out); out.extend(format!("MODSEQ ({})", m.modseq).bytes()); }
    if let Some((id, labels)) = &m.gm {
        if up.contains("X-GM-MSGID") { sep(&mut out); out.extend(format!("X-GM-MSGID {id}").bytes()); }
        if up.contains("X-GM-LABELS") {
            let shown: Vec<String> = labels.iter().map(|l| if l.contains(' ') { format!("\"{l}\"") } else { l.clone() }).collect();
            sep(&mut out); out.extend(format!("X-GM-LABELS ({})", shown.join(" ")).bytes());
        }
    }
    if up.contains("INTERNALDATE") { sep(&mut out); out.extend(format!("INTERNALDATE \"07-Nov-2026 10:{:02}:{:02} +0000\"", (m.uid / 60) % 60, m.uid % 60).bytes()); }
    if up.contains("RFC822.SIZE") { sep(&mut out); out.extend(format!("RFC822.SIZE {}", m.raw.len()).bytes()); }
    let (h, t) = split_raw(&m.raw);
    if up.contains("BODY.PEEK[HEADER]") { sep(&mut out); out.extend(format!("BODY[HEADER] {{{}}}\r\n", h.len()).bytes()); out.extend(h); }
    // A144: only the named header fields (RFC 9051 6.4.5 HEADER.FIELDS), each with its folded lines, then the blank line
    const FIELDS: &str = "BODY.PEEK[HEADER.FIELDS (";
    if let Some(p) = up.find(FIELDS) {
        let names: Vec<&str> = up[p + FIELDS.len()..].split(')').next().unwrap_or("").split_whitespace().collect();
        let text = String::from_utf8_lossy(h).to_string();
        let mut picked: Vec<u8> = Vec::new();
        let mut keep = false;
        for line in text.split_inclusive("\r\n") {
            if line == "\r\n" { break; }
            if !line.starts_with([' ', '\t']) { keep = line.split(':').next().is_some_and(|n| names.iter().any(|w| w.eq_ignore_ascii_case(n.trim()))); }
            if keep { picked.extend(line.as_bytes()); }
        }
        picked.extend(b"\r\n");
        sep(&mut out); out.extend(format!("BODY[HEADER.FIELDS ({})] {{{}}}\r\n", names.join(" "), picked.len()).bytes()); out.extend(&picked);
    }
    if let Some(p) = up.find("BODY.PEEK[TEXT]<0.") {
        let n: usize = up[p + 18..].split('>').next().unwrap_or("0").parse().unwrap_or(0);
        let part = &t[..t.len().min(n)];
        sep(&mut out); out.extend(format!("BODY[TEXT]<0> {{{}}}\r\n", part.len()).bytes()); out.extend(part);
    }
    if up.contains("BODY.PEEK[]") { sep(&mut out); out.extend(format!("BODY[] {{{}}}\r\n", m.raw.len()).bytes()); out.extend(&m.raw); }
    // body-r3: a numbered part, when the test wrote one for this message
    for item in up.split("BODY.PEEK[").skip(1) {
        let section = item.split(']').next().unwrap_or("");
        if !section.starts_with(|c: char| c.is_ascii_digit()) { continue; }
        if let Some((_, _, b)) = parts.iter().find(|(u, k, _)| *u == m.uid && k.eq_ignore_ascii_case(section)) { sep(&mut out); out.extend(format!("BODY[{section}] {{{}}}\r\n", b.len()).bytes()); out.extend(b); }
    }
    out.extend(b")\r\n");
    out
}

async fn serve(sock: Io, id: usize, st: Arc<Mutex<State>>, bump: Arc<Notify>, tls: Option<Arc<tokio_rustls::rustls::ServerConfig>>, compress_on: Arc<AtomicBool>) -> std::io::Result<()> {
    let (r, mut w) = tokio::io::split(sock);
    let mut r = BufReader::new(r);
    let mut tls_done = tls.is_some() && !st.lock().unwrap().starttls_cap;
    w.write_all(b"* OK [CAPABILITY IMAP4rev1] fake ready\r\n").await?;
    let mut selected: Option<String> = None;
    let mut qresync_on = false;
    let mut line = String::new();
    loop {
        line.clear();
        if r.read_line(&mut line).await? == 0 { return Ok(()); }
        let line_t = line.trim_end().to_string();
        let (tag, rest) = line_t.split_once(' ').unwrap_or((&line_t, ""));
        let toks = tokens(rest);
        let cmd = toks.first().map(|s| s.to_ascii_uppercase()).unwrap_or_default();
        let latency = {
            let mut g = st.lock().unwrap();
            g.log.push(format!("c{id} {rest}"));
            g.latency_ms
        };
        if latency > 0 { tokio::time::sleep(Duration::from_millis(latency)).await; }
        let fault = take_fault(&st, rest);
        let mut fetch_bye: Option<(usize, String)> = None;
        match fault.map(|f| (f.kind, f.after)) {
            Some((FaultKind::Bad(why), _)) => { w.write_all(format!("{tag} BAD {why}\r\n").as_bytes()).await?; continue; }
            Some((FaultKind::Bye(why), after)) if cmd == "UID" || cmd == "FETCH" => {
                if rest.to_ascii_uppercase().split_whitespace().any(|t| t == "FETCH") { fetch_bye = Some((after, why)); }
                else { w.write_all(format!("* BYE {why}\r\n").as_bytes()).await?; return Ok(()); }
            }
            Some((FaultKind::Bye(why), _)) if cmd == "IDLE" => {
                w.write_all(format!("+ idling\r\n* BYE {why}\r\n").as_bytes()).await?;
                return Ok(());
            }
            Some((FaultKind::Bye(why), _)) => { w.write_all(format!("* BYE {why}\r\n").as_bytes()).await?; return Ok(()); }
            None => {}
        }
        let mut out: Vec<u8> = Vec::new();
        let mut closing = false;
        let caps = {
            let g = st.lock().unwrap();
            let mut c = String::from("IMAP4rev1");
            if g.cap_idle { c.push_str(" IDLE"); }
            if g.cap_condstore { c.push_str(" CONDSTORE"); }
            if g.qresync { c.push_str(" QRESYNC"); }
            c.push_str(" ENABLE");
            if g.move_cap { c.push_str(" MOVE"); }
            c.push_str(" UIDPLUS");
            if g.cap_special_use { c.push_str(" SPECIAL-USE"); }
            if g.gmail { c.push_str(" X-GM-EXT-1"); }
            c.push_str(" AUTH=XOAUTH2");
            if g.login_disabled { c.push_str(" LOGINDISABLED AUTH=PLAIN"); }
            if g.starttls_cap && !tls_done { c.push_str(" STARTTLS"); }
            if g.cap_compress { c.push_str(" COMPRESS=DEFLATE"); }
            c
        };
        match cmd.as_str() {
            "CAPABILITY" => out.extend(format!("* CAPABILITY {caps}\r\n{tag} OK done\r\n").bytes()),
            "NOOP" => out.extend(format!("{tag} OK done\r\n").bytes()),
            "LOGOUT" => { w.write_all(format!("* BYE bye\r\n{tag} OK done\r\n").as_bytes()).await?; return Ok(()); }
            "STARTTLS" => {
                match (&tls, tls_done) {
                    (Some(cfg), false) => {
                        w.write_all(format!("{tag} OK Begin TLS negotiation now\r\n").as_bytes()).await?;
                        let io = r.into_inner().unsplit(w);
                        let secured = tokio_rustls::TlsAcceptor::from(cfg.clone()).accept(io).await?;
                        let (rh, wh) = tokio::io::split(Box::new(secured) as Io);
                        r = BufReader::new(rh);
                        w = wh;
                        tls_done = true;
                        selected = None;
                        continue;
                    }
                    _ => out.extend(format!("{tag} BAD STARTTLS not available\r\n").bytes()),
                }
            }
            "LOGIN" => {
                let (ok, refuse, disabled) = { let g = st.lock().unwrap(); (g.password == toks.get(2).cloned().unwrap_or_default(), g.refuse_login.clone(), g.login_disabled) };
                out.extend(if disabled { format!("{tag} NO [PRIVACYREQUIRED] LOGIN is disabled, use AUTHENTICATE\r\n") }
                    else if let Some(why) = refuse { format!("{tag} NO [AUTHENTICATIONFAILED] {why}\r\n") }
                    else if ok { format!("{tag} OK [CAPABILITY {caps}] logged in\r\n") } else { format!("{tag} NO [AUTHENTICATIONFAILED] Invalid credentials\r\n") }.bytes());
            }
            "AUTHENTICATE" => {
                let mech = toks.get(1).map(|m| m.to_ascii_uppercase()).unwrap_or_default();
                w.write_all(b"+ \r\n").await?;
                let mut resp = String::new();
                r.read_line(&mut resp).await?;
                let got = STANDARD.decode(resp.trim()).unwrap_or_default();
                let ok = if mech == "PLAIN" {
                    let want = format!("\0owner@example.test\0{}", st.lock().unwrap().password);
                    got == want.as_bytes()
                } else {
                    let want = format!("user=owner@example.test\x01auth=Bearer {}\x01\x01", st.lock().unwrap().token);
                    got == want.as_bytes()
                };
                out.extend(if ok { format!("{tag} OK [CAPABILITY {caps}] authenticated\r\n") } else if mech == "PLAIN" { format!("{tag} NO AUTHENTICATE failed: password refused\r\n") } else { format!("{tag} NO AUTHENTICATE failed: token refused\r\n") }.bytes());
            }
            "COMPRESS" if st.lock().unwrap().cap_compress => {
                // RFC 4978: the tagged OK is the last uncompressed byte of the server; the next byte each way is deflated.
                let answer = st.lock().unwrap().compress_answer;
                let asked = toks.get(1).map(|m| m.to_ascii_uppercase()).unwrap_or_default();
                if compress_on.load(Ordering::SeqCst) { out.extend(format!("{tag} NO [COMPRESSIONACTIVE] DEFLATE active\r\n").bytes()); }
                else if asked != "DEFLATE" { out.extend(format!("{tag} NO [COMPRESSIONACTIVE] unknown mechanism\r\n").bytes()); }
                else if answer == CompressAnswer::No { out.extend(format!("{tag} NO compression is not available now\r\n").bytes()); }
                else if answer == CompressAnswer::Bad { out.extend(format!("{tag} BAD cannot compress\r\n").bytes()); }
                else {
                    w.write_all(format!("{tag} OK DEFLATE active\r\n").as_bytes()).await?;
                    w.flush().await?;
                    compress_on.store(true, Ordering::SeqCst);
                    st.lock().unwrap().compressed_conns += 1;
                    continue;
                }
            }
            "ENABLE" => { qresync_on = rest.to_ascii_uppercase().contains("QRESYNC"); out.extend(format!("* ENABLED QRESYNC\r\n{tag} OK done\r\n").bytes()); }
            "LIST" => {
                let g = st.lock().unwrap();
                for b in &g.boxes {
                    let attr = b.special.filter(|_| g.special_use).map(|s| format!(" {s}")).unwrap_or_default();
                    out.extend(format!("* LIST (\\HasNoChildren{attr}) \"/\" \"{}\"\r\n", b.name).bytes());
                }
                out.extend(format!("{tag} OK done\r\n").bytes());
            }
            "SELECT" | "EXAMINE" => {
                let name = toks.get(1).cloned().unwrap_or_default();
                let mut qsent = 0usize;
                {
                    let g = st.lock().unwrap();
                    match g.boxes.iter().find(|b| b.name == name) {
                        Some(b) => {
                            let upper = rest.to_ascii_uppercase();
                            let q_at = upper.find("QRESYNC (");
                            if q_at.is_some() && !qresync_on {
                                out.extend(format!("{tag} BAD QRESYNC is not enabled\r\n").bytes());
                            } else {
                                selected = Some(name);
                                out.extend(format!("* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)\r\n* {} EXISTS\r\n* 0 RECENT\r\n* OK [UIDVALIDITY {}] ok\r\n* OK [UIDNEXT {}] ok\r\n* OK [HIGHESTMODSEQ {}] ok\r\n", b.msgs.len(), b.uid_validity, b.next_uid, b.highest_modseq).bytes());
                                if let Some(at) = q_at {
                                    let nums: Vec<u64> = upper[at + 9..].split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect();
                                    let (want_uv, since) = (nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(0));
                                    if want_uv == u64::from(b.uid_validity) {
                                        let gone: Vec<u32> = b.expunged.iter().filter(|(_, m)| *m > since).map(|(u, _)| *u).collect();
                                        if !gone.is_empty() { out.extend(format!("* VANISHED (EARLIER) {}\r\n", ranges(gone)).bytes()); }
                                        for (i, m) in b.msgs.iter().enumerate().filter(|(_, m)| m.modseq > since) {
                                            out.extend(format!("* {} FETCH (UID {} FLAGS ({}) MODSEQ ({}))\r\n", i + 1, m.uid, m.flags.iter().cloned().collect::<Vec<_>>().join(" "), m.modseq).bytes());
                                            qsent += 1;
                                        }
                                    }
                                }
                                out.extend(format!("{tag} OK [READ-WRITE] done\r\n").bytes());
                            }
                        }
                        None => out.extend(format!("{tag} NO [NONEXISTENT] Unknown Mailbox\r\n").bytes()),
                    }
                }
                if qsent > 0 { st.lock().unwrap().fetch_items += qsent; }
            }
            "FETCH" | "UID" => {
                let sub = if cmd == "UID" { toks.get(1).cloned().unwrap_or_default().to_ascii_uppercase() } else { "FETCH".into() };
                let by_uid = cmd == "UID";
                let base = if by_uid { 2 } else { 1 };
                match sub.as_str() {
                    "FETCH" => {
                        let items = toks[base + 1..].join(" ");
                        let mut g = st.lock().unwrap();
                        let name = selected.clone().unwrap_or_default();
                        let b = g.boxes.iter().find(|b| b.name == name).unwrap();
                        let max = if by_uid { b.next_uid.saturating_sub(1) } else { b.msgs.len() as u32 };
                        let set = parse_set(&toks[base], max);
                        let since: Option<u64> = items.to_ascii_uppercase().split("CHANGEDSINCE ").nth(1).and_then(|x| x.split([' ', ')']).next()).and_then(|x| x.parse().ok());
                        let mut body: Vec<u8> = Vec::new();
                        if since.is_some() && items.to_ascii_uppercase().contains("VANISHED") && qresync_on {
                            let gone: Vec<u32> = b.expunged.iter().filter(|(_, m)| *m > since.unwrap()).map(|(u, _)| *u).collect();
                            if !gone.is_empty() { body.extend(format!("* VANISHED (EARLIER) {}\r\n", ranges(gone)).bytes()); }
                        }
                        let mut sent = 0;
                        for (i, m) in b.msgs.iter().enumerate() {
                            let key = if by_uid { m.uid } else { i as u32 + 1 };
                            if !in_set(&set, key) { continue; }
                            if since.map(|s| m.modseq <= s).unwrap_or(false) { continue; }
                            if fetch_bye.as_ref().is_some_and(|(after, _)| sent >= *after) { break; }
                            body.extend(fetch_line(i as u32 + 1, m, &items, by_uid, since.is_some(), &g.parts));
                            sent += 1;
                        }
                        g.fetch_items += sent;
                        if let Some((_, why)) = fetch_bye.take() {
                            // the first messages, then the server ends the connection in the middle of the FETCH
                            body.extend(format!("* BYE {why}\r\n").bytes());
                            closing = true;
                        }
                        out.extend(body);
                        if !closing { out.extend(format!("{tag} OK done\r\n").bytes()); }
                    }
                    "SEARCH" => {
                        let g = st.lock().unwrap();
                        let b = g.boxes.iter().find(|b| Some(&b.name) == selected.as_ref()).unwrap();
                        let q: Vec<String> = toks[base..].to_vec();
                        let found: Vec<String> = b.msgs.iter().filter(|m| match q.first().map(|s| s.to_ascii_uppercase()).as_deref() {
                            Some("UID") => in_set(&parse_set(&q[1], b.next_uid.saturating_sub(1)), m.uid),
                            _ => true,
                        }).map(|m| m.uid.to_string()).collect();
                        // fix-realcore round 9: an empty result is `* SEARCH` with no trailing space (RFC 3501 7.2.5), as a real server sends it
                        let list = if found.is_empty() { String::new() } else { format!(" {}", found.join(" ")) };
                        out.extend(format!("* SEARCH{list}\r\n{tag} OK done\r\n").bytes());
                    }
                    "STORE" => {
                        let mut g = st.lock().unwrap();
                        let name = selected.clone().unwrap_or_default();
                        let b = g.boxes.iter_mut().find(|b| b.name == name).unwrap();
                        let set = parse_set(&toks[base], b.next_uid.saturating_sub(1));
                        let op = toks[base + 1].to_ascii_uppercase();
                        let flags: Vec<String> = toks[base + 2].trim_matches(|c| c == '(' || c == ')').split(' ').filter(|f| !f.is_empty()).map(String::from).collect();
                        b.highest_modseq += 1;
                        let ms = b.highest_modseq;
                        for m in b.msgs.iter_mut().filter(|m| in_set(&set, m.uid)) {
                            match op.as_str() { o if o.starts_with('+') => m.flags.extend(flags.iter().cloned()), o if o.starts_with('-') => { for f in &flags { m.flags.remove(f); } } _ => m.flags = flags.iter().cloned().collect() }
                            m.modseq = ms;
                        }
                        b.version += 1;
                        out.extend(format!("{tag} OK done\r\n").bytes());
                    }
                    "COPY" | "MOVE" => {
                        let dest = toks[base + 1].clone();
                        let dest_exists = st.lock().unwrap().boxes.iter().any(|b| b.name == dest);
                        if !dest_exists {
                            w.write_all(format!("{tag} NO [TRYCREATE] no such mailbox\r\n").as_bytes()).await?;
                            continue;
                        }
                        let mut g = st.lock().unwrap();
                        let name = selected.clone().unwrap_or_default();
                        let (picked, seqs): (Vec<(Vec<u8>, BTreeSet<String>)>, Vec<(u32, u32)>) = {
                            let b = g.boxes.iter().find(|b| b.name == name).unwrap();
                            let set = parse_set(&toks[base], b.next_uid.saturating_sub(1));
                            (b.msgs.iter().filter(|m| in_set(&set, m.uid)).map(|m| (m.raw.clone(), m.flags.clone())).collect(),
                             b.msgs.iter().enumerate().filter(|(_, m)| in_set(&set, m.uid)).map(|(i, m)| (i as u32 + 1, m.uid)).collect())
                        };
                        let d = g.boxes.iter_mut().find(|b| b.name == dest).unwrap();
                        let mut placed: Vec<(u32, u32)> = Vec::new();
                        for ((raw, fl), (_, from)) in picked.into_iter().zip(seqs.iter()) { push_msg(d, raw, &[]); d.msgs.last_mut().unwrap().flags = fl; placed.push((*from, d.msgs.last().unwrap().uid)); }
                        let copyuid = if g.copyuid && !placed.is_empty() {
                            let d = g.boxes.iter().find(|b| b.name == dest).unwrap();
                            Some(format!("[COPYUID {} {} {}]", d.uid_validity, placed.iter().map(|(f, _)| f.to_string()).collect::<Vec<_>>().join(","), placed.iter().map(|(_, t)| t.to_string()).collect::<Vec<_>>().join(",")))
                        } else { None };
                        // RFC 6851: a MOVE names the places in an untagged OK, a COPY in the tagged answer
                        if let (Some(c), "MOVE") = (&copyuid, sub.as_str()) { out.extend(format!("* OK {c} moved\r\n").bytes()); }
                        if sub == "MOVE" {
                            let b = g.boxes.iter_mut().find(|b| b.name == name).unwrap();
                            let gone: Vec<u32> = seqs.iter().map(|(_, u)| *u).collect();
                            for u in &gone { b.highest_modseq += 1; let m = b.highest_modseq; b.msgs.retain(|x| x.uid != *u); b.expunged.push((*u, m)); }
                            if qresync_on { out.extend(format!("* VANISHED {}\r\n", ranges(gone)).bytes()); } else { for (s, _) in seqs.iter().rev() { out.extend(format!("* {s} EXPUNGE\r\n").bytes()); } }
                        }
                        match (&copyuid, sub.as_str()) { (Some(c), "COPY") => out.extend(format!("{tag} OK {c} done\r\n").bytes()), _ => out.extend(format!("{tag} OK done\r\n").bytes()) }
                    }
                    "EXPUNGE" => {
                        let mut g = st.lock().unwrap();
                        let name = selected.clone().unwrap_or_default();
                        let b = g.boxes.iter_mut().find(|b| b.name == name).unwrap();
                        let set = parse_set(&toks[base], b.next_uid.saturating_sub(1));
                        let gone: Vec<(usize, u32)> = b.msgs.iter().enumerate().filter(|(_, m)| m.flags.contains("\\Deleted") && in_set(&set, m.uid)).map(|(i, m)| (i + 1, m.uid)).collect();
                        for (_, u) in &gone { b.highest_modseq += 1; let m = b.highest_modseq; b.msgs.retain(|x| x.uid != *u); b.expunged.push((*u, m)); }
                        if qresync_on { if !gone.is_empty() { out.extend(format!("* VANISHED {}\r\n", ranges(gone.iter().map(|(_, u)| *u).collect())).bytes()); } }
                        else { for (s, _) in gone.iter().rev() { out.extend(format!("* {s} EXPUNGE\r\n").bytes()); } }
                        out.extend(format!("{tag} OK done\r\n").bytes());
                    }
                    _ => out.extend(format!("{tag} BAD unknown UID command\r\n").bytes()),
                }
            }
            "APPEND" => {
                let lit: usize = toks.last().and_then(|t| t.trim_matches(|c| c == '{' || c == '}' || c == '+').parse().ok()).unwrap_or(0);
                if st.lock().unwrap().refuse_append {
                    // A real server answers the literal request with NO before reading the bytes.
                    w.write_all(format!("{tag} NO [OVERQUOTA] Quota exceeded\r\n").as_bytes()).await?;
                    continue;
                }
                w.write_all(b"+ Ready for literal data\r\n").await?;
                let mut data = vec![0u8; lit];
                r.read_exact(&mut data).await?;
                let mut crlf = String::new();
                r.read_line(&mut crlf).await?;
                let flags: Vec<String> = toks.iter().find(|t| t.starts_with('(')).map(|t| t.trim_matches(|c| c == '(' || c == ')').split(' ').filter(|f| !f.is_empty()).map(String::from).collect()).unwrap_or_default();
                let mut g = st.lock().unwrap();
                match g.boxes.iter_mut().find(|b| Some(&b.name) == toks.get(1)) {
                    Some(b) => { push_msg(b, data, &[]); b.msgs.last_mut().unwrap().flags = flags.into_iter().collect(); let u = b.msgs.last().unwrap().uid; out.extend(format!("{tag} OK [APPENDUID {} {u}] done\r\n", b.uid_validity).bytes()); }
                    None => out.extend(format!("{tag} NO [TRYCREATE] no such mailbox\r\n").bytes()),
                }
            }
            "IDLE" => {
                {
                    let mut g = st.lock().unwrap();
                    g.idle_starts += 1;
                }
                w.write_all(b"+ idling\r\n").await?;
                let name = selected.clone().unwrap_or_default();
                let mut seen = st.lock().unwrap().boxes.iter().find(|b| b.name == name).map(|b| b.version).unwrap_or(0);
                let epoch0 = st.lock().unwrap().drop_epoch;
                loop {
                    let wake = bump.notified();
                    tokio::pin!(wake);
                    wake.as_mut().enable();
                    let mut l = String::new();
                    tokio::select! {
                        n = r.read_line(&mut l) => { if n? == 0 { return Ok(()); } break; }
                        _ = &mut wake => {
                            if st.lock().unwrap().drop_epoch != epoch0 { return Ok(()); }
                            let (v, n) = { let g = st.lock().unwrap(); g.boxes.iter().find(|b| b.name == name).map(|b| (b.version, b.msgs.len())).unwrap_or((0, 0)) };
                            if v != seen { seen = v; w.write_all(format!("* {n} EXISTS\r\n").as_bytes()).await?; }
                        }
                    }
                }
                {
                    let mut g = st.lock().unwrap();
                    g.idle_dones += 1;
                    g.log.push(format!("c{id} DONE"));
                }
                out.extend(format!("{tag} OK IDLE terminated\r\n").bytes());
            }
            _ => out.extend(format!("{tag} BAD unknown command\r\n").bytes()),
        }
        w.write_all(&out).await?;
        if closing { return Ok(()); }
    }
}
