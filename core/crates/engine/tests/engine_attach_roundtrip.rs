//! fix-realcore round 3 (lead task 1ed1447c, SPEC A160 "the whole attachment path is proven on the real engine"): the two steps engine_files_r6.rs and
//! engine_attachments.rs do not cover, through the real core's commands over the in-process IMAP and SMTP fakes (127.0.0.1 only):
//!   1. a queued message keeps its files across a restart: Send, the core closes, a new core opens the same store, SendNow delivers both files byte for byte;
//!   2. the round trip: the message SMTP got is delivered back into INBOX, the sync stores it, its two file rows carry the names (one non-ASCII, RFC 2231)
//!      and the sizes, and DownloadAttachment puts on this Mac the very bytes that were attached (equal bytes).
//! Broken copies (one edit each, red first):
//!   outbox/src/adapters/store/mod.rs: `ctx.files_to_outbox(draft_id, entry_id)?;` deleted (1: the reopened outbox holds no file, red)
//!   send/src/attachments.rs `part`: `.header(ContentDisposition::attachment(&file.file_name))` -> `.header(ContentDisposition::attachment("file"))` (2: the names come back wrong, red)
//!   send/src/attachments.rs `base64_lines`: `out.push_str("\r\n");` (the last one) -> `out.push_str("AA\r\n");` (2: the downloaded bytes differ, red)
//!   draft_files.rs `state`: the `["message", id, "files"]` arm deleted (2: message/<id>/files is not served, red)
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../send/tests/support/mod.rs"]
mod smtp_fakes;

use std::path::Path;
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
const ACCENTED: &str = "Offre Téléfonica été 2026.pdf";
const BIG: &str = "scan-3mb.bin";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
/// Reads until `ok` holds; a red names what it read last (round 5: a wait that only said "never reached" hid its cause).
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
fn binary(n: usize, seed: u32) -> Vec<u8> {
    let mut x: u32 = seed | 1;
    (0..n).map(|_| { x ^= x << 13; x ^= x >> 17; x ^= x << 5; (x & 0xff) as u8 }).collect()
}
/// A short fingerprint for the failure text (the assertion compares the whole bytes): length and FNV-1a.
fn sha(b: &[u8]) -> String { format!("{} bytes, fnv {:016x}", b.len(), b.iter().fold(0xcbf29ce484222325u64, |h, x| (h ^ u64::from(*x)).wrapping_mul(0x100000001b3))) }
fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p.to_string_lossy().into_owned()
}
fn deliver(imap: &FakeImap, mbox: &str, raw: Vec<u8>) -> u32 {
    imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(imap_fakes::Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
        uid
    })
}
/// The numbered parts of a flat multipart message as an IMAP server numbers them (1, 2, 3 ...): each part's body still in its transfer encoding.
/// Round 6: the in-process fake serves `BODY.PEEK[<n>]` only from its hand-written `parts` table (sync-imap/tests/support/mod.rs:149-150, :572-575),
/// never from the message itself, so a test that downloads a file registers the parts of the message it delivers.
fn flat_parts(raw: &str) -> Vec<(String, Vec<u8>)> {
    let at = raw.find("boundary=").expect("a multipart message names its boundary") + "boundary=".len();
    let rest = &raw[at..];
    let boundary: String = if let Some(q) = rest.strip_prefix('"') { q.chars().take_while(|c| *c != '"').collect() } else { rest.chars().take_while(|c| !c.is_whitespace() && *c != ';').collect() };
    let marker = format!("--{boundary}");
    let mut out = Vec::new();
    for (i, chunk) in raw.split(&marker).skip(1).enumerate() {
        if chunk.starts_with("--") { break; }
        let body = chunk.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
        let body = body.strip_suffix("\r\n").unwrap_or(body);
        out.push(((i + 1).to_string(), body.as_bytes().to_vec()));
    }
    out
}
fn open(db: &Path, keys: &Arc<MemoryKeychain>) -> RustCore {
    RustCore::with_parts(Store::open(&db.join("whoosh.sqlite")).unwrap(), keys.clone(), detector(), TestTransport::PlainLoopback)
}
/// A saved draft with the two files attached; answers (draft id, the two files' bytes).
fn draft_with_files(core: &RustCore, files: &Path) -> (String, Vec<(String, Vec<u8>)>) {
    let pdf = b"%PDF-1.4\n% \xe2\xe3\xcf\xd3\nTelvora offer\nendobj\n".to_vec();
    let big = binary(3 * 1024 * 1024, 11);
    let draft = core.run(Command::SaveDraft { from_account_id: None, to: vec!["friend@example.test".into()], cc: vec![], subject: "Two files".into(), body: "Both attached.".into(), reply_to_thread_id: None })
        .expect("saved").created_id.unwrap();
    for (name, bytes) in [(ACCENTED, &pdf), (BIG, &big)] {
        let path = write_file(files, name, bytes);
        core.run(Command::AddDraftAttachment { draft_id: draft.clone(), source: AttachmentSource::LocalFile { path } }).expect("attached");
    }
    (draft, vec![(ACCENTED.to_string(), pdf), (BIG.to_string(), big)])
}
fn queued_names(core: &RustCore, entry: &str) -> Vec<String> {
    let raw = core.state(format!("outbox/{entry}/attachments")).unwrap_or_default();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);
    v["attachments"].as_array().map(|a| a.iter().filter_map(|x| x["file_name"].as_str().map(String::from)).collect()).unwrap_or_default()
}

