//! attach-fetch (SPEC A152): the TEXT attachments of stored mail are fetched in the background, read and cut into `chunk` rows with no command and no click
//! (core/crates/engine/src/attachment_auto.rs). Over the scripted IMAP server of body-r2 (127.0.0.1 only); nothing here leaves this Mac. A mail's text or a
//! file's text never reaches an assertion message: the tests compare counts, booleans and ids only.
//!
//! Each test names its BROKEN COPY: a one-line change to the code under test that must turn it red (edit, run, undo). The lead runs them; this lane could not run cargo.
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/body_r2_server/mod.rs"]
mod server;

use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;
use contracts::*;
use engine::RustCore;
use server::{Fixture, Server};
use store::{AttachmentIn, NewMessage};

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(25);
/// What the engine's constants say (attachment_auto.rs); a test of the bound reads them from there, never types them again.
const PER_STEP: usize = 4;
const PAUSE_MS: u64 = 2_000;
const MAX_BYTES: u64 = 10 * 1024 * 1024;
const CAP_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const STATE: &str = "attachment/auto-fetch/transfer";
const WORDS: &str = "Warehouse budget approved by the quarterly committee";

fn wait_until(what: &str, within: Duration, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < within {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {within:?}: {what}");
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, uid: u32, at_ms: i64) -> NewMessage {
    let id = format!("{account}:INBOX:1:{uid}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{account}:<{uid}@thread>"), account_id: account.into(), sender_name: "Compta".into(), from_address: "compta@files.example".into(),
            to: vec![USER.into()], subject: format!("Facture {uid}"), preview: "Voir la facture".into(), received_at_ms: at_ms, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(60_000), tab_id: None,
    }
}

fn adapter(port: u16) -> sync_imap::ImapAdapter {
    sync_imap::ImapAdapter::new(sync_imap::ImapConfig {
        account_id: "acc".into(), account_name: "acc".into(), host: "127.0.0.1".into(), port, security: sync_imap::Security::PlainLoopbackOnly,
        username: USER.into(), auth: sync_imap::Auth::Password("app-password".into()), extra_roots_der: Vec::new(), limits: imap_fakes::limits(),
    })
}

/// A real, tiny PDF with one text line (the engine's PDF reader finds `WORDS` in it).
fn tiny_pdf() -> Vec<u8> {
    format!("%PDF-1.4\n1 0 obj\n<< /Length 80 >>\nstream\nBT /F1 12 Tf 72 720 Td ({WORDS}) Tj ET\nendstream\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n").into_bytes()
}

fn tiny_text() -> Vec<u8> { format!("Notes of the meeting. {WORDS}. Next step: sign.\n").into_bytes() }

fn tiny_png() -> Vec<u8> { [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A].iter().copied().chain((0..64u8).map(|i| i.wrapping_mul(37))).collect() }

/// One kind of file for `mail`: (MIME type, file name, bytes).
type File = (&'static str, &'static str, Vec<u8>);

/// A mail with a text body (part 1) and each file as part 2, 3, ... (base64), as the server holds it: the whole message and the body of each part by its number.
fn mail(uid: u32, files: &[File]) -> Fixture {
    let mut raw = format!("From: Compta <compta@files.example>\r\nTo: owner@example.test\r\nSubject: Facture {uid}\r\nMessage-ID: <{uid}@files.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/mixed; boundary=\"mix\"\r\n\r\n--mix\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nVoir le fichier ci-joint.\r\n");
    let mut sections = Vec::new();
    for (i, (mime, name, bytes)) in files.iter().enumerate() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        let wrapped = b64.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("\r\n");
        raw.push_str(&format!("--mix\r\nContent-Type: {mime}; name=\"{name}\"\r\nContent-Transfer-Encoding: base64\r\nContent-Disposition: attachment; filename=\"{name}\"\r\n\r\n{wrapped}\r\n"));
        sections.push(((i + 2).to_string(), wrapped.into_bytes()));
    }
    raw.push_str("--mix--\r\n");
    Fixture { raw: raw.into_bytes(), sections }
}

struct Rig { _rt: tokio::runtime::Runtime, srv: Server, dir: tempfile::TempDir, core: RustCore, account: String }

