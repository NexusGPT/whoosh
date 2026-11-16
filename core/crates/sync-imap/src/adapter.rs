//! ImapAdapter: the ImapMailAdapter port over IMAP. One connection pool per account, capped at
//! the connection table (`ImapLimits`, filled from docs/CONSTANTS.md); every operation that reads a folder's numbers or
//! changes it re-SELECTs its folder. The body and header reads (body-r1) skip the SELECT when the leased session already has
//! that folder selected (`select_once`; the session remembers its selection in `Caps::selected`).
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex},
};

use async_imap::{error::Error as ImapError, Client, Session};
use async_trait::async_trait;
use contracts::{ActionFailed, CoreError, SyncCursor};
use futures::future::BoxFuture;
use imap_proto::{AttributeValue, Capability, Response, ResponseCode, Status};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    checked::{self, Fetched, Selected},
    command_limit,
    connections::{ConnectionTable, ImapLimits, Provider},
    consts::IMAP_UID_SET_CHUNK,
    envelope::{self, Raw},
    port::*,
    roles::{role_for, FolderRole},
    transcript::Transcript,
    transport::{self, Conn, Fail},
    validate::Step,
    xoauth2::{xoauth2_raw, XOAuth2},
};
pub use crate::transport::Security;

/// Supplies a fresh OAuth access token each time a connection is opened.
#[async_trait]
pub trait TokenSource: Send + Sync {
    async fn access_token(&self) -> Result<String, String>;
}

#[derive(Clone)]
pub enum Auth {
    /// LOGIN with an app password (Gmail, iCloud, any host).
    Password(String),
    /// SASL XOAUTH2 (Microsoft, Gmail OAuth).
    XOAuth2(Arc<dyn TokenSource>),
}

#[derive(Clone)]
pub struct ImapConfig {
    pub account_id: String,
    pub account_name: String,
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub username: String,
    pub auth: Auth,
    /// Extra trusted CA certificates (DER) for a server with a private CA; empty in normal use.
    /// The server name is still checked against the certificate.
    pub extra_roots_der: Vec<Vec<u8>>,
    /// The ledger numbers: connection table, backoff, transcript size.
    pub limits: ImapLimits,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Caps { pub(crate) idle: bool, pub(crate) condstore: bool, pub(crate) qresync: bool, mv: bool, uidplus: bool, pub(crate) special_use: bool, pub(crate) gmail: bool,
    /// imap-compress: this connection runs COMPRESS=DEFLATE (RFC 4978) in both directions. The one fact kept per connection (also `ImapAdapter::connections_compressed`).
    pub(crate) compressed: bool,
    /// body-r1: the folder this session has selected now, as `folder_key` (None: none, or unknown after a failed SELECT). It rides
    /// on the capabilities because a session is rebuilt from its parts after IDLE (idle.rs), and the selection survives IDLE.
    pub(crate) selected: Option<u64> }

impl Caps {
    /// The capability names worth showing in a validation report.
    pub(crate) fn names(&self) -> Vec<String> {
        [(self.idle, "IDLE"), (self.condstore, "CONDSTORE"), (self.qresync, "QRESYNC"), (self.mv, "MOVE"), (self.uidplus, "UIDPLUS"), (self.special_use, "SPECIAL-USE"), (self.gmail, "X-GM-EXT-1"), (self.compressed, "COMPRESS=DEFLATE")]
            .iter().filter(|(on, _)| *on).map(|(_, n)| n.to_string()).collect()
    }
}

/// imap-compress: how many per-connection compressed/clear facts an adapter keeps (a private bound so a long-lived account never grows the list; ledger: not yet a CONSTANTS row).
const OPENED_KEPT: usize = 64;

pub(crate) struct Sess { pub(crate) s: Session<Conn>, pub(crate) caps: Caps }

pub(crate) enum E { Imap(ImapError), Words { words: String, retryable: bool } }
impl From<ImapError> for E { fn from(e: ImapError) -> Self { E::Imap(e) } }
impl From<std::io::Error> for E { fn from(e: std::io::Error) -> Self { E::Words { words: e.to_string(), retryable: true } } }
fn words(w: impl Into<String>) -> E { E::Words { words: w.into(), retryable: false } }

pub struct ImapAdapter {
    cfg: ImapConfig,
    table: ConnectionTable,
    permits: Arc<Semaphore>,
    idle_slots: Arc<Semaphore>,
    pool: Mutex<Vec<Sess>>,
    roles: Mutex<HashMap<String, FolderRole>>,
    transcript: Arc<Transcript>,
    /// imap-compress: for each connection opened (the most recent `OPENED_KEPT`), whether it runs compressed. Oldest first.
    opened: Mutex<Vec<bool>>,
    /// body-r2: why the last body fetch of (folder, UID) held no body, in words (what the server sent back, the size it named).
    /// `body_miss` reads and clears it: the engine's log line says which case it was.
    body_misses: Mutex<HashMap<(String, u32), String>>,
    /// fix-realcore round 9: the (folder, UID) pairs a body fetch found the server no longer lists (`checked::fetch_bodies` `gone`), taken once.
    body_gone: Mutex<std::collections::HashSet<(String, u32)>>,
}

pub(crate) struct Lease { pub(crate) sess: Option<Sess>, pub(crate) _permit: OwnedSemaphorePermit }

#[derive(Clone)]
struct Ctx { account_id: String, folder: String, role: FolderRole, uid_validity: u32 }

/// One message the server reported as changed since a modseq.
struct Changed { uid: u32, flags: Vec<String>, modseq: Option<u64>, gm: Option<u64> }

/// What `SELECT folder (QRESYNC (uidvalidity modseq))` answered (RFC 7162 section 3.2.5).
#[derive(Default)]
struct QSelect { uid_validity: Option<u32>, highest_modseq: Option<u64>, changed: Vec<Changed>, vanished: Vec<u32> }

const FLAG_SILENT: [&str; 3] = ["+FLAGS.SILENT", "-FLAGS.SILENT", "FLAGS.SILENT"];

/// UIDs as an IMAP set: runs of consecutive numbers become ranges.
pub(crate) fn uid_set(uids: &[u32]) -> String {
    let mut v: Vec<u32> = uids.to_vec();
    v.sort_unstable();
    v.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 { j += 1; }
        out.push(if j == i { v[i].to_string() } else { format!("{}:{}", v[i], v[j]) });
        i = j + 1;
    }
    out.join(",")
}

