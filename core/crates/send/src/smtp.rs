//! The lettre adapter. One connection per call; the server's real answer goes into the words.
//! fix-realcore round 15 (owner, main115: a reply failed with "network error: Connection reset by peer (os error 54)" and no stage): the
//! connection is driven step by step (lettre's own `SmtpConnection`, the same steps its transport takes), so a failure names the step it broke
//! at, in the words ("at STARTTLS: ...") and in the code (`network_<stage>` for a dropped or reset connection, the server's own code otherwise).
use crate::account::Security;
use crate::ports::{DeliverPort, DeliverRequest};
use contracts::{ActionFailed, CoreError};
use lettre::address::Address;
use lettre::transport::smtp::authentication::{Credentials, Mechanism, DEFAULT_MECHANISMS};
use lettre::transport::smtp::client::{SmtpConnection, TlsParameters};
use lettre::transport::smtp::commands::{Data, Mail, Rcpt};
use lettre::transport::smtp::extension::{ClientId, Extension, MailBodyParameter, MailParameter};
use lettre::address::Envelope as LettreEnvelope;
use std::net::IpAddr;
use std::time::Duration;

#[derive(Default)]
pub struct LettreDelivery;

fn invalid(field: &str, words: String) -> CoreError {
    CoreError::Invalid { field: field.to_string(), reason_words: words }
}

fn is_loopback(host: &str) -> bool {
    host == "localhost" || host.parse::<IpAddr>().map(|ip| ip.is_loopback()).unwrap_or(false)
}

fn failed_with_code(account_name: &str, words: String, retryable: bool, code: Option<String>) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: account_name.to_string(), reason_words: words, retryable, code } }
}

/// The server's reply text lettre keeps as the error's source ("4.7.1 Try again later"); empty when it kept none.
fn server_text(e: &lettre::transport::smtp::Error) -> String {
    std::error::Error::source(e).map(|s| s.to_string()).unwrap_or_default()
}

fn parse_address(field: &str, raw: &str) -> Result<Address, CoreError> {
    raw.parse::<Address>().map_err(|e| invalid(field, format!("'{raw}' is not an address: {e}")))
}

/// How long one read or write on the connection may wait: lettre's own default for its transport (`DEFAULT_TIMEOUT`, private there), kept.
const SMTP_TIMEOUT: Duration = Duration::from_secs(60);

/// The steps of one send, in order. `AfterData` is the one step after which the server may already hold the message: the whole body and its
/// final dot were written, and only the answer is missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage { Connect, StartTls, Auth, MailFrom, RcptTo, Data, AfterData }

impl Stage {
    /// The identifier in the failure code (`network_<id>`), never read by the owner.
    pub fn id(self) -> &'static str {
        match self { Stage::Connect => "connect", Stage::StartTls => "starttls", Stage::Auth => "auth", Stage::MailFrom => "mail_from", Stage::RcptTo => "rcpt_to", Stage::Data => "data", Stage::AfterData => "after_data" }
    }
    /// The step in the owner's words.
    fn words(self) -> &'static str {
        match self {
            Stage::Connect => "the greeting", Stage::StartTls => "STARTTLS", Stage::Auth => "AUTH", Stage::MailFrom => "MAIL FROM", Stage::RcptTo => "RCPT TO",
            Stage::Data => "DATA", Stage::AfterData => "the end of the message (the server may have it)",
        }
    }
}

/// The code prefix of a connection that broke (reset, closed, timed out, TLS) at a step: `network_<stage id>`.
pub const NETWORK_CODE_PREFIX: &str = "network_";

/// What lettre says when the server closed the connection before its answer was complete (connection.rs `read_response`: the stream ended).
const CLOSED_MID_ANSWER: &str = "incomplete response";

/// A connection that broke, as opposed to the server answering with a code (4xx, 5xx) or the client refusing to go on: a reset or an
/// I/O failure (lettre's Network, Connection), TLS, a time-out, and a server that closed the stream before it answered (lettre files that
/// one as a Response error with the words above, and no code).
fn is_network(e: &lettre::transport::smtp::Error) -> bool {
    if e.is_transient() || e.is_permanent() || e.is_client() { return false; }
    !e.is_response() || e.to_string().contains(CLOSED_MID_ANSWER)
}

fn stage_failure(account_name: &str, stage: Stage, e: &lettre::transport::smtp::Error) -> CoreError {
    // The real server text, never the bare status (R#20), and the step it broke at.
    if is_network(e) {
        let words = format!("the connection to the mail server broke at {}: {e}", stage.words());
        // a reset after the body was written is NOT retried: the server may have delivered it (outbox deliver_entry)
        return failed_with_code(account_name, words, stage != Stage::AfterData, Some(format!("{NETWORK_CODE_PREFIX}{}", stage.id())));
    }
    let words = format!("the mail server did not accept the message at {}: {e}", stage.words());
    // delta 12: the reply code and the server's enhanced status beside the words ("451 4.7.1")
    let code = e.status().and_then(|c| contracts::smtp_failure_code(&c.to_string(), &server_text(e)));
    failed_with_code(account_name, words, !e.is_permanent(), code)
}