impl Rig {
    /// A core with one account, the server holding `mails` (uid, fixture), NO mail stored yet and no source registered.
    fn open(mails: Vec<(u32, Fixture)>) -> Rig {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let srv = rt.block_on(Server::start(&[], 1));
        for (uid, fx) in mails { srv.put(uid, fx); }
        let dir = tempfile::tempdir().unwrap();
        let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
        let account = core.store().add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        Rig { _rt: rt, srv, dir, core, account }
    }
    fn id(&self, uid: u32) -> String { format!("{}:INBOX:1:{uid}", self.account) }
    fn source(&self) { self.core.set_body_source(&self.account, Arc::new(adapter(self.srv.port))); }
    fn store_mail(&self, uid: u32, at_ms: i64) { self.core.store().insert_messages(vec![row(&self.account, uid, at_ms)]).unwrap(); }
    /// A file row written by hand for the stored mail `uid` (the rows a mail stored before this change already has): the part is `ordinal + 2`.
    fn put_rows(&self, uid: u32, files: &[(&str, &str, u64)]) {
        let rows: Vec<AttachmentIn> = files.iter().enumerate().map(|(i, (mime, name, size))| AttachmentIn {
            part: (i + 2).to_string(), name: (*name).into(), mime_type: (*mime).into(), size_bytes: *size, content_id: None, inline: false, encoding: "base64".into(),
        }).collect();
        self.core.store().put_attachment_rows(vec![(self.id(uid), rows)], true).unwrap();
    }
    fn att(&self, uid: u32, ordinal: usize) -> String { store::attachment_row_id(&self.id(uid), ordinal) }
    fn files(&self, uid: u32) -> Vec<AttachmentFile> { self.core.attachments(self.id(uid)) }
    fn on_disk(&self, uid: u32, ordinal: usize) -> bool { self.files(uid).iter().find(|f| f.id == self.att(uid, ordinal)).is_some_and(|f| f.local_path.is_some()) }
    fn chunks(&self, uid: u32, ordinal: usize) -> Vec<String> { self.core.store().attachment_chunks(&self.att(uid, ordinal)).unwrap().into_iter().map(|(_, t, _)| t).collect() }
    fn state(&self) -> serde_json::Value {
        let s = self.core.state(STATE.to_string()).expect("the state path answers");
        serde_json::from_str(&s).unwrap_or_else(|e| panic!("the state is not JSON ({e})"))
    }
    fn n(&self, key: &str) -> u64 { self.state()[key].as_u64().unwrap_or_else(|| panic!("the state has no number {key}")) }
    fn log(&self) -> String { std::fs::read_to_string(self.dir.path().join("core.log")).unwrap_or_default() }
    fn parts(&self) -> Vec<String> { self.srv.part_fetches() }
    /// Waits until the automatic fetch has taken `fetched` and `failed` files and the queue is empty.
    fn drained(&self, fetched: u64, failed: u64) {
        wait_until(&format!("the automatic fetch ends with {fetched} fetched and {failed} failed"), WAIT, || {
            let s = self.state();
            s["queued"] == 0 && s["fetched"] == fetched && s["failed"] == failed
        });
    }
    /// The mail is prefetched the way the sync does it (its body, rows and files come by the background paths, never by a command).
    fn prefetch(&self) { self.core.prefetch_bodies(&self.account, "INBOX"); }
}

/// (1) A mail with a small PDF and a text file: both are fetched and their `chunk` rows are written, with no DownloadAttachment and no read of the files.
/// BROKEN COPY: bodies.rs `download_with`, delete the line `crate::draft_files::download::auto::after_bodies(self, &self.store, &self.runtime, &kept_ids);`
/// (nothing is queued: red on the wait for the files).
#[test]
fn a_mail_with_a_pdf_and_a_text_file_gets_both_fetched_and_cut_with_no_command() {
    let r = Rig::open(vec![(7, mail(7, &[("application/pdf", "budget.pdf", tiny_pdf()), ("text/plain", "notes.txt", tiny_text())]))]);
    r.store_mail(7, 1_700_000_007_000);
    r.source();
    r.prefetch();
    r.drained(2, 0);
    assert!(r.on_disk(7, 0) && r.on_disk(7, 1), "both files are on this Mac");
    assert_eq!(r.parts().len(), 2, "one part fetch each, no more: {:?}", r.parts());
    for ordinal in 0..2 {
        let chunks = r.chunks(7, ordinal);
        assert!(!chunks.is_empty(), "file {ordinal} has no chunk rows");
        assert!(chunks.iter().any(|t| t.contains("budget approved")), "no piece of file {ordinal} holds its words");
    }
    assert!(r.log().lines().all(|l| !l.contains("not fetched automatically")), "no failure line");
}