/// A flag or keyword goes into a command line: only atom characters pass.
fn safe_flag(f: &str) -> Result<&str, E> {
    let body = f.strip_prefix('\\').unwrap_or(f);
    if !body.is_empty() && body.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '-' | '.')) { Ok(f) } else { Err(words(format!("flag {f:?} is not allowed"))) }
}

/// The message's own Date header, as epoch milliseconds: the send date the Sent copy carries.
fn send_date_ms(raw: &[u8]) -> Option<i64> {
    mail_parser::MessageParser::default().parse_headers(raw)?.date().map(|d| d.to_timestamp() * 1000)
}

/// "07-Nov-2026 10:00:00 +0000" from epoch milliseconds (UTC), for APPEND.
fn imap_date(ms: i64) -> String {
    const MON: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("\"{d:02}-{}-{y} {:02}:{:02}:{:02} +0000\"", MON[(m - 1) as usize], rem / 3600, rem % 3600 / 60, rem % 60)
}

impl ImapAdapter {
    pub fn new(cfg: ImapConfig) -> Self {
        let table = cfg.limits.table_for(&cfg.host);
        Self::with_table(cfg, table)
    }

    /// For an account whose provider is known by other means than its host name (the add-account detection
    /// found Google behind a custom domain, or a test names the provider without a real host).
    pub fn with_provider(cfg: ImapConfig, provider: Provider) -> Self {
        let mut table = cfg.limits.table_for(&cfg.host);
        if table.provider != provider {
            table = cfg.limits.table_for(match provider { Provider::Gmail => "imap.gmail.com", Provider::Other => "" });
        }
        Self::with_table(cfg, table)
    }

    fn with_table(cfg: ImapConfig, table: ConnectionTable) -> Self {
        let transcript = Transcript::new(cfg.limits.transcript_lines);
        Self {
            permits: Arc::new(Semaphore::new(table.max_connections)),
            idle_slots: Arc::new(Semaphore::new(table.idle_connections)),
            table,
            cfg,
            pool: Mutex::new(Vec::new()),
            roles: Mutex::new(HashMap::new()),
            transcript,
            opened: Mutex::new(Vec::new()),
            body_misses: Mutex::new(HashMap::new()),
            body_gone: Mutex::new(std::collections::HashSet::new()),
        }
    }

    pub fn account_name(&self) -> &str { &self.cfg.account_name }

    /// What the sync engine reads: connection cap, IDLE cap and reconnect backoff for this account.
    pub fn connection_table(&self) -> &ConnectionTable { &self.table }

    /// body-r2: the case of a body the server did not send for (`folder`, `uid`), in words, once (the answer is taken).
    pub fn body_miss(&self, folder: &str, uid: u32) -> Option<String> {
        self.body_misses.lock().expect("misses lock").remove(&(folder.to_string(), uid))
    }

    /// fix-realcore round 9: true, once, when the last body fetch found that the server no longer lists (`folder`, `uid`) (`UID SEARCH UID <n>`
    /// answered without it after three asks with no FETCH line): the mail left the folder.
    pub fn body_gone(&self, folder: &str, uid: u32) -> bool {
        self.body_gone.lock().expect("gone lock").remove(&(folder.to_string(), uid))
    }

    /// imap-compress: one entry per connection this adapter opened (sync, IDLE and transfer connections all open here): true when
    /// that connection runs COMPRESS=DEFLATE. Oldest first; only the last `OPENED_KEPT` are kept.
    pub fn connections_compressed(&self) -> Vec<bool> { self.opened.lock().expect("opened lock").clone() }

    /// The protocol transcript (passwords, tokens and message bodies already redacted).
    pub fn transcript(&self) -> Arc<Transcript> { self.transcript.clone() }

    pub(crate) fn core_err(&self, e: E) -> CoreError {
        let (reason_words, retryable) = match e {
            E::Words { words, retryable } => (words, retryable),
            E::Imap(ImapError::No(s)) | E::Imap(ImapError::Bad(s)) => (s, false),
            E::Imap(e @ (ImapError::Io(_) | ImapError::ConnectionLost)) => (e.to_string(), true),
            E::Imap(e) => (e.to_string(), false),
        };
        CoreError::Action { failure: ActionFailed { account_name: self.cfg.account_name.clone(), reason_words, retryable, code: None } }
    }

    async fn connect(&self) -> Result<Sess, E> {
        let dial = Dial { host: &self.cfg.host, port: self.cfg.port, security: self.cfg.security, username: &self.cfg.username, auth: &self.cfg.auth, extra_roots_der: &self.cfg.extra_roots_der };
        let sess = open_session(&dial, &Some(self.transcript.clone())).await.map_err(|f| E::Words { words: f.raw, retryable: f.retryable })?;
        let mut opened = self.opened.lock().expect("opened lock");
        if opened.len() >= OPENED_KEPT { opened.remove(0); }
        opened.push(sess.caps.compressed);
        Ok(sess)
    }

    pub(crate) async fn lease(&self) -> Result<Lease, CoreError> {
        let permit = self.permits.clone().acquire_owned().await.map_err(|_| self.core_err(words("the adapter is closed")))?;
        let pooled = self.pool.lock().expect("pool lock").pop();
        let sess = match pooled {
            Some(s) => s,
            None => self.connect().await.map_err(|e| self.core_err(e))?,
        };
        Ok(Lease { sess: Some(sess), _permit: permit })
    }

    pub(crate) fn give_back(&self, sess: Sess) {
        let mut pool = self.pool.lock().expect("pool lock");
        if pool.len() < self.table.max_connections { pool.push(sess); }
    }

    async fn run<R: Send>(&self, f: impl for<'a> FnOnce(&'a mut Sess) -> BoxFuture<'a, Result<R, E>> + Send) -> Result<R, CoreError> {
        let mut lease = self.lease().await?;
        let mut sess = lease.sess.take().expect("lease holds a session");
        match f(&mut sess).await {
            Ok(r) => {
                while sess.s.unsolicited_responses.try_recv().is_ok() {}
                self.give_back(sess);
                Ok(r)
            }
            Err(e) => Err(self.core_err(e)), // the session is dropped: its state is unknown
        }
    }

    fn role_of(&self, folder: &str) -> FolderRole {
        self.roles.lock().expect("roles lock").get(folder).copied().unwrap_or_else(|| role_for(folder, &[]))
    }

    fn ctx(&self, folder: &str, uid_validity: u32) -> Ctx {
        Ctx { account_id: self.cfg.account_id.clone(), folder: folder.to_string(), role: self.role_of(folder), uid_validity }
    }
}