#[test]
fn a_queued_message_keeps_its_files_across_a_restart_and_sends_them_byte_for_byte() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    let smtp = FakeSmtp::start(Mode::Xoauth2);
    let (db, files) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let core = open(db.path(), &keys);
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } })
        .expect("added").created_id.unwrap();
    wait_for("listening", || core.sync_phase(&id), |p| *p == Some(SyncPhase::Listening));
    let (draft, sent) = draft_with_files(&core, files.path());
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    assert_eq!(queued_names(&core, &entry), vec![ACCENTED.to_string(), BIG.into()]);
    drop(core);   // the app closes inside the undo window: nothing was sent
    assert!(smtp.received().is_empty(), "nothing left before the restart");
    let core = open(db.path(), &keys);
    assert_eq!(queued_names(&core, &entry), vec![ACCENTED.to_string(), BIG.into()], "the reopened outbox still holds both files");
    // round 5: `with_parts` starts no sync by itself ("Nothing syncs until an account is added or `start_stored_accounts` is called", rust_core.rs);
    // the app's launch calls it (open_production), so the reopened core does too
    core.start_stored_accounts();
    wait_for("listening again", || core.sync_phase(&id), |p| *p == Some(SyncPhase::Listening));
    // the undo window may have ended while the sync came back (the worker then sends on its own): SendNow is asked, its answer is not the judge
    let now = core.run(Command::SendNow { outbox_id: entry.clone() });
    wait_for("SMTP got the message", || (smtp.received().len(), format!("{now:?}"), core.state(format!("outbox/{entry}/attachments"))), |r| r.0 > 0);
    let got = smtp.received();
    assert_eq!(got.len(), 1, "one message after the restart");
    for (name, bytes) in &sent {
        let back = send::part_bytes(got[0].data.as_bytes(), name, 0).unwrap_or_else(|| panic!("no file {name:?} in the message SMTP got"));
        assert!(back == *bytes, "{name}: the bytes sent after the restart: {} against {}", sha(&back), sha(bytes));
    }
    println!("PASS outbox-keeps-files-across-a-restart: both files sent byte for byte after the core reopened");
}