/// (2) A picture is not fetched (only its row stays); the text file of the same mail is.
/// BROKEN COPY: attachment_auto.rs `text_kind`, `matches!(..., AttachmentKind::Pdf | AttachmentKind::PlainText)` -> `true` (the picture is fetched: red on the part count).
#[test]
fn a_picture_is_not_fetched() {
    let r = Rig::open(vec![(7, mail(7, &[("image/png", "photo.png", tiny_png()), ("text/plain", "notes.txt", tiny_text())]))]);
    r.store_mail(7, 1_700_000_007_000);
    r.source();
    r.prefetch();
    r.drained(1, 0);
    std::thread::sleep(Duration::from_millis(500));
    assert!(!r.on_disk(7, 0), "the picture stays a row");
    assert!(r.on_disk(7, 1), "the text file is here");
    assert_eq!(r.parts().len(), 1, "only the text file's part was asked: {:?}", r.parts());
    assert!(r.parts()[0].contains("BODY.PEEK[3]"), "the text file is part 3: {:?}", r.parts());
}

/// (3) A file over the size limit is not fetched. The row says 10 MiB + 1 (a stored mail's row, the backfill finds it); the server HOLDS its part, so a copy that
/// fetched it would show in the part count. A small text file beside it proves the queue worked.
/// BROKEN COPY: attachment_auto.rs `within_size`, `size <= ATTACH_AUTO_FETCH_MAX_BYTES` -> `size <= ATTACH_AUTO_FETCH_MAX_BYTES * 2` (and the SQL bound
/// `a.size_bytes <= ?2` in `backfill_rows` the same way): the big file is fetched.
#[test]
fn a_file_over_the_size_limit_is_not_fetched() {
    let r = Rig::open(vec![(7, mail(7, &[("application/pdf", "big.pdf", tiny_pdf()), ("text/plain", "notes.txt", tiny_text())]))]);
    r.store_mail(7, 1_700_000_007_000);
    r.put_rows(7, &[("application/pdf", "big.pdf", MAX_BYTES + 1), ("text/plain", "notes.txt", 60)]);
    r.source();
    r.drained(1, 0);
    std::thread::sleep(Duration::from_millis(500));
    assert!(!r.on_disk(7, 0), "the big file is not fetched");
    assert!(r.on_disk(7, 1), "the small one is");
    assert_eq!(r.parts().len(), 1, "{:?}", r.parts());
    assert!(r.parts()[0].contains("BODY.PEEK[3]"), "{:?}", r.parts());
    assert_eq!(r.n("failed"), 0, "a file over the limit is not a failure");
}

/// (4) A file already on this Mac is not fetched again, and never enters the queue: its rows carry a place (a real file), the mail's body then comes by the
/// prefetch (rows rewritten, place kept). The text file beside it is fetched. The state says nothing was skipped late (`skipped` 0: the file was never queued).
/// BROKEN COPY: attachment_auto.rs `enqueue`, delete `on_disk || ` from the `if` (the file is queued and only the late check in `one` skips it: `skipped` is 1).
#[test]
fn a_file_already_on_disk_is_not_fetched_again() {
    let r = Rig::open(vec![(7, mail(7, &[("application/pdf", "budget.pdf", tiny_pdf()), ("text/plain", "notes.txt", tiny_text())]))]);
    r.store_mail(7, 1_700_000_007_000);
    r.put_rows(7, &[("application/pdf", "budget.pdf", tiny_pdf().len() as u64), ("text/plain", "notes.txt", tiny_text().len() as u64)]);
    let mine = r.dir.path().join("already-here.pdf");
    std::fs::write(&mine, tiny_pdf()).unwrap();
    assert!(r.core.store().set_attachment_local_path(&r.att(7, 0), Some(&mine.to_string_lossy())).unwrap());
    r.source();
    // the backfill ran on the stored rows: the on-disk file is not in it (its row has a place) and the text file was fetched
    r.drained(1, 0);
    assert!(r.on_disk(7, 1));
    // a second mail of the same shape arrives by the prefetch: its first file is on disk before the body lands
    r.srv.put(8, mail(8, &[("application/pdf", "budget.pdf", tiny_pdf()), ("text/plain", "notes.txt", tiny_text())]));
    r.store_mail(8, 1_700_000_008_000);
    r.put_rows(8, &[("application/pdf", "budget.pdf", tiny_pdf().len() as u64), ("text/plain", "notes.txt", tiny_text().len() as u64)]);
    let mine8 = r.dir.path().join("already-here-8.pdf");
    std::fs::write(&mine8, tiny_pdf()).unwrap();
    assert!(r.core.store().set_attachment_local_path(&r.att(8, 0), Some(&mine8.to_string_lossy())).unwrap());
    r.prefetch();
    r.drained(2, 0);
    assert_eq!(r.parts().len(), 2, "one part fetch per text file that was not here, none for the two on disk: {:?}", r.parts());
    assert!(r.parts().iter().all(|p| p.contains("BODY.PEEK[3]")), "{:?}", r.parts());
    assert_eq!(std::fs::read(&mine8).unwrap(), tiny_pdf(), "the file on disk was not touched");
    assert!(r.chunks(8, 0).is_empty(), "no text round for a file that was not fetched");
    assert_eq!(r.n("skipped"), 0, "the files on disk were never queued");
}