/// Everything `open_session` needs to reach one server.
pub(crate) struct Dial<'a> {
    pub host: &'a str,
    pub port: u16,
    pub security: Security,
    pub username: &'a str,
    pub auth: &'a Auth,
    pub extra_roots_der: &'a [Vec<u8>],
}

/// Why a session could not be opened: the step that failed, words for the owner, and the
/// server's or system's raw text (what the sync engine logs).
pub(crate) struct OpenFail { pub step: Step, pub words: String, pub raw: String, pub retryable: bool }

pub(crate) fn imap_fail(step: Step, e: ImapError) -> OpenFail {
    let retryable = matches!(e, ImapError::Io(_) | ImapError::ConnectionLost);
    let raw = match e { ImapError::No(s) | ImapError::Bad(s) => s, other => other.to_string() };
    let words = match step {
        Step::ImapLogin => format!("The server refused the sign-in: {raw}"),
        Step::ImapList => format!("The server refused the folder list: {raw}"),
        _ => format!("The server answered with an error: {raw}"),
    };
    OpenFail { step, words, raw, retryable }
}

/// `imap_fail` for a command read by `checked`: the same step words, the server's text as it came.
pub(crate) fn step_fail(step: Step, e: E) -> OpenFail {
    match e {
        E::Imap(e) => imap_fail(step, e),
        E::Words { words: raw, retryable } => {
            let words = match step {
                Step::ImapList => format!("The server refused the folder list: {raw}"),
                _ => format!("The server answered with an error: {raw}"),
            };
            OpenFail { step, words, raw, retryable }
        }
    }
}

/// SASL PLAIN (RFC 4616) for a server that disables LOGIN before TLS or by policy.
struct PlainAuth { raw: String }
impl async_imap::Authenticator for PlainAuth {
    type Response = String;
    fn process(&mut self, _challenge: &[u8]) -> Self::Response { self.raw.clone() }
}

async fn pre_login_caps(client: &mut Client<Conn>) -> Result<Vec<String>, ImapError> {
    use tokio::io::AsyncWriteExt;
    // Client has no public run_command before login: the command goes out on the stream directly.
    client.get_mut().write_all(b"pc1 CAPABILITY\r\n").await?;
    client.get_mut().flush().await?;
    let mut atoms = Vec::new();
    loop {
        let r = client.read_response().await?.ok_or(ImapError::ConnectionLost)?;
        match r.parsed() {
            Response::Capabilities(cs) => {
                for c in cs {
                    match c {
                        Capability::Atom(a) => atoms.push(a.to_ascii_uppercase()),
                        Capability::Auth(a) => atoms.push(format!("AUTH={}", a.to_ascii_uppercase())),
                        _ => {}
                    }
                }
            }
            Response::Done { tag, .. } if tag.0 == "pc1" => break,
            _ => {}
        }
    }
    Ok(atoms)
}

/// Connect, secure (ssl or STARTTLS), sign in (LOGIN, AUTHENTICATE PLAIN or XOAUTH2) and read the
/// capabilities. Each failure names its step; validation shows those words as they are.
pub(crate) async fn open_session(d: &Dial<'_>, tap: &Option<Arc<Transcript>>) -> Result<Sess, OpenFail> {
    // the whole open (connect, TLS, greeting, sign-in, capabilities) is under the command limit (command_limit.rs): a server that
    // accepts the connection and then goes quiet ends in a retryable refusal
    match tokio::time::timeout(command_limit::limit(), open_session_unbounded(d, tap)).await {
        Ok(r) => r,
        Err(_) => {
            let raw = command_limit::timed_out_words("while it was being opened");
            Err(OpenFail { step: Step::ImapConnect, words: format!("{} did not finish opening the connection: {raw}. Check the server name and the port.", d.host), raw, retryable: true })
        }
    }
}

/// imap-compress: why one opening attempt ended. `CompressRefused`: the server listed COMPRESS=DEFLATE and answered NO or BAD. The
/// library's `Session::compress` takes the session by value and drops it on any error (async-imap-0.12.0 src/extensions/compress.rs:166-173),
/// so the connection is gone; the open goes again without asking for compression.
enum Attempt { Failed(OpenFail), CompressRefused }
impl From<OpenFail> for Attempt { fn from(f: OpenFail) -> Self { Attempt::Failed(f) } }

async fn open_session_unbounded(d: &Dial<'_>, tap: &Option<Arc<Transcript>>) -> Result<Sess, OpenFail> {
    match open_once(d, tap, true).await {
        Ok(s) => Ok(s),
        Err(Attempt::Failed(f)) => Err(f),
        Err(Attempt::CompressRefused) => open_once(d, tap, false).await.map_err(|a| match a { Attempt::Failed(f) => f, Attempt::CompressRefused => unreachable!("compression is not asked on the second attempt") }),
    }
}

