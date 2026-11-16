//! The outbound guard of a hidden test copy. The owner's rule: "nothing leaves this Mac except the relay". A rig core (the test copy
//! `RustCore::open_production(.., rig: true)` opens) closes this process to every host but this Mac's own, ONCE, at open
//! (`close_to_this_mac`); nothing reopens it, and no command, state path or environment variable reaches the setter. Every dialer and
//! resolver of the core (IMAP, SMTP, the account detector's HTTP, DNS and TLS probe, the sign-in authority, the one-click unsubscribe)
//! calls `outbound_allowed(host)` BEFORE it resolves a name: a DNS query is traffic too. This Mac's own address (127.0.0.0/8, ::1, the
//! name localhost) is always allowed, so the rig's loopback redirections keep working. A refusal is counted with its host and served by
//! the rig core on the state path `rig/refused-outbound` (`refused_json`).
//!
//! Lowest crate the dialers share is `contracts`, which this lane may not edit: this crate is the guard's home until the contract lane
//! takes it (the lines are named in docs/lanes/engine-act-r6.md, round 5).
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use contracts::{ActionFailed, CoreError};

/// The refusal's code beside its words (`ActionFailed.code`).
pub const OUTBOUND_REFUSED_CODE: &str = "rig_outbound_refused";
/// The state path the rig core serves the refusals on.
pub const REFUSED_STATE_PATH: &str = "rig/refused-outbound";

static CLOSED: AtomicBool = AtomicBool::new(false);
static REFUSED: AtomicU64 = AtomicU64::new(0);

fn last() -> &'static Mutex<Option<String>> {
    static LAST: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    LAST.get_or_init(Default::default)
}

/// The rig core's one call, at open: from now on this process reaches this Mac only. There is no way back.
pub fn close_to_this_mac() { CLOSED.store(true, Ordering::SeqCst); }

/// True once a rig core closed this process.
pub fn is_closed() -> bool { CLOSED.load(Ordering::SeqCst) }

/// This Mac's own address: 127.0.0.0/8, ::1 (bracketed or not), or the name localhost (any case, a trailing dot allowed).
pub fn is_this_mac(host: &str) -> bool {
    let h = host.trim().trim_start_matches('[').trim_end_matches(']').trim_end_matches('.').to_ascii_lowercase();
    h == "localhost" || h.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// THE guard. Ok for this Mac's own address, and for every host while no rig core opened in this process. Else the attempt is counted
/// (`refused_count`, `last_refused`) and refused in words; the caller opens nothing and resolves nothing.
pub fn outbound_allowed(host: &str) -> Result<(), CoreError> {
    if !is_closed() || is_this_mac(host) { return Ok(()); }
    REFUSED.fetch_add(1, Ordering::SeqCst);
    *last().lock().unwrap_or_else(|e| e.into_inner()) = Some(host.to_string());
    Err(CoreError::Action { failure: ActionFailed {
        account_name: String::new(),
        reason_words: format!("{host} is not this Mac: the test copy reaches this Mac only (\"nothing leaves this Mac except the relay\"), so it was not asked"),
        retryable: false,
        code: Some(OUTBOUND_REFUSED_CODE.to_string()),
    } })
}

/// The host a URL names to the HTTP client, as `url::Url::parse` (the parser reqwest uses) reads it: `https://h:443/p` -> `h`,
/// `http://[::1]:8/` -> `[::1]`, `http://0x7f.0.0.1/` -> `127.0.0.1`, `https://e.example\@127.0.0.1/` -> `e.example` (a backslash ends
/// the authority of http and https). "" when that parser refuses the URL or names no host: the guard refuses "" on a rig, so a URL the
/// client could not name is never allowed. rig-net-rust (audit r77, gap 4): the one host parser of the guard and of its callers.
pub fn host_of_url(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
}

/// `outbound_allowed` on the host the client dials (`host_of_url`): on a rig, a URL that parser cannot name is refused.
pub fn outbound_allowed_url(url: &str) -> Result<(), CoreError> { outbound_allowed(&host_of_url(url)) }

/// How many outbound attempts this process refused.
pub fn refused_count() -> u64 { REFUSED.load(Ordering::SeqCst) }

/// The host of the last refused attempt.
pub fn last_refused() -> Option<String> { last().lock().unwrap_or_else(|e| e.into_inner()).clone() }

/// `rig/refused-outbound`: `{"count":n,"last_host":"h"|null}`.
pub fn refused_json() -> String {
    let host = last_refused().map(|h| {
        let mut o = String::from("\"");
        for c in h.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
        o.push('"');
        o
    }).unwrap_or_else(|| "null".into());
    format!("{{\"count\":{},\"last_host\":{host}}}", refused_count())
}

/// The name lookup the guarded dialers use when they resolve themselves (the IMAP dialer, the TLS probe). Production: the system
/// resolver. Tests replace it (`set_resolver`) with one that counts and answers 127.0.0.1, so a test can see that the resolver was
/// never asked, and a guard removed by a mutant reaches a local listener instead of the network.
pub trait Resolve: Send + Sync {
    fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>>;
}

fn resolver() -> &'static Mutex<Option<Arc<dyn Resolve>>> {
    static R: OnceLock<Mutex<Option<Arc<dyn Resolve>>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Tests only: every guarded lookup of this process goes through `r`.
pub fn set_resolver(r: Arc<dyn Resolve>) { *resolver().lock().unwrap_or_else(|e| e.into_inner()) = Some(r); }

/// The replaced resolver, when a test set one.
pub fn test_resolver() -> Option<Arc<dyn Resolve>> { resolver().lock().unwrap_or_else(|e| e.into_inner()).clone() }

/// A blocking lookup: the test resolver when one is set, else the system's. Call `outbound_allowed` first.
pub fn lookup(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    match test_resolver() {
        Some(r) => r.lookup(host, port),
        None => Ok((host, port).to_socket_addrs()?.collect()),
    }
}
