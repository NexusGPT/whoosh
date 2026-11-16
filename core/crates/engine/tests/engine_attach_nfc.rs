//! fix-realcore round 7 (main100 `--slice-attachsend`, every red on the accented file): a file of this Mac whose name is decomposed (macOS: a letter,
//! then U+0301) is kept and sent under its composed name (NFC): `draft/<id>/attachments` lists it composed, and the message SMTP gets names it composed
//! (RFC 2231 `%C3%A9`, never the decomposed `e%CC%81`), so other mail programs show the accents in place. The bytes go as they are.
//! Broken copies (one edit each, red first):
//!   draft_files.rs `add`: `let file_name = parse::nfc(&file_name);` -> `let file_name = file_name;` (the row keeps the decomposed name: red)
//!   send/src/attachments.rs `part`: `let name = parse::nfc(&file.file_name);` -> `let name = file.file_name.clone();` together with the first
//!       (the wire holds e%CC%81: red)
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../send/tests/support/mod.rs"]
mod smtp_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use smtp_fakes::{FakeSmtp, Mode};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(40);
const COMPOSED: &str = "Offre T\u{e9}l\u{e9}fonica \u{e9}t\u{e9} 2026.pdf";
const DECOMPOSED: &str = "Offre Te\u{301}le\u{301}fonica e\u{301}te\u{301} 2026.pdf";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn wait_for<T: std::fmt::Debug>(what: &str, mut read: impl FnMut() -> T, ok: impl Fn(&T) -> bool) -> T {
    let t0 = Instant::now();
    loop {
        let v = read();
        if ok(&v) { return v; }
        if t0.elapsed() >= WAIT { panic!("never reached within {WAIT:?}: {what}; last read: {v:?}"); }
        std::thread::sleep(Duration::from_millis(40));
    }
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

#[test]
fn a_file_named_decomposed_on_this_mac_is_listed_and_sent_composed() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    let smtp = FakeSmtp::start(Mode::Xoauth2);
    let (db, files) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let core = RustCore::with_parts(Store::open(&db.path().join("whoosh.sqlite")).unwrap(), Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } })
        .expect("added").created_id.unwrap();
    wait_for("listening", || core.sync_phase(&id), |p| *p == Some(SyncPhase::Listening));
    let bytes = b"%PDF-1.4\n% offer\nendobj\n".to_vec();
    let path = files.path().join(DECOMPOSED);
    std::fs::write(&path, &bytes).unwrap();
    let draft = core.run(Command::SaveDraft { from_account_id: None, to: vec!["friend@example.test".into()], cc: vec![], subject: "Offer".into(), body: "Attached.".into(), reply_to_thread_id: None })
        .expect("saved").created_id.unwrap();
    core.run(Command::AddDraftAttachment { draft_id: draft.clone(), source: AttachmentSource::LocalFile { path: path.to_string_lossy().into_owned() } }).expect("attached");
    let listed: serde_json::Value = serde_json::from_str(&core.state(format!("draft/{draft}/attachments")).expect("served")).unwrap();
    let name = listed["attachments"][0]["file_name"].as_str().unwrap().to_string();
    assert_eq!(name, COMPOSED, "the row's name is composed: {:?}", name.chars().map(|c| format!("{:x}", c as u32)).collect::<Vec<_>>());
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    let now = core.run(Command::SendNow { outbox_id: entry });
    let got = wait_for("SMTP got the message", || (smtp.received(), format!("{now:?}")), |r| !r.0.is_empty()).0;
    let data = &got[0].data;
    let upper = data.to_ascii_uppercase();
    assert!(!upper.contains("%CC%81") && !data.contains('\u{301}'), "no decomposed accent on the wire");
    assert!(upper.contains("%C3%A9") || data.contains('\u{e9}'), "the composed é is on the wire");
    let back = send::part_bytes(data.as_bytes(), COMPOSED, 0).expect("the file is in the message under its composed name");
    assert!(back == bytes, "the bytes go as they are");
    println!("PASS name-sent-composed: listed and sent as {COMPOSED:?}, no U+0301 on the wire");
}