async fn open_once(d: &Dial<'_>, tap: &Option<Arc<Transcript>>, ask_compress: bool) -> Result<Sess, Attempt> {
    let opened = transport::connect(d.host, d.port, d.security, d.extra_roots_der, tap).await.map_err(|f| match f {
        Fail::Unreachable(e) => OpenFail { step: Step::ImapConnect, words: format!("Cannot reach {} on port {}: {e}. Check the server name and the port.", d.host, d.port), raw: e.to_string(), retryable: true },
        Fail::Tls(e) => OpenFail { step: Step::ImapTls, words: format!("The secure connection to {} failed: {e}.", d.host), raw: e.to_string(), retryable: false },
        Fail::StartTls(e) => OpenFail { step: Step::ImapTls, words: format!("The server at {} would not switch to a secure connection (STARTTLS): {e}", d.host), raw: e, retryable: false },
        Fail::Policy(e) => OpenFail { step: Step::ImapConnect, words: e.clone(), raw: e, retryable: false },
    })?;
    let lost = |e: String| OpenFail { step: Step::ImapConnect, words: format!("{} closed the connection before it greeted us: {e}", d.host), raw: e, retryable: true };
    let mut client = Client::new(opened.conn);
    if !opened.greeted {
        match client.read_response().await {
            Ok(Some(_)) => {}
            Ok(None) => return Err(lost("the connection ended".into()).into()),
            Err(e) => return Err(lost(e.to_string()).into()),
        }
    }
    let pre = pre_login_caps(&mut client).await.map_err(|e| imap_fail(Step::ImapConnect, e))?;
    let plain_only = pre.iter().any(|c| c == "LOGINDISABLED") && pre.iter().any(|c| c == "AUTH=PLAIN");
    let mut s = match d.auth {
        Auth::Password(p) if plain_only => {
            client.authenticate("PLAIN", PlainAuth { raw: format!("\0{}\0{}", d.username, p) }).await.map_err(|(e, _)| imap_fail(Step::ImapLogin, e))?
        }
        Auth::Password(p) => client.login(d.username, p).await.map_err(|(e, _)| imap_fail(Step::ImapLogin, e))?,
        Auth::XOAuth2(src) => {
            let token = src.access_token().await.map_err(|w| OpenFail { step: Step::ImapLogin, words: format!("No sign-in token is available: {w}"), raw: w, retryable: false })?;
            let raw = xoauth2_raw(d.username, &token);
            client.authenticate("XOAUTH2", XOAuth2 { raw }).await.map_err(|(e, _)| imap_fail(Step::ImapLogin, e))?
        }
    };
    let c = s.capabilities().await.map_err(|e| imap_fail(Step::ImapLogin, e))?;
    let mut caps = Caps {
        idle: c.has_str("IDLE"),
        condstore: c.has_str("CONDSTORE") || c.has_str("QRESYNC"),
        qresync: c.has_str("QRESYNC"),
        mv: c.has_str("MOVE"),
        uidplus: c.has_str("UIDPLUS"),
        special_use: c.has_str("SPECIAL-USE"),
        gmail: c.has_str("X-GM-EXT-1"),
        compressed: false,
        selected: None,
    };
    // imap-compress (RFC 4978): after the sign-in and before any mailbox command, a server that lists COMPRESS=DEFLATE gets
    // `COMPRESS DEFLATE`. On OK both directions are deflated from the next byte (the library switches the stream only after the
    // tagged OK line is read). A connection already compressed is never asked again (`caps.compressed`).
    if ask_compress && !caps.compressed && c.has_str("COMPRESS=DEFLATE") {
        match s.compress(Conn::deflated).await {
            Ok(deflated) => { s = deflated; caps.compressed = true; }
            Err(ImapError::No(_)) | Err(ImapError::Bad(_)) => return Err(Attempt::CompressRefused),
            Err(e) => return Err(imap_fail(Step::ImapConnect, e).into()),
        }
    }
    if caps.qresync {
        s.run_command_and_check_ok("ENABLE QRESYNC").await.map_err(|e| imap_fail(Step::ImapLogin, e))?;
    }
    Ok(Sess { s, caps })
}

pub(crate) struct Listed { pub folder: Folder, pub attrs: Vec<String> }

/// LIST "" "*": every selectable folder with its attributes and mapped role.
pub(crate) async fn list_on(s: &mut Sess) -> Result<Vec<Listed>, E> {
    Ok(checked::list(s).await?.into_iter().filter(|r| r.selectable).map(|r| {
        let folder = Folder { role: role_for(&r.name, &r.attrs), name: r.name, delimiter: r.delimiter };
        Listed { folder, attrs: r.attrs }
    }).collect())
}

/// A mailbox name as one quoted IMAP string.
fn quote_mailbox(name: &str) -> Result<String, E> {
    if name.contains(['\r', '\n', '\0']) { return Err(words("the folder name has control characters")); }
    Ok(format!("\"{}\"", name.replace('\\', "\\\\").replace('"', "\\\"")))
}

/// SELECT with the QRESYNC parameter: one command returns the folder's numbers, the UIDs expunged
/// since `modseq` (VANISHED (EARLIER)) and the messages whose flags changed or that arrived.
async fn select_qresync(s: &mut Sess, folder: &str, uid_validity: u64, modseq: u64) -> Result<QSelect, E> {
    let mut q = QSelect::default();
    s.caps.selected = None;
    checked::run_to_end(s, &format!("SELECT {} (QRESYNC ({uid_validity} {modseq}))", quote_mailbox(folder)?), |r| match r {
        Response::Data { status: Status::Ok, outcome } => match &outcome.code {
            Some(ResponseCode::UidValidity(v)) => q.uid_validity = Some(*v),
            Some(ResponseCode::HighestModSeq(m)) => q.highest_modseq = Some(*m),
            _ => {}
        },
        Response::Vanished { uids, .. } => {
            for range in uids { q.vanished.extend(range.clone()); }
        }
        Response::Fetch(_, attrs) => {
            let mut c = Changed { uid: 0, flags: Vec::new(), modseq: None, gm: None };
            for a in attrs.iter() {
                match a {
                    AttributeValue::Uid(u) => c.uid = *u,
                    AttributeValue::Flags(f) => c.flags = f.iter().map(|x| x.to_string()).collect(),
                    AttributeValue::ModSeq(m) => c.modseq = Some(*m),
                    AttributeValue::GmailMsgId(g) => c.gm = Some(*g),
                    _ => {}
                }
            }
            if c.uid != 0 { q.changed.push(c); }
        }
        _ => {}
    }).await?;
    s.caps.selected = Some(folder_key(folder));
    Ok(q)
}

async fn select(s: &mut Sess, folder: &str) -> Result<Selected, E> {
    let condstore = s.caps.condstore;
    // RFC 9051 6.3.2: a SELECT that fails leaves no folder selected
    s.caps.selected = None;
    let mb = checked::select(s, &quote_mailbox(folder)?, condstore).await?;
    s.caps.selected = Some(folder_key(folder));
    Ok(mb)
}

