//! fix-realcore round 15 (owner, main115: a reply failed on "Connection reset by peer", attempts 1, never tried again). Through the real
//! core's commands, its outbox timer (OUTBOX_TICK_MS) and the real sender, against the in-process IMAP fake and the staged SMTP server
//! (send/tests/staged/mod.rs; 127.0.0.1 only): `outbox/<id>/send_state` says `retrying` with the step and the next try after a reset,
//! then `sent` with attempts 2 and one mail at the server, by itself; a break after the body says `failed` with no next try.
//! It waits one real SEND_RETRY_START_S (30 s) pause; the run is under a time limit (WAIT).
//! Broken copies (one edit each, red first):
//!   E1 src/rust_core.rs: the `["outbox", id, "send_state"]` arm deleted          -> both tests (send_state not served)
//!   E2 src/outbox_tick.rs `send_state`: `if view.retry_at_ms.is_some() { "retrying" }` -> `if false { "retrying" }`
//!                                                                                -> a_reset_is_shown_retrying_then_sent_by_itself
//!   E4 src/rust_core.rs: the `(path == "outbox/open")` line deleted              -> a_reset_is_shown_retrying_then_sent_by_itself
//!   E5 src/outbox_tick.rs: `Arc::new(send::LoggedDelivery { log: smtp_log(&store) })` -> `Arc::new(send::LettreDelivery)` (round 15b: no step lines)
//!                                                                                -> a_reset_is_shown_retrying_then_sent_by_itself
//!   E3 src/outbox_tick.rs `send_state`: `.and_then(|c| c.strip_prefix(send::smtp::NETWORK_CODE_PREFIX))` -> `.and_then(|_| None::<&str>)`
//!                                                                                -> both tests (no step)
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[path = "../../send/tests/staged/mod.rs"]
mod staged;

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use staged::{BreakAt, How, StagedSmtp};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(contracts::constants::SEND_RETRY_START_S + 30);

fn wait_for<T: std::fmt::Debug>(what: &str, mut read: impl FnMut() -> T, ok: impl Fn(&T) -> bool) -> T {
    let t0 = Instant::now();
    loop {
        let v = read();
        if ok(&v) { return v; }
        if t0.elapsed() >= WAIT { panic!("never reached within {WAIT:?}: {what}; last read: {v:?}"); }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn state(core: &RustCore, entry: &str) -> serde_json::Value {
    core.state(format!("outbox/{entry}/send_state")).map(|s| serde_json::from_str(&s).expect("send_state is JSON")).unwrap_or(serde_json::Value::Null)
}
/// A core with one account on the IMAP fake and the staged SMTP server, and one message sent at once (SendNow).
fn sent_once(smtp: &StagedSmtp) -> (RustCore, String, tempfile::TempDir, FakeImap, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    let db = tempfile::tempdir().unwrap();
    let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let det: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    let core = RustCore::with_parts(Store::open(&db.path().join("whoosh.sqlite")).unwrap(), keys, det, TestTransport::PlainLoopback);
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } })
        .expect("added").created_id.unwrap();
    wait_for("listening", || core.sync_phase(&id), |p| *p == Some(SyncPhase::Listening));
    let draft = core.run(Command::SaveDraft { from_account_id: None, to: vec!["friend@example.test".into()], cc: vec![], subject: "Re: the quote".into(), body: "Yes, that works.".into(), reply_to_thread_id: None })
        .expect("saved").created_id.unwrap();
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    let _ = core.run(Command::SendNow { outbox_id: entry.clone() });   // its failure is the entry's state, read below
    (core, entry, db, imap, rt)
}

#[test]
fn a_reset_is_shown_retrying_then_sent_by_itself() {
    let smtp = StagedSmtp::start(BreakAt::MailFrom, How::Reset, 1);
    let t0 = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let (core, entry, db, _imap, _rt) = sent_once(&smtp);
    let first = wait_for("retrying after the reset", || state(&core, &entry), |v| v["state"] == "retrying");
    println!("after the reset: {first}");
    let open: serde_json::Value = serde_json::from_str(&core.state("outbox/open".into()).expect("outbox/open is served")).unwrap();
    println!("outbox/open: {open}");
    let row = open.as_array().and_then(|a| a.iter().find(|r| r["id"] == entry.as_str())).expect("the retrying entry is listed open");
    assert_eq!(row["send_state"]["state"], "retrying");
    assert_eq!(row["subject"], "Re: the quote");
    assert_eq!(first["attempts"], 1);
    assert_eq!(first["stage"], "mail_from");
    assert!(first["words"].as_str().unwrap_or("").contains("broke at MAIL FROM"), "{first}");
    let next = first["next_try_ms"].as_i64().expect("a next try time");
    let pause = contracts::constants::SEND_RETRY_START_S as i64 * 1000;
    assert!(next >= t0 + pause && next <= t0 + pause + 20_000, "the next try is SEND_RETRY_START_S after the first: {next} vs {t0}");
    let done = wait_for("sent by itself", || state(&core, &entry), |v| v["state"] == "sent");
    println!("then: {done}");
    assert_eq!(done["attempts"], 2);
    assert!(done["sent_at_ms"].as_i64().is_some_and(|t| t >= next), "{done}");
    assert_eq!(smtp.received().len(), 1, "one mail at the server");
    assert_eq!(smtp.mail_sessions(), 2, "two tries reached MAIL FROM (the account check before them never does)");
    // round 15b: core.log holds each try's steps (send/src/smtp.rs LoggedDelivery), the break at MAIL FROM, then the server's 250 after the body
    let log = std::fs::read_to_string(db.path().join("core.log")).unwrap_or_default();
    let smtp_lines: Vec<&str> = log.lines().filter(|l| l.contains(" smtp ")).collect();
    println!("core.log smtp lines:\n{}", smtp_lines.join("\n"));
    assert!(smtp_lines.iter().any(|l| l.contains("mail_from: broke: network error: Connection reset by peer")), "the break is logged with its step");
    assert!(smtp_lines.iter().any(|l| l.contains("after_data: 250")), "the second try's answer after the body is logged");
    assert_eq!(smtp_lines.iter().filter(|l| l.contains(": try: ")).count(), 2, "one try line per try");
    assert!(!log.contains("Yes, that works.") && !log.contains(PW), "never the message or the secret");
    println!("PASS engine reset-then-sent: attempts 2, one mail, 2 sending connections, {} in all", smtp.connections.load(Ordering::SeqCst));
}

#[test]
fn a_break_after_the_body_is_shown_failed_with_its_step_and_no_next_try() {
    let smtp = StagedSmtp::start(BreakAt::AfterData, How::Reset, 1000);
    let (core, entry, _db, _imap, _rt) = sent_once(&smtp);
    let v = wait_for("failed after the body", || state(&core, &entry), |v| v["state"] == "failed");
    println!("after the body: {v}");
    assert_eq!(v["stage"], "after_data");
    assert_eq!(v["attempts"], 1);
    assert!(v["next_try_ms"].is_null(), "{v}");
    assert!(v["words"].as_str().unwrap_or("").contains("the server may have it"), "{v}");
    std::thread::sleep(Duration::from_millis(3 * contracts::constants::OUTBOX_TICK_MS));
    assert_eq!(smtp.mail_sessions(), 1, "never tried again by itself");
    assert_eq!(smtp.received().len(), 1);
    assert!(core.state("outbox/no-such-entry/send_state".into()).is_none(), "an unknown id serves nothing");
}