/// (5) A failing fetch leaves ONE core.log line (the attachment id and the reason in words, no file text), the queue goes on to the next file, and the failed file
/// is not asked again at the next start (the ledger holds its failure): the core is opened again on the same folder and the server is asked nothing more for it.
/// The server holds the mail but not part 2, so that fetch fails; parts 3 and 4 are fine.
/// BROKEN COPY: (a) attachment_auto.rs `one`, delete the `crate::log::log_line(log, ...not fetched automatically...)` line (red: no line); (b) `enqueue`,
/// delete `|| again` (red after the restart: the failed file is asked again).
#[test]
fn a_failing_fetch_leaves_one_log_line_and_the_queue_goes_on() {
    let mut fx = mail(7, &[("application/pdf", "gone.pdf", tiny_pdf()), ("text/plain", "a.txt", tiny_text()), ("text/plain", "b.txt", tiny_text())]);
    fx.sections.retain(|(k, _)| k != "2");
    let r = Rig::open(vec![(7, fx)]);
    r.store_mail(7, 1_700_000_007_000);
    r.source();
    r.prefetch();
    r.drained(2, 1);
    assert!(!r.on_disk(7, 0) && r.on_disk(7, 1) && r.on_disk(7, 2), "the failed file is a row, the others are here");
    let failing = r.att(7, 0);
    let lines: Vec<String> = r.log().lines().filter(|l| l.contains(&failing) && l.contains("not fetched automatically")).map(str::to_string).collect();
    assert_eq!(lines.len(), 1, "exactly one line for the failed file: {}", lines.len());
    assert!(lines[0].split("not fetched automatically:").nth(1).is_some_and(|why| why.trim().split_whitespace().count() >= 3), "the reason is in words");
    assert!(r.log().lines().all(|l| !l.contains("Warehouse")), "no file text in the log");
    let asked_before = r.parts().iter().filter(|p| p.contains("BODY.PEEK[2]")).count();
    assert_eq!(asked_before, 1, "asked once, not in a loop: {:?}", r.parts());
    // the next start: a new text row appears (proves the backfill ran), the failed one stays quiet
    let Rig { _rt, srv, dir, core, account } = r;
    drop(core);
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the core opens again");
    srv.put(9, mail(9, &[("text/plain", "c.txt", tiny_text())]));
    core.store().insert_messages(vec![row(&account, 9, 1_700_000_009_000)]).unwrap();
    let id9 = format!("{account}:INBOX:1:9");
    core.store().put_attachment_rows(vec![(id9.clone(), vec![AttachmentIn { part: "2".into(), name: "c.txt".into(), mime_type: "text/plain".into(), size_bytes: 60, content_id: None, inline: false, encoding: "base64".into() }])], true).unwrap();
    core.set_body_source(&account, Arc::new(adapter(srv.port)));
    wait_until("the new text file after the restart", WAIT, || core.attachments(id9.clone()).iter().all(|f| f.local_path.is_some()) && core.state(STATE.to_string()).is_some_and(|s| s.contains("\"fetched\":1")));
    std::thread::sleep(Duration::from_millis(500));
    let again = srv.part_fetches().iter().filter(|p| p.contains("BODY.PEEK[2]") && p.contains("UID FETCH 7 ")).count();
    assert_eq!(again, 1, "the failed file was not asked again at the next start: {:?}", srv.part_fetches());
}