/// A folder name as the session remembers its selection: a 64-bit FNV-1a of the exact name.
fn folder_key(folder: &str) -> u64 {
    folder.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// body-r1: SELECT only when this session does not have `folder` selected already. For the reads that need no fresh folder
/// numbers (a body, a header block); every call that reads UIDVALIDITY, EXISTS or HIGHESTMODSEQ selects again.
async fn select_once(s: &mut Sess, folder: &str) -> Result<(), E> {
    if s.caps.selected == Some(folder_key(folder)) { return Ok(()); }
    select(s, folder).await.map(|_| ())
}

/// The folder's persisted position: UIDVALIDITY and the highest modseq seen.
fn imap_cursor(uid_validity: u32, highest_mod_seq: u64) -> SyncCursor {
    SyncCursor::Imap { uid_validity: u64::from(uid_validity), highest_mod_seq }
}

fn cursor_of(mb: &Selected) -> SyncCursor {
    imap_cursor(mb.uid_validity.unwrap_or(0), mb.highest_modseq.unwrap_or(0))
}

fn from_fetch(ctx: &Ctx, f: &Fetched) -> Option<FetchedMessage> {
    let uid = f.uid?;
    let header = f.header.as_deref().unwrap_or_default();
    let mut m = envelope::build(Raw {
        account_id: &ctx.account_id,
        folder: &ctx.folder,
        role: ctx.role,
        uid,
        uid_validity: ctx.uid_validity,
        modseq: f.modseq,
        size: f.size,
        flags: f.flags.clone(),
        internal_ms: f.internal_ms.unwrap_or(0),
        header,
        text: f.text.as_deref().unwrap_or_default(),
        gmail: f.gm_msgid.map(|id| GmailIdentity { msg_id: id, labels: f.gm_labels.clone().unwrap_or_default() }),
    });
    // A144: the mail's own chain (References, else In-Reply-To), from the header block this fetch already holds
    m.references = crate::blocking::thread_chain(header);
    Some(m)
}

fn fetch_query(opts: FetchOptions, gmail: bool) -> String {
    let gm = if gmail { " X-GM-MSGID X-GM-LABELS" } else { "" };
    format!("(UID FLAGS INTERNALDATE RFC822.SIZE{gm} BODY.PEEK[HEADER] BODY.PEEK[TEXT]<0.{}>)", opts.preview_bytes)
}

async fn fetch_set(s: &mut Sess, ctx: &Ctx, set: &str, by_uid: bool, opts: FetchOptions) -> Result<Vec<FetchedMessage>, E> {
    let q = fetch_query(opts, s.caps.gmail);
    let fetches = checked::fetch(s, &format!("{}FETCH {set} {q}", if by_uid { "UID " } else { "" })).await?;
    let mut out: Vec<FetchedMessage> = fetches.iter().filter_map(|f| from_fetch(ctx, f)).collect();
    out.sort_by(|a, b| b.uid.cmp(&a.uid)); // newest first
    Ok(out)
}

/// A144: what changed in `folder` since the last sync on a server without CONDSTORE (Microsoft 365 / Exchange Online), which keeps no
/// change list. The SELECT gives the UIDVALIDITY (a changed one is a `Reset`); ONE `UID SEARCH UID 1:*` lists the UIDs the folder holds
/// now: a known UID missing from it is `vanished`, a UID above the highest known one is `new` (fetched with `fetch_set`). The known UIDs
/// still there come back with their current flags as `flag_changes` (`UID FETCH <set> (UID FLAGS)`, at most `IMAP_UID_SET_CHUNK` UIDs per
/// line). The cursor keeps the UIDVALIDITY and modseq 0 (no modseq exists here). With no known UID there is nothing to compare: `Reset`
/// (the engine reads the newest page), before any command.
async fn resync_plain(s: &mut Sess, folder: &str, ctx0: Ctx, want_uv: u64, known: &[u32], opts: FetchOptions) -> Result<Resync, E> {
    let Some(max_known) = known.iter().copied().max() else {
        return Ok(Resync::Reset { reason_words: "the server has no CONDSTORE, so changes cannot be listed".into() });
    };
    let mb = select(s, folder).await?;
    let uv = mb.uid_validity.unwrap_or(0);
    if u64::from(uv) != want_uv {
        return Ok(Resync::Reset { reason_words: "the folder was rebuilt on the server (UIDVALIDITY changed)".into() });
    }
    let live: BTreeSet<u32> = checked::uid_search(s, "UID 1:*").await?;
    let mut known_sorted: Vec<u32> = known.to_vec();
    known_sorted.sort_unstable();
    known_sorted.dedup();
    let (still, vanished): (Vec<u32>, Vec<u32>) = known_sorted.iter().copied().partition(|u| live.contains(u));
    let new_uids: Vec<u32> = live.iter().copied().filter(|u| *u > max_known).collect();
    let mut flag_changes: Vec<FlagChange> = Vec::with_capacity(still.len());
    for chunk in still.chunks(IMAP_UID_SET_CHUNK) {
        let fetched = checked::fetch(s, &format!("UID FETCH {} (UID FLAGS)", uid_set(chunk))).await?;
        for c in fetched {
            let Some(uid) = c.uid else { continue };
            if chunk.binary_search(&uid).is_ok() {
                flag_changes.push(FlagChange { uid, flags: c.flags, modseq: None, gmail_msg_id: c.gm_msgid });
            }
        }
    }
    let ctx = Ctx { uid_validity: uv, ..ctx0 };
    let mut new: Vec<FetchedMessage> = Vec::new();
    for chunk in new_uids.chunks(IMAP_UID_SET_CHUNK) {
        new.extend(fetch_set(s, &ctx, &uid_set(chunk), true, opts).await?);
    }
    new.sort_by(|a, b| b.uid.cmp(&a.uid)); // newest first, as `fetch_set` gives one chunk
    Ok(Resync::Delta(Delta { new, flag_changes, vanished, cursor: imap_cursor(uv, 0) }))
}

/// APPEND that keeps the server's own words when it refuses. The library's `append` turns any
/// refusal before the literal into one generic error, which would hide "quota exceeded".
async fn append_checked(s: &mut Sess, mailbox: &str, flags: Option<&str>, date: Option<&str>, content: &[u8]) -> Result<(), E> {
    use tokio::io::AsyncWriteExt;
    if mailbox.contains(['\r', '\n', '\0']) { return Err(words("the folder name has control characters")); }
    let quoted = format!("\"{}\"", mailbox.replace('\\', "\\\\").replace('"', "\\\""));
    let cmd = format!("APPEND {quoted}{}{} {{{}}}", flags.map(|f| format!(" {f}")).unwrap_or_default(), date.map(|d| format!(" {d}")).unwrap_or_default(), content.len());
    let id = command_limit::within("APPEND (sending it)", s.s.run_command(&cmd)).await?;
    let mut literal_sent = false;
    loop {
        let act = s.s.get_mut().1.clone();
        let r = command_limit::on_silence(&act, s.s.read_response()).await.map_err(|_| command_limit::timed_out("APPEND"))??.ok_or(ImapError::ConnectionLost)?;
        match r.parsed() {
            imap_proto::Response::Continue { .. } if !literal_sent => {
                let w = s.s.get_mut();
                // the literal goes out under the silence limit: a server that stops reading ends it, a slow uplink that keeps moving does not
                let act = w.1.clone();
                command_limit::on_silence(&act, async {
                    w.write_all(content).await?;
                    w.write_all(b"\r\n").await?;
                    w.flush().await
                }).await.map_err(|_| command_limit::timed_out("APPEND (taking the message)"))??;
                literal_sent = true;
            }
            imap_proto::Response::Done { tag, status, outcome, .. } if *tag == id => {
                return if *status == imap_proto::Status::Ok { Ok(()) } else {
                    Err(words(outcome.information.as_ref().map(|i| i.to_string()).unwrap_or_else(|| "the server refused the message".into())))
                };
            }
            _ => {}
        }
    }
}

#[async_trait]
impl ImapMailAdapter for ImapAdapter {
    async fn list_folders(&self) -> Result<Vec<Folder>, CoreError> {
        let folders: Vec<Folder> = self.run(|s| Box::pin(async move { Ok(list_on(s).await?.into_iter().map(|l| l.folder).collect()) })).await?;
        let mut roles = self.roles.lock().expect("roles lock");
        for f in &folders { roles.insert(f.name.clone(), f.role); }
        Ok(folders)
    }

    async fn folder_state(&self, folder: &str) -> Result<ImapFolderState, CoreError> {
        let f = folder.to_string();
        self.run(move |s| Box::pin(async move {
            let mb = select(s, &f).await?;
            let uid_validity = mb.uid_validity.ok_or_else(|| words("the server gave no UIDVALIDITY"))?;
            Ok(ImapFolderState { folder: f, uid_validity, highest_modseq: mb.highest_modseq, exists: mb.exists, condstore: s.caps.condstore, qresync: s.caps.qresync })
        })).await
    }

    async fn initial_sync(&self, folder: &str, first_screen: u32, history_page: u32, opts: FetchOptions, sink: &mut dyn MessageSink) -> Result<FolderSnapshot, CoreError> {
        let f = folder.to_string();
        let ctx0 = self.ctx(folder, 0);
        let (mb_cursor, exists, _uid_validity, first) = self.run(move |s| Box::pin(async move {
            let mb = select(s, &f).await?;
            let uv = mb.uid_validity.ok_or_else(|| words("the server gave no UIDVALIDITY"))?;
            let ctx = Ctx { uid_validity: uv, ..ctx0 };
            let msgs = if mb.exists == 0 || first_screen == 0 { Vec::new() } else {
                let start = mb.exists.saturating_sub(first_screen - 1).max(1);
                fetch_set(s, &ctx, &format!("{start}:*"), false, opts).await?
            };
            Ok((cursor_of(&mb), mb.exists, uv, msgs))
        })).await?;
        let mut oldest = first.last().map(|m| m.uid);
        let mut snapshot = FolderSnapshot { folder: folder.to_string(), cursor: mb_cursor, exists, messages: first.clone(), oldest_uid: oldest };
        // The first screen is delivered before any older message is requested.
        sink.deliver(first).await?;
        while let Some(below) = oldest {
            let page = self.history_page(folder, below, history_page, opts).await?;
            if page.messages.is_empty() { break; }
                oldest = page.oldest_uid;
            snapshot.oldest_uid = oldest;
            let done = page.done;
            sink.deliver(page.messages).await?;
            if done { break; }
        }
        Ok(snapshot)
    }

    async fn history_page(&self, folder: &str, before_uid: u32, limit: u32, opts: FetchOptions) -> Result<HistoryPage, CoreError> {
        if before_uid <= 1 || limit == 0 {
            return Ok(HistoryPage { messages: Vec::new(), oldest_uid: None, done: true });
        }
        let f = folder.to_string();
        let ctx0 = self.ctx(folder, 0);
        self.run(move |s| Box::pin(async move {
            let mb = select(s, &f).await?;
            let ctx = Ctx { uid_validity: mb.uid_validity.unwrap_or(0), ..ctx0 };
            // The newest page asks for "1:*": the highest number a server could hand out is not needed on the wire
            // (a server that reads UIDs as signed 32-bit numbers would refuse 4294967294).
            let upper = if before_uid == u32::MAX { "*".to_string() } else { (before_uid - 1).to_string() };
            let all: BTreeSet<u32> = checked::uid_search(s, &format!("UID 1:{upper}")).await?;
            let take: Vec<u32> = all.iter().rev().take(limit as usize).copied().collect();
            let done = all.len() <= take.len();
            if take.is_empty() { return Ok(HistoryPage { messages: Vec::new(), oldest_uid: None, done: true }); }
            let messages = fetch_set(s, &ctx, &uid_set(&take), true, opts).await?;
            let oldest_uid = messages.last().map(|m| m.uid);
            Ok(HistoryPage { messages, oldest_uid, done })
        })).await
    }

    async fn resync(&self, folder: &str, cursor: &SyncCursor, known_uids: &[u32], opts: FetchOptions) -> Result<Resync, CoreError> {
        let SyncCursor::Imap { uid_validity: want_uv, highest_mod_seq: since } = cursor else {
            return Err(CoreError::Invalid { field: "cursor".into(), reason_words: "this is not an IMAP cursor".into() });
        };
        let (want_uv, since) = (*want_uv, *since);
        let known: Vec<u32> = known_uids.to_vec();
        let f = folder.to_string();
        let ctx0 = self.ctx(folder, 0);
        self.run(move |s| Box::pin(async move {
            if !s.caps.condstore {
                // A144: no change list (Microsoft 365 / Exchange Online): the known UIDs are checked against what the server lists now
                return resync_plain(s, &f, ctx0, want_uv, &known, opts).await;
            }
            // One command with QRESYNC; SELECT plus a CHANGEDSINCE fetch with CONDSTORE alone.
            let (uv, highest, qsel) = if s.caps.qresync {
                let q = select_qresync(s, &f, want_uv, since).await?;
                (q.uid_validity.unwrap_or(0), q.highest_modseq, Some(q))
            } else {
                let mb = select(s, &f).await?;
                (mb.uid_validity.unwrap_or(0), mb.highest_modseq, None)
            };
            if u64::from(uv) != want_uv {
                return Ok(Resync::Reset { reason_words: "the folder was rebuilt on the server (UIDVALIDITY changed)".into() });
            }
            let new_cursor = imap_cursor(uv, highest.unwrap_or(since));
            let (changed, mut vanished): (Vec<Changed>, Vec<u32>) = match qsel {
                Some(q) => (q.changed, q.vanished),
                None => {
                    // fix-realcore round 9 (item 7e1fc235): an expunge need not raise HIGHESTMODSEQ on a CONDSTORE server without QRESYNC
                    // (RFC 7162 3.1: only a flag change does), so an unchanged HIGHESTMODSEQ still asks which known UIDs the server lists;
                    // before, it answered "nothing changed" and a mail deleted elsewhere stayed in the list (and its body was asked for days)
                    let changed: Vec<Changed> = if highest == Some(since) { Vec::new() } else {
                        let items = if s.caps.gmail { "(UID FLAGS X-GM-MSGID)" } else { "(UID FLAGS)" };
                        let fetched = checked::fetch(s, &format!("UID FETCH 1:* {items} (CHANGEDSINCE {since})")).await?;
                        fetched.into_iter().filter_map(|c| Some(Changed { uid: c.uid?, flags: c.flags, modseq: c.modseq, gm: c.gm_msgid })).collect()
                    };
                    // CONDSTORE without QRESYNC: a message is gone when the server no longer lists its UID.
                    // the highest known UID, whatever the order the caller passed them in (the store's query has no ORDER BY)
                    let gone = match known.iter().max() {
                        Some(max) => {
                            let live: BTreeSet<u32> = checked::uid_search(s, &format!("UID 1:{max}")).await?;
                            known.iter().filter(|u| !live.contains(u)).copied().collect()
                        }
                        None => Vec::new(),
                    };
                    (changed, gone)
                }
            };
            vanished.sort_unstable();
            vanished.dedup();
            let max_known = known.iter().copied().max().unwrap_or(0);
            let ctx = Ctx { uid_validity: uv, ..ctx0 };
            let mut new_uids = Vec::new();
            let mut flag_changes = Vec::new();
            for c in changed {
                if c.uid > max_known { new_uids.push(c.uid); } else {
                    flag_changes.push(FlagChange { uid: c.uid, flags: c.flags, modseq: c.modseq, gmail_msg_id: c.gm });
                }
            }
            let new = if new_uids.is_empty() { Vec::new() } else { fetch_set(s, &ctx, &uid_set(&new_uids), true, opts).await? };
            Ok(Resync::Delta(Delta { new, flag_changes, vanished, cursor: new_cursor }))
        })).await
    }

    async fn set_flags(&self, folder: &str, uids: &[u32], op: FlagOp, flags: &[&str]) -> Result<(), CoreError> {
        if uids.is_empty() { return Ok(()); }
        let (f, set) = (folder.to_string(), uid_set(uids));
        let flags: Vec<String> = flags.iter().map(|x| x.to_string()).collect();
        self.run(move |s| Box::pin(async move {
            let checked: Vec<&str> = flags.iter().map(|x| safe_flag(x)).collect::<Result<_, _>>()?;
            select(s, &f).await?;
            let verb = FLAG_SILENT[match op { FlagOp::Add => 0, FlagOp::Remove => 1, FlagOp::Replace => 2 }];
            checked::run_to_end(s, &format!("UID STORE {set} {verb} ({})", checked.join(" ")), |_| {}).await?;
            Ok(())
        })).await
    }

    async fn move_placed(&self, folder: &str, uids: &[u32], dest: &str) -> Result<Vec<MovedUid>, CoreError> {
        if uids.is_empty() { return Ok(Vec::new()); }
        let (f, d, set) = (folder.to_string(), dest.to_string(), uid_set(uids));
        self.run(move |s| Box::pin(async move {
            select(s, &f).await?;
            let target = quote_mailbox(&d)?;
            let placed = if s.caps.mv {
                checked::run_copy(s, &format!("UID MOVE {set} {target}")).await?
            } else if s.caps.uidplus {
                let placed = checked::run_copy(s, &format!("UID COPY {set} {target}")).await?;
                checked::run_to_end(s, &format!("UID STORE {set} +FLAGS.SILENT (\\Deleted)"), |_| {}).await?;
                checked::run_to_end(s, &format!("UID EXPUNGE {set}"), |_| {}).await?;
                placed
            } else {
                return Err(words("the server has neither MOVE nor UIDPLUS, so a safe move is not possible"));
            };
            Ok(placed.map(|c| c.pairs.into_iter().map(|(from_uid, to_uid)| MovedUid { from_uid, uid_validity: c.uid_validity, to_uid }).collect()).unwrap_or_default())
        })).await
    }

    async fn append(&self, folder: &str, raw: &[u8], flags: &[&str], internal_date_ms: Option<i64>) -> Result<(), CoreError> {
        let (f, raw) = (folder.to_string(), raw.to_vec());
        let flags: Vec<String> = flags.iter().map(|x| x.to_string()).collect();
        self.run(move |s| Box::pin(async move {
            let checked: Vec<&str> = flags.iter().map(|x| safe_flag(x)).collect::<Result<_, _>>()?;
            let fl = if checked.is_empty() { None } else { Some(format!("({})", checked.join(" "))) };
            let date = internal_date_ms.map(imap_date);
            append_checked(s, &f, fl.as_deref(), date.as_deref(), &raw).await?;
            Ok(())
        })).await
    }

    /// Save the copy of a message that was just delivered over SMTP: into the Sent folder (RFC 6154 `\\Sent`, else the
    /// names hosts use), flagged `\\Seen`, dated with the message's own Date header. Gmail saves every message
    /// sent through its SMTP server itself, so for a Gmail account nothing is appended (no connection is opened).
    async fn append_sent(&self, raw: &[u8]) -> Result<(), CoreError> {
        if self.table.provider == Provider::Gmail {
            return Ok(());
        }
        let folders = self.list_folders().await?;
        let sent = folders.iter().find(|f| f.role == FolderRole::Sent)
            .ok_or_else(|| self.core_err(words("no Sent folder was found on the server")))?;
        self.append(&sent.name, raw, &["\\Seen"], send_date_ms(raw)).await
    }

    async fn fetch_raw(&self, folder: &str, uid: u32) -> Result<(Vec<u8>, Vec<String>, Option<i64>), CoreError> {
        let f = folder.to_string();
        self.run(move |s| Box::pin(async move {
            select_once(s, &f).await?;
            let got = checked::fetch(s, &format!("UID FETCH {uid} (UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])")).await?;
            let case = checked::body_case(&got, uid);
            let m = got.into_iter().find(|m| m.uid == Some(uid)).ok_or_else(|| words(format!("the server has no message with UID {uid}: {case}")))?;
            let body = m.body.filter(|b| !b.is_empty()).ok_or_else(|| words(format!("the server sent no message body: {case}")))?;
            Ok((body, m.flags, m.internal_ms))
        })).await
    }

    async fn fetch_raw_many(&self, folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        if uids.is_empty() { return Ok(Vec::new()); }
        let (f, asked) = (folder.to_string(), uids.to_vec());
        let fetched = self.run(move |s| Box::pin(async move {
            select_once(s, &f).await?;
            checked::fetch_bodies(s, &asked).await
        })).await?;
        // body-r2: a body the server did not send is absent from the answer, and its case is kept for the log line
        let mut misses = self.body_misses.lock().expect("misses lock");
        for (uid, why) in fetched.missed { misses.insert((folder.to_string(), uid), why); }
        drop(misses);
        let mut gone = self.body_gone.lock().expect("gone lock");
        for uid in fetched.gone { gone.insert((folder.to_string(), uid)); }
        Ok(fetched.got)
    }

    /// body-r3: one numbered part of one message (`UID FETCH uid (UID BODY.PEEK[part])`, no \Seen), in its transfer encoding.
    async fn fetch_part(&self, folder: &str, uid: u32, part: &str) -> Result<Vec<u8>, CoreError> {
        let (f, p) = (folder.to_string(), part.to_string());
        let got = self.run(move |s| Box::pin(async move {
            select_once(s, &f).await?;
            checked::fetch_part(s, uid, &p).await
        })).await?;
        got.map_err(|why| self.err_words(format!("the server sent no part {part} of UID {uid}: {why}")))
    }

    async fn delete(&self, folder: &str, uids: &[u32]) -> Result<(), CoreError> {
        if uids.is_empty() { return Ok(()); }
        let (f, set) = (folder.to_string(), uid_set(uids));
        self.run(move |s| Box::pin(async move {
            if !s.caps.uidplus { return Err(words("the server has no UIDPLUS, so a delete would also remove other flagged messages")); }
            select(s, &f).await?;
            checked::run_to_end(s, &format!("UID STORE {set} +FLAGS.SILENT (\\Deleted)"), |_| {}).await?;
            checked::run_to_end(s, &format!("UID EXPUNGE {set}"), |_| {}).await?;
            Ok(())
        })).await
    }
}

impl ImapAdapter {
    /// The header block of one message (`UID FETCH uid BODY.PEEK[HEADER]`): what a reply needs from its parent.
    pub async fn message_headers(&self, folder: &str, uid: u32) -> Result<Vec<u8>, CoreError> {
        let f = folder.to_string();
        self.run(move |s| Box::pin(async move {
            select_once(s, &f).await?;
            let got = checked::fetch(s, &format!("UID FETCH {uid} (UID BODY.PEEK[HEADER])")).await?;
            let m = got.into_iter().find(|m| m.uid == Some(uid)).ok_or_else(|| words(format!("the server has no message with UID {uid}")))?;
            m.header.ok_or_else(|| words("the server sent no message headers"))
        })).await
    }

    /// A144: the threading headers (Message-ID, and the chain: References, else In-Reply-To) of several mails of one folder, by UID,
    /// without setting \Seen. One `UID FETCH <set> (UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO)])` per
    /// `IMAP_UID_SET_CHUNK` UIDs. A UID the server does not answer, or a mail with no Message-ID, is absent from the answer (never an error).
    pub async fn thread_headers_many(&self, folder: &str, uids: &[u32]) -> Result<Vec<(u32, send::ParentHeaders)>, CoreError> {
        if uids.is_empty() { return Ok(Vec::new()); }
        let f = folder.to_string();
        let mut asked: Vec<u32> = uids.to_vec();
        asked.sort_unstable();
        asked.dedup();
        self.run(move |s| Box::pin(async move {
            select_once(s, &f).await?;
            let mut out: Vec<(u32, send::ParentHeaders)> = Vec::new();
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            for chunk in asked.chunks(IMAP_UID_SET_CHUNK) {
                let got = checked::fetch(s, &format!("UID FETCH {} (UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO)])", uid_set(chunk))).await?;
                for m in got {
                    let (Some(uid), Some(block)) = (m.uid, m.header.as_deref()) else { continue };
                    if chunk.binary_search(&uid).is_err() || !seen.insert(uid) { continue; }
                    if let Ok(p) = crate::blocking::parent_headers_from(block) { out.push((uid, p)); }
                }
            }
            Ok(out)
        })).await
    }

    /// real-replyto: `thread_headers_many` plus each mail's Reply-To entries (name, address), from the SAME fetch (one more field name in the
    /// same `HEADER.FIELDS` list: `... IN-REPLY-TO REPLY-TO`), so the fill-in pass learns a stored mail's Reply-To with no extra round trip.
    /// A mail with no Message-ID is absent from the answer, as in `thread_headers_many`.
    pub async fn thread_headers_reply_to_many(&self, folder: &str, uids: &[u32]) -> Result<Vec<(u32, send::ParentHeaders, Vec<(String, String)>)>, CoreError> {
        if uids.is_empty() { return Ok(Vec::new()); }
        let f = folder.to_string();
        let mut asked: Vec<u32> = uids.to_vec();
        asked.sort_unstable();
        asked.dedup();
        self.run(move |s| Box::pin(async move {
            select_once(s, &f).await?;
            let mut out: Vec<(u32, send::ParentHeaders, Vec<(String, String)>)> = Vec::new();
            let mut seen: BTreeSet<u32> = BTreeSet::new();
            for chunk in asked.chunks(IMAP_UID_SET_CHUNK) {
                let got = checked::fetch(s, &format!("UID FETCH {} (UID BODY.PEEK[HEADER.FIELDS (MESSAGE-ID REFERENCES IN-REPLY-TO REPLY-TO)])", uid_set(chunk))).await?;
                for m in got {
                    let (Some(uid), Some(block)) = (m.uid, m.header.as_deref()) else { continue };
                    if chunk.binary_search(&uid).is_err() || !seen.insert(uid) { continue; }
                    if let Ok(p) = crate::blocking::parent_headers_from(block) { out.push((uid, p, crate::envelope::reply_to_named(block))); }
                }
            }
            Ok(out)
        })).await
    }

    pub(crate) fn caps_idle(sess: &Sess) -> bool { sess.caps.idle }
    pub(crate) fn idle_slots(&self) -> Arc<Semaphore> { self.idle_slots.clone() }
    /// The SELECT before IDLE. A lost connection is retryable like any other call's (the supervisor reconnects); it was
    /// an `Unexpected` error once, which ended push for good on the first BYE.
    pub(crate) async fn select_for_idle(&self, sess: &mut Sess, folder: &str) -> Result<(), CoreError> {
        select(sess, folder).await.map(|_| ()).map_err(|e| self.core_err(e))
    }
    pub(crate) fn err_words(&self, w: impl Into<String>) -> CoreError { self.core_err(words(w)) }
    pub(crate) fn err_imap(&self, e: ImapError) -> CoreError { self.core_err(E::Imap(e)) }
    pub(crate) fn err_lost(&self, bye: Option<&str>) -> CoreError { self.core_err(checked::lost(bye)) }
}