/// What a bearer token says it may do, for the step log: who it is for (its audience) and its permission names. Never the token, never a
/// person: only the `aud` and `scp` fields of its readable middle part, the scopes cut to their last word (`SMTP.Send`). Lead, 13 Nov (owner
/// picture 208: Microsoft 365 answered 451 at AUTH): whether the token carries the sending permission must be a fact in the log.
fn token_facts(token: &str) -> String {
    use base64::Engine;
    let Some(payload) = token.split('.').nth(1) else { return "not a readable token".to_string() };
    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')) else { return "not a readable token".to_string() };
    let text = String::from_utf8_lossy(&bytes);
    let field = |k: &str| -> Option<String> {
        let start = text.find(&format!("\"{k}\":\""))? + k.len() + 4;
        let rest = text.get(start..)?;
        Some(rest.get(..rest.find('"')?)?.to_string())
    };
    let scopes = field("scp").map(|s| s.split(' ').map(|x| x.rsplit('/').next().unwrap_or(x)).collect::<Vec<_>>().join(" ")).unwrap_or_else(|| "none named".to_string());
    format!("token for {}, scopes: {scopes}", field("aud").unwrap_or_else(|| "no audience named".to_string()))
}

impl DeliverPort for LettreDelivery {
    fn execute(&self, request: &DeliverRequest) -> Result<(), CoreError> {
        deliver(request, &|_| {})
    }
}

/// fix-realcore round 15b (lead: "core.log has no line for this send at all"): where each try's step lines go (the engine writes them to core.log).
pub type StepLog = std::sync::Arc<dyn Fn(&str) + Send + Sync>;

/// The same delivery as `LettreDelivery`, with one line per step of every try: the step, then the server's code and first answer line, or how
/// the connection broke. Codes and server words only: never the secret or the token (only its length), never an address or a line of the message.
pub struct LoggedDelivery {
    pub log: StepLog,
}

impl DeliverPort for LoggedDelivery {
    fn execute(&self, request: &DeliverRequest) -> Result<(), CoreError> {
        deliver(request, &|line| (self.log)(line))
    }
}

/// At most this many characters of a server answer go into a step line (an answer line is short; a longer one is cut, never the code).
const ANSWER_CHARS_MAX: usize = 160;

fn answer(r: &lettre::transport::smtp::response::Response) -> String {
    let first: String = r.first_line().unwrap_or("").chars().take(ANSWER_CHARS_MAX).collect();
    format!("{} {first}", r.code())
}

fn broke(stage: Stage, e: &lettre::transport::smtp::Error) -> String {
    let how = if is_network(e) { "broke" } else { "refused" };
    let words: String = e.to_string().chars().take(ANSWER_CHARS_MAX * 2).collect();
    format!("{}: {how}: {words}", stage.id())
}