/// (6) The backfill: mail stored before this change (rows, no files) is queued once at the start, newest mail first, and its text files are fetched and cut.
/// BROKEN COPY: attachment_auto.rs `backfill_rows`, `ORDER BY m.received_at_ms DESC` -> `ASC` (red on the order).
#[test]
fn the_backfill_fetches_stored_mail_newest_first() {
    let r = Rig::open(vec![(11, mail(11, &[("text/plain", "n.txt", tiny_text())])), (12, mail(12, &[("text/plain", "n.txt", tiny_text())])), (13, mail(13, &[("text/plain", "n.txt", tiny_text())]))]);
    // stored out of order of their UIDs on purpose: the newest by date is uid 12
    for (uid, at) in [(11u32, 1_700_000_001_000i64), (12, 1_700_000_009_000), (13, 1_700_000_005_000)] {
        r.store_mail(uid, at);
        r.put_rows(uid, &[("text/plain", "n.txt", 60)]);
    }
    r.source();
    r.drained(3, 0);
    let order: Vec<u32> = r.parts().iter().map(|p| [11u32, 12, 13].into_iter().find(|u| p.contains(&format!("UID FETCH {u} "))).expect("a known uid")).collect();
    assert_eq!(order, vec![12, 13, 11], "newest mail first: {:?}", r.parts());
    for uid in [11, 12, 13] { assert!(r.on_disk(uid, 0) && !r.chunks(uid, 0).is_empty(), "uid {uid} fetched and cut"); }
    // once: registering the source again queues nothing and fetches nothing more
    r.source();
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(r.parts().len(), 3, "{:?}", r.parts());
}

/// (7) The per-step bound: six text files are queued; the loop takes PER_STEP of them, then yields for the pause; the rest come after it.
/// BROKEN COPY: attachment_auto.rs `run_loop`, `for _ in 0..ATTACH_AUTO_FETCH_PER_STEP` -> `for _ in 0..100` (all six are taken at once: red on `fetched == PER_STEP`).
#[test]
fn the_queue_yields_after_the_per_step_bound() {
    let files: Vec<File> = (0..6).map(|_| ("text/plain", "n.txt", tiny_text())).collect();
    let r = Rig::open(vec![(7, mail(7, &files))]);
    r.store_mail(7, 1_700_000_007_000);
    r.put_rows(7, &(0..6).map(|_| ("text/plain", "n.txt", 60u64)).collect::<Vec<_>>());
    r.source();
    wait_until("the first step", WAIT, || r.n("fetched") >= PER_STEP as u64);
    std::thread::sleep(Duration::from_millis(PAUSE_MS / 3));
    let s = r.state();
    assert_eq!((s["fetched"].as_u64(), s["queued"].as_u64()), (Some(PER_STEP as u64), Some(6 - PER_STEP as u64)), "the queue yields after one step: {s}");
    r.drained(6, 0);
}

/// (8) The total cap: the ledger says the automatic files already take the cap less 100 bytes (their row has a place, so the ledger still counts them); two text
/// files wait. Nothing is fetched, the queue is dropped, ONE core.log line says so, and the state path says `capped`.
/// BROKEN COPY: attachment_auto.rs `one`, `if used + f.size_bytes > ATTACH_AUTO_KEEP_TOTAL_MAX_BYTES` -> `if false` (both files are fetched: red).
#[test]
fn at_the_total_cap_nothing_more_is_fetched_and_one_line_says_so() {
    let r = Rig::open(vec![(7, mail(7, &[("text/plain", "a.txt", tiny_text()), ("text/plain", "b.txt", tiny_text())])), (8, mail(8, &[("text/plain", "old.txt", tiny_text())]))]);
    r.store_mail(8, 1_700_000_001_000);
    r.put_rows(8, &[("text/plain", "old.txt", 60)]);
    let old = r.dir.path().join("old.txt");
    std::fs::write(&old, tiny_text()).unwrap();
    assert!(r.core.store().set_attachment_local_path(&r.att(8, 0), Some(&old.to_string_lossy())).unwrap());
    std::fs::write(r.dir.path().join("attachment-autofetch.ledger"), format!("ok\t{}\t{}\n", r.att(8, 0), CAP_BYTES - 100)).unwrap();
    r.store_mail(7, 1_700_000_007_000);
    r.put_rows(7, &[("text/plain", "a.txt", 200), ("text/plain", "b.txt", 200)]);
    r.source();
    wait_until("the cap is reached", WAIT, || r.state()["capped"] == true);
    std::thread::sleep(Duration::from_millis(500));
    let s = r.state();
    assert_eq!((s["fetched"].as_u64(), s["queued"].as_u64()), (Some(0), Some(0)), "{s}");
    assert!(!r.on_disk(7, 0) && !r.on_disk(7, 1), "nothing was fetched");
    assert!(r.parts().is_empty(), "no part asked: {:?}", r.parts());
    assert_eq!(r.log().lines().filter(|l| l.contains("attachment auto fetch: stopped")).count(), 1, "one line says so");
    assert!(s["bytes_on_disk"].as_u64().is_some_and(|b| b == CAP_BYTES - 100), "the ledger's total is read: {s}");
}
