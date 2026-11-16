//! gap-engine-tests (item f4e1fbe8): the background download of every body, judged ON THE WIRE of the in-process IMAP fake with the REAL
//! adapter (engine_sync_full_r1.rs judges the crawl over an in-process FakeSource, which cannot see a socket). INBOX holds 40 mails, Archive
//! 30, Sent 20; the account is added, the crawl runs, and the fake's command log and its counter of simultaneous sessions say:
//!   every-body-of-every-folder-is-stored   90 rows hold a body in the store
//!   inbox-first-newest-first               the first body FETCH is on INBOX, and the highest UID of each INBOX body FETCH falls call by call
//!   fetches-come-in-batches                no body FETCH asks for more than BODY_PREFETCH_BATCH UIDs
//!   one-connection                         at most 2 sessions were ever open at once (the sync's own IDLE session, and one for the bodies)
//! One line per case: `PASS <case>: ...` (run with `-- --nocapture`). The case names are the ones the hidden `--slice-crawl` of
//! checks/realcore-check.py will print on a built bundle; this file is the in-process half of them.
//! RED-FIRST broken copies (one edit each, the lead runs them):
//!   fetches-come-in-batches: core/crates/engine/src/crawl.rs `b.len() >= BODY_PREFETCH_BATCH as usize` -> `b.len() >= 2 * BODY_PREFETCH_BATCH as usize`
//!   every-body-of-every-folder-is-stored: core/crates/engine/src/crawl.rs `if self.crawl.loaded.swap(true, Ordering::SeqCst) { return; }` ->
//!       `if self.crawl.loaded.swap(true, Ordering::SeqCst) || true { return; }` (the crawl never runs: fewer than 90 bodies are stored)
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::constants::BODY_PREFETCH_BATCH;
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(90);
const MAILS: usize = 90;

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// The UIDs a command's set names ("7:10,3,1:2" -> 7 8 9 10 3 1 2).
fn uids_of(set: &str) -> Vec<u32> {
    set.split(',').flat_map(|r| match r.split_once(':') { Some((a, b)) => (a.parse().unwrap()..=b.parse().unwrap()).collect::<Vec<u32>>(), None => vec![r.parse().unwrap()] }).collect()
}

/// The body FETCHes of the fake's log in order: (connection, folder selected on that connection at that moment, the UIDs asked).
fn body_fetches(log: &[String]) -> Vec<(String, String, Vec<u32>)> {
    let mut selected: HashMap<String, String> = HashMap::new();
    let mut out = Vec::new();
    for line in log {
        let Some((conn, rest)) = line.split_once(' ') else { continue };
        let mut words = rest.split_whitespace();
        match words.next().map(str::to_ascii_uppercase).as_deref() {
            Some("SELECT") => { selected.insert(conn.to_string(), words.next().unwrap_or("").trim_matches('"').to_string()); }
            Some("UID") if words.next().is_some_and(|w| w.eq_ignore_ascii_case("FETCH")) && rest.contains("BODY.PEEK[]") => {
                let set = rest.split_whitespace().nth(2).unwrap_or("");
                out.push((conn.to_string(), selected.get(conn).cloned().unwrap_or_default(), uids_of(set)));
            }
            _ => {}
        }
    }
    out
}

/// Each connection of the fake's log, in the order it first spoke: its first and last line number and the kinds of its commands (a SELECT
/// with its folder, a UID command with what it reads; any other command by its first word only, so a LOGIN prints no password). Printed
/// only when the session count is over: it names which work held the extra session and when (reds-search, round 4).
fn sessions(log: &[String]) -> String {
    let mut order: Vec<String> = Vec::new();
    let mut seen: HashMap<String, (usize, usize, Vec<String>)> = HashMap::new();
    for (n, line) in log.iter().enumerate() {
        let Some((conn, rest)) = line.split_once(' ') else { continue };
        let words: Vec<&str> = rest.split_whitespace().collect();
        let Some(first) = words.first().map(|w| w.to_ascii_uppercase()) else { continue };
        let kind = match first.as_str() {
            "UID" => {
                let what = if rest.contains("BODY.PEEK[]") { " body" } else if rest.to_ascii_uppercase().contains("HEADER") { " headers" } else { "" };
                format!("UID {}{what}", words.get(1).copied().unwrap_or("").to_ascii_uppercase())
            }
            "SELECT" | "EXAMINE" => format!("{first} {}", words.get(1).copied().unwrap_or("")),
            _ => first.clone(),
        };
        let e = seen.entry(conn.to_string()).or_insert_with(|| { order.push(conn.to_string()); (n, n, Vec::new()) });
        e.1 = n;
        if !e.2.contains(&kind) { e.2.push(kind); }
    }
    order.iter().map(|c| { let (a, b, k) = &seen[c]; format!("{c} lines {a}-{b} of {}: {}", log.len(), k.join(", ")) }).collect::<Vec<_>>().join(" | ")
}

#[test]
fn the_crawl_over_the_wire_stores_every_body_inbox_first_in_batches_on_one_connection() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 40);
    imap.seed("Archive", 30);
    imap.seed("Sent", 20);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let store = core.store();
    let with_body = || -> usize {
        store.write(|ctx| {
            let n: i64 = ctx.tx.query_row("SELECT count(*) FROM message WHERE body_text IS NOT NULL", [], |r| r.get(0))?;
            Ok(n as usize)
        }).unwrap_or(0)
    };
    wait_until(&format!("{MAILS} bodies are stored"), || with_body() == MAILS);
    assert!(core.background().settle_crawl(WAIT), "the crawl ended");
    println!("PASS every-body-of-every-folder-is-stored: {} of {MAILS} rows hold a body", with_body());
    assert_eq!(with_body(), MAILS);

    let log = imap.log();
    let fetches = body_fetches(&log);
    assert!(!fetches.is_empty(), "the log holds body fetches: {} lines", log.len());
    assert_eq!(fetches[0].1, "INBOX", "the first body fetch is on INBOX: {:?}", fetches.iter().take(3).collect::<Vec<_>>());
    let inbox_tops: Vec<u32> = fetches.iter().filter(|f| f.1 == "INBOX").map(|f| *f.2.iter().max().unwrap()).collect();
    assert!(inbox_tops.windows(2).all(|w| w[0] > w[1]), "each INBOX fetch is below the last (newest first): {inbox_tops:?}");
    let first_other = fetches.iter().position(|f| f.1 != "INBOX");
    let last_inbox = fetches.iter().rposition(|f| f.1 == "INBOX").unwrap();
    assert!(first_other.is_none_or(|i| i > last_inbox), "INBOX is finished before another folder's bodies are asked for: {:?}", fetches.iter().map(|f| f.1.as_str()).collect::<Vec<_>>());
    println!("PASS inbox-first-newest-first: {} INBOX fetches, highest UIDs {inbox_tops:?}, then the other folders", inbox_tops.len());

    let biggest = fetches.iter().map(|f| f.2.len()).max().unwrap();
    assert!(biggest <= BODY_PREFETCH_BATCH as usize, "a body fetch asked for {biggest} UIDs, over {BODY_PREFETCH_BATCH}");
    println!("PASS fetches-come-in-batches: {} fetches, the largest asked for {biggest} UIDs (limit {BODY_PREFETCH_BATCH})", fetches.len());

    let most = imap.with(|s| s.max_open);
    assert!(most <= 2, "{most} sessions were open at once, over the sync's own IDLE session plus one; each connection of the log: {}", sessions(&log));
    println!("PASS one-connection: at most {most} sessions open at once (limit 2: the IDLE session and one more)");
}