fn deliver(request: &DeliverRequest, log: &dyn Fn(&str)) -> Result<(), CoreError> {
    let smtp = &request.smtp;
    netguard::outbound_allowed(&smtp.host)?;   // engine-act-r6 round 5: a rig copy sends to this Mac only, its name never resolved
    if smtp.security == Security::Plaintext && !is_loopback(&smtp.host) {
        return Err(invalid("smtp.security", format!("plain SMTP is refused for {}: only the loopback address may skip TLS", smtp.host)));
    }
    let tls = match smtp.security {
        Security::Plaintext => None,
        Security::Implicit | Security::StartTls => Some(TlsParameters::new(smtp.host.clone()).map_err(|e| invalid("smtp.host", e.to_string()))?),
    };
    let from = parse_address("envelope.from", &request.envelope.from)?;
    let to = request.envelope.to.iter().map(|a| parse_address("envelope.to", a)).collect::<Result<Vec<_>, _>>()?;
    let envelope = LettreEnvelope::new(Some(from), to).map_err(|e| invalid("envelope", e.to_string()))?;
    // The name given at EHLO: the neutral address literal `[127.0.0.1]`, never this Mac's host name. The host name holds the owner's
    // name and the server writes it into the Received header every recipient can read. A submission server accepts a literal (RFC 5321 4.1.3).
    let hello = ClientId::Ipv4(std::net::Ipv4Addr::LOCALHOST);
    let name = request.account_name.as_str();
    let auth = match &smtp.credentials {
        Some(c) if c.bearer => format!("XOAUTH2, token {} characters, {}", c.secret.chars().count(), token_facts(&c.secret)),
        Some(_) => "PLAIN or LOGIN".to_string(),
        None => "none".to_string(),
    };
    let line = |s: String| log(&format!("smtp {name} {}:{}: {s}", smtp.host, smtp.port));
    line(format!("try: {:?}, auth {auth}, EHLO name {hello}, {} recipient(s), {} bytes", smtp.security, envelope.to().len(), request.rfc822.len()));
    let fail = |conn: &mut SmtpConnection, stage: Stage, e: lettre::transport::smtp::Error| {
        line(broke(stage, &e));
        conn.abort();
        stage_failure(name, stage, &e)
    };

    // 1. the TCP (or TLS, port 465) connection, the 220 greeting (lettre reads it before it writes) and EHLO. Each address of the name in turn,
    //    as lettre does, but one by one so the line says which address (IPv4 or IPv6) answered or broke.
    let wrapper = if smtp.security == Security::Implicit { tls.as_ref() } else { None };
    let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(&(smtp.host.as_str(), smtp.port))
        .map_err(|e| { line(format!("connect: the name was not resolved: {e}")); failed_with_code(name, format!("the mail server {} was not found: {e}", smtp.host), true, Some(format!("{NETWORK_CODE_PREFIX}{}", Stage::Connect.id()))) })?
        .collect();
    let mut last = None;
    let mut found = None;
    for addr in &addrs {
        match SmtpConnection::connect::<std::net::SocketAddr>(*addr, Some(SMTP_TIMEOUT), &hello, wrapper, None) {
            Ok(c) => { line(format!("connect {addr}: greeting read, EHLO answered: {}", c.server_info())); found = Some(c); break; }
            Err(e) => { line(format!("connect {addr}: {}", broke(Stage::Connect, &e))); last = Some(e); }
        }
    }
    let mut conn = match (found, last) {
        (Some(c), _) => c,
        (None, Some(e)) => return Err(stage_failure(name, Stage::Connect, &e)),
        (None, None) => { line("connect: the name has no address".into()); return Err(failed_with_code(name, format!("the mail server {} has no address", smtp.host), true, Some(format!("{NETWORK_CODE_PREFIX}{}", Stage::Connect.id())))); }
    };
    // 2. STARTTLS on 587, sent as a command, then EHLO again (lettre's `starttls` does both)
    if smtp.security == Security::StartTls {
        let params = tls.as_ref().expect("STARTTLS has its parameters");
        if !conn.can_starttls() {
            line("starttls: not offered".into());
            conn.abort();
            return Err(failed_with_code(name, format!("the mail server did not accept the message at STARTTLS: {} does not offer STARTTLS on port {}", smtp.host, smtp.port), false, None));
        }
        conn.starttls(params, &hello).map_err(|e| fail(&mut conn, Stage::StartTls, e))?;
        line(format!("starttls: TLS up, EHLO answered: {}", conn.server_info()));
    }
    // 3. AUTH: an OAuth2 token by XOAUTH2 only (delta 14), one line `AUTH XOAUTH2 <base64 of user=<smtp user name>^Aauth=Bearer <token>^A^A>`;
    //    a password by lettre's PLAIN and LOGIN
    if let Some(c) = &smtp.credentials {
        let mechanisms: &[Mechanism] = if c.bearer { &[Mechanism::Xoauth2] } else { DEFAULT_MECHANISMS };
        let r = conn.auth(mechanisms, &Credentials::new(c.username.clone(), c.secret.clone())).map_err(|e| fail(&mut conn, Stage::Auth, e))?;
        line(format!("auth: {}", answer(&r)));
    }
    // 4. MAIL FROM, RCPT TO, DATA, the body: the steps of lettre's `SmtpConnection::send`, one by one, each answer awaited (no PIPELINING, no BDAT)
    let mut options = vec![];
    if envelope.from().into_iter().chain(envelope.to()).any(|a| !AsRef::<str>::as_ref(a).is_ascii()) {
        if !conn.server_info().supports_feature(Extension::SmtpUtfEight) {
            line("mail_from: an address is not plain ASCII and SMTPUTF8 is not offered".into());
            conn.abort();
            return Err(failed_with_code(name, "the mail server did not accept the message at MAIL FROM: an address is not plain ASCII and the server does not take SMTPUTF8".into(), false, None));
        }
        options.push(MailParameter::SmtpUtfEight);
    }
    if !request.rfc822.is_ascii() {
        if !conn.server_info().supports_feature(Extension::EightBitMime) {
            line("mail_from: the message is not plain ASCII and 8BITMIME is not offered".into());
            conn.abort();
            return Err(failed_with_code(name, "the mail server did not accept the message at MAIL FROM: the message is not plain ASCII and the server does not take 8BITMIME".into(), false, None));
        }
        options.push(MailParameter::Body(MailBodyParameter::EightBitMime));
    }
    let r = conn.command(Mail::new(envelope.from().cloned(), options)).map_err(|e| fail(&mut conn, Stage::MailFrom, e))?;
    line(format!("mail_from: {}", answer(&r)));
    for rcpt in envelope.to() {
        let r = conn.command(Rcpt::new(rcpt.clone(), vec![])).map_err(|e| fail(&mut conn, Stage::RcptTo, e))?;
        line(format!("rcpt_to: {}", answer(&r)));
    }
    let r = conn.command(Data).map_err(|e| fail(&mut conn, Stage::Data, e))?;
    line(format!("data: {}", answer(&r)));
    let r = conn.message(&request.rfc822).map_err(|e| fail(&mut conn, Stage::AfterData, e))?;
    line(format!("after_data: {}", answer(&r)));
    let _ = conn.quit();   // the message is accepted; a failed goodbye changes nothing
    Ok(())
}