#[test]
fn a_sent_message_comes_back_with_both_file_rows_and_downloads_the_same_bytes() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    let smtp = FakeSmtp::start(Mode::Xoauth2);
    let (db, files) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let keys = Arc::new(MemoryKeychain::new(Namespace::rig()));
    let core = open(db.path(), &keys);
    let id = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } })
        .expect("added").created_id.unwrap();
    wait_for("listening", || core.sync_phase(&id), |p| *p == Some(SyncPhase::Listening));
    let (draft, sent) = draft_with_files(&core, files.path());
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    let now = core.run(Command::SendNow { outbox_id: entry });
    wait_for("SMTP got the message", || (smtp.received().len(), format!("{now:?}")), |r| r.0 > 0);
    let data = smtp.received()[0].data.clone();
    assert!(data.contains("multipart/mixed"), "the message is multipart/mixed");
    assert!(data.contains("filename*"), "the non-ASCII name goes as an RFC 2231 parameter");
    // the same message arrives back in INBOX (a mail to oneself): the sync stores it, the body fetch writes its file rows
    // its own Message-ID, so the store never takes it for the Sent copy it already holds
    let back = data.replacen("Message-ID: <", "Message-ID: <back-", 1);
    assert_ne!(back, data, "the message carries a Message-ID header");
    let parts = flat_parts(&back);
    assert_eq!(parts.len(), 3, "the text, then one part per file (send/src/compose.rs:148)");
    let uid = deliver(&imap, "INBOX", back.into_bytes());
    imap.with(|s| for (section, body) in &parts { s.parts.push((uid, section.clone(), body.clone())); });
    // round 5: the fake's IDLE wakes only on its own notice (support/mod.rs: `bump.notify_waiters()`, which `add_message` and `change_flags` send);
    // a message pushed into a box by hand is never announced, so the sync never fetched it ("the arrived message is stored" could never be met).
    // A flag change on no message sends that notice and changes nothing else.
    imap.change_flags("INBOX", &[], "\\Seen");
    let mid = || core.store().uids_in_folder(&id, "INBOX").unwrap().into_iter().find(|(u, _)| *u == uid).map(|(_, m)| m);
    wait_for("the arrived message is stored", || core.store().uids_in_folder(&id, "INBOX").unwrap().into_iter().map(|(u, _)| u).collect::<Vec<u32>>(), |u| u.contains(&uid));
    let message = mid().unwrap();
    let _ = core.body(message.clone());
    wait_for("its two file rows", || core.attachments(message.clone()).iter().map(|a| (a.file_name.clone(), a.size_bytes, a.inline)).collect::<Vec<_>>(), |v| v.iter().filter(|x| !x.2).count() == 2);
    let rows = core.attachments(message.clone());
    for (name, bytes) in &sent {
        let row = rows.iter().find(|a| &a.file_name == name).unwrap_or_else(|| panic!("no row named {name:?}: {:?}", rows.iter().map(|a| &a.file_name).collect::<Vec<_>>()));
        assert_eq!(row.size_bytes, bytes.len() as u64, "{name}: the row's size is the file's");
        if row.local_path.is_none() {
            core.run(Command::DownloadAttachment { attachment_id: row.id.clone() }).expect("download asked");
        }
        let rid = row.id.clone();
        wait_for(&format!("{name} on this Mac"), || core.state(format!("attachment/{rid}/transfer")), |s| s.as_ref().is_some_and(|s| s.contains("on_this_mac")));
        let here = core.attachments(message.clone()).into_iter().find(|a| a.id == rid).and_then(|a| a.local_path).expect("a local path once on this Mac");
        let got = std::fs::read(&here).unwrap();
        assert!(got == *bytes, "{name}: the downloaded bytes are the attached bytes: {} against {}", sha(&got), sha(bytes));
        // the reads the hidden --slice-attachsend uses: attachment/<id>/local names the same file and size
        let local: serde_json::Value = serde_json::from_str(&core.state(format!("attachment/{rid}/local")).expect("attachment/<id>/local is served")).unwrap();
        assert_eq!(local["path"].as_str(), Some(here.as_str()), "{name}: attachment/<id>/local names the file on this Mac");
        assert_eq!(local["size"].as_u64(), Some(bytes.len() as u64));
    }
    let files: serde_json::Value = serde_json::from_str(&core.state(format!("message/{message}/files")).expect("message/<id>/files is served")).unwrap();
    let listed: Vec<(String, u64, bool)> = files.as_array().unwrap().iter().filter(|f| f["inline"] == false)
        .map(|f| (f["file_name"].as_str().unwrap().to_string(), f["size_bytes"].as_u64().unwrap(), f["on_this_mac"].as_bool().unwrap())).collect();
    let mut want: Vec<(String, u64, bool)> = sent.iter().map(|(n, b)| (n.clone(), b.len() as u64, true)).collect();
    let mut have = listed.clone();
    want.sort(); have.sort();
    assert_eq!(have, want, "message/<id>/files: both files, their sizes, on this Mac");
    println!("PASS attachment-round-trip: 2 rows (names and sizes as attached), both downloads byte for byte");
}
