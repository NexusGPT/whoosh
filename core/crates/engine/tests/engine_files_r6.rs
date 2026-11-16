//! engine-act-r6 round 2 (contracts delta 22): a file the owner attaches really leaves with the mail. Through the real core's commands only:
//! `AddDraftAttachment` / `RemoveDraftAttachment` on a saved draft, `Forward` carrying the original's files, `Send` then `SendNow` or `Undo`;
//! read back from the core's state paths (`draft/<id>/attachments`, `outbox/<id>/attachments`), from what the local fake SMTP server received
//! and from the fake IMAP server's Sent folder. Nothing leaves this Mac. The single-edit mutant each test was seen red on is named under it.
#![allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../send/tests/support/mod.rs"]
mod smtp_fakes;

use std::path::{Path, PathBuf};
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
const WAIT: Duration = Duration::from_secs(25);
const ACCENTED: &str = "Devis été 2026 final.pdf";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

struct Rig { core: RustCore, imap: FakeImap, smtp: FakeSmtp, files: tempfile::TempDir, _rt: tokio::runtime::Runtime, _db: tempfile::TempDir }

/// The fake IMAP server (with `inbox` raw messages in INBOX before the account is added), the fake SMTP server (it offers AUTH and keeps
/// every message), and the account added through the core; the sync is listening when it returns.
fn rig(inbox: Vec<Vec<u8>>) -> Rig {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    for raw in inbox { deliver(&imap, "INBOX", raw); }
    let smtp = FakeSmtp::start(Mode::Xoauth2);
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(&db.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    Rig { core, imap, smtp, files: tempfile::tempdir().unwrap(), _rt: rt, _db: db }
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
fn sent_box(imap: &FakeImap) -> Vec<Vec<u8>> { imap.with(|s| s.boxes.iter().find(|b| b.name == "Sent").unwrap().msgs.iter().map(|m| m.raw.clone()).collect()) }

/// Bytes no text encoding survives (every byte value, bare CR and LF), pseudo-random.
fn binary(n: usize, seed: u32) -> Vec<u8> {
    let mut x: u32 = seed | 1;
    (0..n).map(|_| { x ^= x << 13; x ^= x >> 17; x ^= x << 5; (x & 0xff) as u8 }).collect()
}
fn write_file(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let p: PathBuf = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p.to_string_lossy().into_owned()
}
fn local(path: &str) -> AttachmentSource { AttachmentSource::LocalFile { path: path.into() } }
fn saved_draft(core: &RustCore) -> String {
    core.run(Command::SaveDraft { from_account_id: None, to: vec!["friend@example.test".into()], cc: vec![], subject: "The files".into(), body: "Here they are.".into(), reply_to_thread_id: None })
        .expect("saved").created_id.unwrap()
}
fn attach(core: &RustCore, draft: &str, source: AttachmentSource) -> Result<String, CoreError> {
    core.run(Command::AddDraftAttachment { draft_id: draft.into(), source }).map(|o| o.created_id.expect("the attachment id"))
}
fn code(r: Result<impl std::fmt::Debug, CoreError>) -> String {
    match r { Err(CoreError::Invalid { field, .. }) => field, Err(CoreError::NotFound { kind, .. }) => format!("not found: {kind}"), other => format!("not refused: {other:?}") }
}
/// The list a state path answers: (file name, state, owner) of each file, and the total.
fn listed(core: &RustCore, path: &str) -> (Vec<(String, String, String)>, u64) {
    let raw = core.state(path.into()).unwrap_or_else(|| panic!("no answer for {path}"));
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{path}: {e}: {raw}"));
    let rows = v["attachments"].as_array().unwrap().iter().map(|a| (a["file_name"].as_str().unwrap().to_string(), a["state"].as_str().unwrap().to_string(), a["owner_id"].as_str().unwrap().to_string())).collect();
    (rows, v["total_bytes"].as_u64().unwrap())
}
fn names(rows: &[(String, String, String)]) -> Vec<String> { rows.iter().map(|r| r.0.clone()).collect() }
/// Send the draft, then deliver it at once (no wait for the undo window). Answers the outbox entry.
fn send_now(core: &RustCore, draft: &str) -> String {
    let entry = core.run(Command::Send { draft_id: draft.into() }).expect("queued").created_id.unwrap();
    core.run(Command::SendNow { outbox_id: entry.clone() }).expect("sent now");
    entry
}

// ---------------------------------------------------------------- 1. the files go, byte for byte, and the Sent copy is what SMTP got

#[test]
fn the_sent_message_holds_each_file_byte_for_byte_and_the_sent_copy_is_what_smtp_got() {
    let r = rig(vec![]);
    let text = b"line one\nline two\n\nno CR in this file\n".to_vec();
    let big = binary(5 * 1024 * 1024, 7);
    let pdf = b"%PDF-1.4\n% \xe2\xe3\xcf\xd3\nendobj\n".to_vec();
    let (pt, pb, pp) = (write_file(r.files.path(), "notes.txt", &text), write_file(r.files.path(), "photo.raw", &big), write_file(r.files.path(), ACCENTED, &pdf));
    let draft = saved_draft(&r.core);
    for p in [&pt, &pb, &pp] { attach(&r.core, &draft, local(p)).expect("attached"); }
    assert_eq!(attach(&r.core, &draft, local(&pt)).expect("the same file again"), format!("{draft}/1"), "adding a file twice answers its id, adds nothing");
    let (rows, total) = listed(&r.core, &format!("draft/{draft}/attachments"));
    assert_eq!(names(&rows), vec!["notes.txt".to_string(), "photo.raw".into(), ACCENTED.into()]);
    assert!(rows.iter().all(|x| x.1 == "ready"), "{rows:?}");
    assert_eq!(total, (text.len() + big.len() + pdf.len()) as u64);

    let entry = r.core.run(Command::Send { draft_id: draft.clone() }).expect("queued").created_id.unwrap();
    let (queued, _) = listed(&r.core, &format!("outbox/{entry}/attachments"));
    assert_eq!(names(&queued), names(&rows), "the queued message carries the draft's files");
    assert!(queued.iter().all(|x| x.2 == entry), "owned by the outbox entry now: {queued:?}");
    r.core.run(Command::SendNow { outbox_id: entry.clone() }).expect("sent now");

    let got = r.smtp.received();
    assert_eq!(got.len(), 1, "one message reached SMTP");
    let data = got[0].data.as_bytes();
    for (name, bytes) in [("notes.txt", &text), ("photo.raw", &big), (ACCENTED, &pdf)] {
        let back = send::part_bytes(data, name, 0).unwrap_or_else(|| panic!("no file {name:?} in the message SMTP got"));
        assert!(back == *bytes, "{name}: {} bytes back of {}", back.len(), bytes.len());
    }
    wait_until("the Sent copy on the server", || !sent_box(&r.imap).is_empty());
    let copies = sent_box(&r.imap);
    assert_eq!(copies.len(), 1);
    assert_eq!(String::from_utf8_lossy(&copies[0]).trim_end_matches("\r\n"), got[0].data.trim_end_matches("\r\n"), "the Sent copy is the very message SMTP got");
    let copy = r.core.message(format!("sent-{entry}")).expect("the stored sent copy");
    let (mut listed_names, mut sent_names) = (copy.attachment_names.clone(), names(&rows));
    listed_names.sort();
    sent_names.sort();
    assert_eq!(listed_names, sent_names, "the stored copy lists its files (the store reads names in name order)");
    assert_eq!(r.core.attachments(format!("sent-{entry}")).len(), 3);
}
// mutant "sender-reads-no-files": outbox/src/adapters/sender/mod.rs, `(Some(port), false) => port.execute(draft)?,` -> `(Some(_), false) => Vec::new(),` (red: refused in words, nothing reaches SMTP)
// mutant "files-stay-with-the-draft": outbox/src/adapters/store/mod.rs, `ctx.files_to_outbox(draft_id, entry_id)?;` deleted (red: outbox/<id>/attachments empty; the draft's delete trigger takes them)

// ---------------------------------------------------------------- 2. undo of a send gives the draft back with its files

#[test]
fn undo_of_a_send_gives_the_draft_back_with_its_files() {
    let r = rig(vec![]);
    let p = write_file(r.files.path(), "contract.pdf", b"%PDF the contract");
    let draft = saved_draft(&r.core);
    attach(&r.core, &draft, local(&p)).expect("attached");
    let entry = r.core.run(Command::Send { draft_id: draft.clone() }).expect("queued").created_id.unwrap();
    assert!(listed(&r.core, &format!("draft/{draft}/attachments")).0.is_empty(), "the draft is in the outbox");
    r.core.run(Command::Undo).expect("undo inside the window");
    let (back, _) = listed(&r.core, &format!("draft/{draft}/attachments"));
    assert_eq!(names(&back), vec!["contract.pdf".to_string()], "the draft is back with its file");
    assert!(back.iter().all(|x| x.2 == draft), "owned by the draft again: {back:?}");
    assert!(listed(&r.core, &format!("outbox/{entry}/attachments")).0.is_empty());
    // and it can be sent again, with the file
    let again = send_now(&r.core, &draft);
    assert_ne!(again, entry);
    let got = r.smtp.received();
    assert_eq!(got.len(), 1, "only the second send left");
    assert_eq!(send::part_bytes(got[0].data.as_bytes(), "contract.pdf", 0).as_deref(), Some(&b"%PDF the contract"[..]));
}
// mutant "undo-forgets-the-files": outbox/src/adapters/store/mod.rs, `ctx.files_to_draft(entry_id, &d.id)?;` deleted (red: the draft comes back with no file)

// ---------------------------------------------------------------- 3. every refusal, in words with its code

#[test]
fn each_refusal_has_its_code_and_nothing_half_attached_is_queued() {
    let r = rig(vec![]);
    let draft = saved_draft(&r.core);
    let dir = r.files.path();
    assert_eq!(code(attach(&r.core, &draft, local("relative/notes.txt"))), ATTACHMENT_BAD_SOURCE_CODE, "a path that is not absolute nor under ~/");
    assert_eq!(code(attach(&r.core, &draft, AttachmentSource::StoredMessage { message_id: "".into(), attachment_id: "".into() })), ATTACHMENT_BAD_SOURCE_CODE);
    assert_eq!(code(attach(&r.core, &draft, local(&dir.join("never-there.pdf").to_string_lossy()))), ATTACHMENT_GONE_CODE);
    let over = write_file(dir, "over.bin", &vec![0u8; ATTACHMENT_FILE_MAX_BYTES as usize + 1]);
    assert_eq!(code(attach(&r.core, &draft, local(&over))), ATTACHMENT_TOO_LARGE_CODE);
    let eight = write_file(dir, "eight.bin", &vec![1u8; 8 * 1024 * 1024]);
    let seven = write_file(dir, "seven.bin", &vec![2u8; 7 * 1024 * 1024]);
    attach(&r.core, &draft, local(&eight)).expect("8 MiB is under both caps");
    assert_eq!(code(attach(&r.core, &draft, local(&seven))), ATTACHMENT_TOTAL_TOO_LARGE_CODE, "8 + 7 MiB is over the 14 MiB total");
    assert_eq!(code(attach(&r.core, "draft-nobody", local(&eight))), "not found: draft");
    assert_eq!(code(attach(&r.core, &draft, AttachmentSource::StoredMessage { message_id: "m-none".into(), attachment_id: "a-none".into() })), "not found: attachment");
    assert_eq!(code(r.core.run(Command::RemoveDraftAttachment { draft_id: draft.clone(), attachment_id: "nope".into() })), "not found: attachment");
    // a file deleted after it was added: the draft reads it gone, and Send is refused before anything is queued
    let soon_gone = write_file(dir, "soon-gone.txt", b"here now");
    attach(&r.core, &draft, local(&soon_gone)).expect("attached");
    std::fs::remove_file(&soon_gone).unwrap();
    let (rows, _) = listed(&r.core, &format!("draft/{draft}/attachments"));
    assert_eq!(rows.iter().find(|x| x.0 == "soon-gone.txt").map(|x| x.1.as_str()), Some("gone"), "{rows:?}");
    assert_eq!(code(r.core.run(Command::Send { draft_id: draft.clone() })), ATTACHMENT_GONE_CODE);
    assert!(r.core.outbox().is_empty(), "nothing queued");
    assert_eq!(names(&listed(&r.core, &format!("draft/{draft}/attachments")).0), vec!["eight.bin".to_string(), "soon-gone.txt".into()], "the draft keeps its files");
    // the owner removes the gone one; once queued, the draft's files are closed: undo the send to change them
    r.core.run(Command::RemoveDraftAttachment { draft_id: draft.clone(), attachment_id: format!("{draft}/2") }).expect("removed");
    r.core.run(Command::Send { draft_id: draft.clone() }).expect("queued");
    assert_eq!(code(attach(&r.core, &draft, local(&seven))), ATTACHMENT_DRAFT_CLOSED_CODE);
    assert_eq!(code(r.core.run(Command::RemoveDraftAttachment { draft_id: draft.clone(), attachment_id: format!("{draft}/1") })), ATTACHMENT_DRAFT_CLOSED_CODE);
}
// mutant "send-not-checked-before-queueing": actions_bridge.rs, the `crate::draft_files::check_sendable(self.store(), &draft_id)?;` line of `Send` deleted (red: the gone file's draft is queued)
// mutant "closed-draft-not-refused": draft_files.rs `draft_open`, `check_attachment_draft_open(open)` -> `check_attachment_draft_open(true)` (red: attachment_draft_closed never answered)

// ---------------------------------------------------------------- 4. Forward carries the original's files, fetched from the server when not on this Mac

/// A message with three files: two small ones and one over the per-file cap (its bytes are small here: the cap is read from the stored size).
fn original_with_files(one: &[u8], two: &[u8]) -> Vec<u8> {
    use base64::Engine as _;
    let b = |x: &[u8]| base64::engine::general_purpose::STANDARD.encode(x);
    format!(
        "From: Karen Bosch <karen@linea-example.com>\r\nTo: {USER}\r\nSubject: The quote and the plan\r\nMessage-ID: <quote-1@linea-example.com>\r\n\
         Date: Sat, 07 Nov 2026 10:00:00 +0000\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"BOUND\"\r\n\r\n\
         --BOUND\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBoth files attached.\r\n\
         --BOUND\r\nContent-Type: application/pdf; name=\"quote.pdf\"\r\nContent-Disposition: attachment; filename=\"quote.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n\
         --BOUND\r\nContent-Type: image/png; name=\"site plan.png\"\r\nContent-Disposition: attachment; filename=\"site plan.png\"\r\nContent-Transfer-Encoding: base64\r\n\r\n{}\r\n\
         --BOUND--\r\n",
        b(one), b(two)
    ).into_bytes()
}

#[test]
fn forward_carries_the_originals_files_and_sends_them_one_too_large_is_refused_until_removed() {
    let quote = b"%PDF the quote, on this Mac".to_vec();
    let plan = binary(64 * 1024, 11);
    let r = rig(vec![original_with_files(&quote, &plan)]);
    let store = r.core.store();
    let mid = { let s = store.clone(); wait_until("the original is stored", || s.write(|c| Ok(c.tx.query_row("SELECT id FROM message WHERE subject = 'The quote and the plan'", [], |r| r.get::<_, String>(0)).ok())).unwrap().is_some()); store.write(|c| Ok(c.tx.query_row("SELECT id FROM message WHERE subject = 'The quote and the plan'", [], |r| r.get::<_, String>(0))?)).unwrap() };
    // the files as the store lists them: the quote downloaded to this Mac, the plan not (fetched from the server when sent), one over the cap
    let on_mac = write_file(r.files.path(), "quote.pdf", &quote);
    let (m, qlen, plen) = (mid.clone(), quote.len() as i64, plan.len() as i64);
    // body-r3: the sync's own body fetch now writes this message's file rows; this test lists its own three (a file on this Mac, one to fetch, one too large)
    assert!(r.core.settle_bodies(WAIT));
    store.write({ let m = m.clone(); move |c| { c.tx.execute("DELETE FROM attachment WHERE message_id = ?1", [&m])?; Ok(()) } }).unwrap();
    store.write(move |c| {
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a1', ?1, 'quote.pdf', 'application/pdf', ?2, ?3)", rusqlite::params![m, qlen, on_mac])?;
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a2', ?1, 'site plan.png', 'image/png', ?2, NULL)", rusqlite::params![m, plen])?;
        c.tx.execute("INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES ('a3', ?1, 'scan.tiff', 'image/tiff', ?2, NULL)", rusqlite::params![m, ATTACHMENT_FILE_MAX_BYTES as i64 + 1])?;
        Ok(())
    }).unwrap();
    let draft = r.core.run(Command::Forward { message_id: mid.clone() }).expect("forward").created_id.unwrap();
    let (rows, _) = listed(&r.core, &format!("draft/{draft}/attachments"));
    assert_eq!(rows.iter().map(|x| (x.0.as_str(), x.1.as_str())).collect::<Vec<_>>(), vec![("quote.pdf", "ready"), ("site plan.png", "will_download"), ("scan.tiff", "too_large")], "every file of the original, in its order");
    let (subject, body): (String, String) = { let d = draft.clone(); store.write(move |c| Ok(c.tx.query_row("SELECT subject, body FROM draft WHERE id = ?1", [&d], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap() };
    assert_eq!(subject, "Fwd: The quote and the plan");
    assert!(body.contains("Attachments: quote.pdf, site plan.png, scan.tiff") && body.contains("Forwarded message"), "{body}");
    r.core.run(Command::UpdateDraft { draft_id: draft.clone(), from_account_id: None, to: vec!["boss@example.test".into()], cc: vec![], bcc: vec![], subject: subject.clone(), body: body.clone() }).expect("a recipient");
    // the one over the cap stops the send until the owner removes it
    assert_eq!(code(r.core.run(Command::Send { draft_id: draft.clone() })), ATTACHMENT_TOO_LARGE_CODE);
    assert!(r.core.outbox().is_empty());
    r.core.run(Command::RemoveDraftAttachment { draft_id: draft.clone(), attachment_id: format!("{draft}/3") }).expect("removed");
    assert!(body.contains("scan.tiff"), "the name list in the body is text and stays");
    send_now(&r.core, &draft);
    let got = r.smtp.received();
    assert_eq!(got.len(), 1, "{:?}", r.core.outbox().iter().map(|e| format!("{:?}", e.state)).collect::<Vec<_>>());
    let data = got[0].data.as_bytes();
    assert_eq!(send::part_bytes(data, "quote.pdf", 0).as_deref(), Some(&b"%PDF the quote, on this Mac"[..]), "the file on this Mac");
    assert!(send::part_bytes(data, "site plan.png", 0).as_deref() == Some(&plan[..]), "the file fetched from the server, byte for byte");
    assert!(send::part_bytes(data, "scan.tiff", 0).is_none(), "the removed one did not go");
}
// mutant "forward-carries-no-files": draft_files.rs `forward`, `for row in forward_attachments(id.clone(), original) { ctx.draft_file_insert(&row)?; }` -> `for _ in [0u8; 0] {}` (red: the draft lists nothing)
// mutant "forwarded-file-never-fetched": draft_files.rs `stored_bytes`, the server fetch replaced by `Err(fail("off".into()))` (red: the send fails in words, nothing reaches SMTP)

// ---------------------------------------------------------------- 5. a removed file is not sent

#[test]
fn a_removed_file_is_not_sent() {
    let r = rig(vec![]);
    let (keep, drop) = (write_file(r.files.path(), "keep.txt", b"this one goes"), write_file(r.files.path(), "drop.txt", b"this one stays home"));
    let draft = saved_draft(&r.core);
    attach(&r.core, &draft, local(&keep)).expect("attached");
    let dropped = attach(&r.core, &draft, local(&drop)).expect("attached");
    r.core.run(Command::RemoveDraftAttachment { draft_id: draft.clone(), attachment_id: dropped }).expect("removed");
    assert!(Path::new(&drop).exists(), "the file of this Mac is never deleted");
    assert_eq!(names(&listed(&r.core, &format!("draft/{draft}/attachments")).0), vec!["keep.txt".to_string()]);
    send_now(&r.core, &draft);
    let got = r.smtp.received();
    assert_eq!(got.len(), 1);
    let data = got[0].data.as_bytes();
    assert_eq!(send::part_bytes(data, "keep.txt", 0).as_deref(), Some(&b"this one goes"[..]));
    assert!(send::part_bytes(data, "drop.txt", 0).is_none(), "the removed file did not go");
    assert!(!got[0].data.contains("this one stays home"));
}
// mutant "remove-keeps-the-row": draft_files.rs `remove`, `ctx.draft_file_delete(&draft, &id)?` -> `true` (red: drop.txt still listed and sent)
