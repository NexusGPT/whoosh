//! attach-engine (contracts delta 23): `DownloadAttachment` in the real core, over the scripted IMAP server of body-r2 (tests/body_r2_server, 127.0.0.1 only)
//! behind a small gate on 127.0.0.1 (`Gate`: it can hold the server's answers back, so a download can be seen while it runs, and a silent server can be
//! played). Nothing here leaves this Mac. Each test names its red mutants: a one-edit change to the code under test (source edited, test run, edit undone;
//! the logs are in the lane's output folder).
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/body_r2_server/mod.rs"]
mod server;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::constants::ATTACHMENT_TEXT_MAX_CHARS;
use contracts::*;
use engine::RustCore;
use server::{files_mail, FixtureFile, Server};
use store::NewMessage;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(20);
/// The silent server ends at the adapter's own limit (IMAP_COMMAND_TIMEOUT_MS, 30 s) plus the open: wait longer than one limit.
const SILENCE_WAIT: Duration = Duration::from_secs(75);
const AT_ONCE: Duration = Duration::from_millis(50);
/// How long a test waits for the command's own answer before it fails (the command must not wait for the network at all).
const ANSWER_WITHIN: Duration = Duration::from_secs(3);

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

fn row(account: &str, uid: u32, size: u64) -> NewMessage {
    let id = format!("{account}:INBOX:1:{uid}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{account}:<{uid}@thread>"), account_id: account.into(), sender_name: "Compta".into(), from_address: "compta@files.example".into(),
            to: vec![USER.into()], subject: format!("Facture {uid}"), preview: "Voir la facture".into(), received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(size as i64), tab_id: None,
    }
}

/// A relay on 127.0.0.1 between the core and the scripted server. While `hold` is set it keeps the server's bytes back (the client sees a silent server).
struct Gate { hold: AtomicBool }

async fn relay(target: u16, gate: Arc<Gate>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((client, _)) = listener.accept().await else { return };
            let gate = gate.clone();
            tokio::spawn(async move {
                let Ok(upstream) = TcpStream::connect(("127.0.0.1", target)).await else { return };
                let ((mut cr, mut cw), (mut sr, mut sw)) = (client.into_split(), upstream.into_split());
                let up = tokio::spawn(async move { let _ = tokio::io::copy(&mut cr, &mut sw).await; });
                let mut buf = [0u8; 4096];
                loop {
                    let n = match sr.read(&mut buf).await { Ok(0) | Err(_) => break, Ok(n) => n };
                    while gate.hold.load(Ordering::SeqCst) { tokio::time::sleep(Duration::from_millis(10)).await; }
                    if cw.write_all(&buf[..n]).await.is_err() { break; }
                }
                up.abort();
            });
        }
    });
    port
}

/// The Thread stream's `AttachmentTransferChanged` events, in order: (attachment id, message id, transfer).
#[derive(Default)]
struct Events(Mutex<Vec<(String, String, AttachmentTransfer)>>);
impl EventSink for Events {
    fn on_event(&self, event: CoreEvent) {
        if let CoreEvent::AttachmentTransferChanged { attachment_id, message_id, transfer } = event { self.0.lock().unwrap().push((attachment_id, message_id, transfer)); }
    }
}

struct Rig { _rt: tokio::runtime::Runtime, srv: Server, gate: Arc<Gate>, dir: tempfile::TempDir, core: RustCore, events: Arc<Events>, account: String, files: Vec<FixtureFile> }

fn open_core(dir: &std::path::Path) -> (RustCore, Arc<Events>) {
    std::env::set_var("WHOOSH_ATTACH_AUTO_FETCH", "off"); // this file proves the fetch ON A CLICK: the automatic fetch (attachment_auto.rs) is off here
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let events = Arc::new(Events::default());
    core.subscribe(StreamKind::Thread, events.clone());
    (core, events)
}

fn adapter(port: u16) -> sync_imap::ImapAdapter {
    sync_imap::ImapAdapter::new(sync_imap::ImapConfig {
        account_id: "acc".into(), account_name: "acc".into(), host: "127.0.0.1".into(), port, security: sync_imap::Security::PlainLoopbackOnly,
        username: USER.into(), auth: sync_imap::Auth::Password("app-password".into()), extra_roots_der: Vec::new(), limits: imap_fakes::limits(),
    })
}

impl Rig {
    /// A core with one account whose messages 7 and 8 are stored with their preview only and read once by the background prefetch (rows, no files).
    fn start() -> Rig {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let srv = rt.block_on(Server::start(&[], 1));
        let (fx, files) = files_mail();
        srv.put(7, fx.clone());
        srv.put(8, fx.clone());
        let gate = Arc::new(Gate { hold: AtomicBool::new(false) });
        let port = rt.block_on(relay(srv.port, gate.clone()));
        let dir = tempfile::tempdir().unwrap();
        let (core, events) = open_core(dir.path());
        let account = core.store().add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        core.store().insert_messages(vec![row(&account, 7, fx.raw.len() as u64), row(&account, 8, fx.raw.len() as u64)]).unwrap();
        core.set_body_source(&account, Arc::new(adapter(port)));
        let r = Rig { _rt: rt, srv, gate, dir, core, events, account, files };
        r.core.prefetch_bodies(&r.account, "INBOX");
        wait_until("the rows of both messages", WAIT, || r.core.settle_bodies(Duration::from_millis(1)) && r.core.attachments(r.id(7)).len() == 3 && r.core.attachments(r.id(8)).len() == 3);
        assert!(r.core.attachments(r.id(8)).iter().all(|a| a.local_path.is_none()), "the prefetch keeps rows only");
        r
    }
    fn id(&self, uid: u32) -> String { format!("{}:INBOX:1:{uid}", self.account) }
    /// The file whose name satisfies `pick`, of message `uid`.
    fn file(&self, uid: u32, pick: impl Fn(&str) -> bool) -> AttachmentFile { self.core.attachments(self.id(uid)).into_iter().find(|a| pick(&a.file_name)).expect("the file is listed") }
    fn pdf(&self, uid: u32) -> AttachmentFile { self.file(uid, |n| n.starts_with("Facture")) }
    fn original(&self, name: &str) -> Vec<u8> { self.files.iter().find(|f| f.name == name).unwrap().bytes.clone() }
    fn transfer(&self, id: &str) -> String { self.core.state(format!("attachment/{id}/transfer")).unwrap_or_else(|| "none".into()) }
    fn download(&self, id: &str) -> Result<CommandOutcome, CoreError> { self.core.run(Command::DownloadAttachment { attachment_id: id.to_string() }) }
    /// `download` on a helper thread with a bounded wait: a command that waits for the network (the server's bytes are held) ends this test red in
    /// `ANSWER_WITHIN` with a sentence, never a test that waits for ever. The gate is let go first so the helper thread can end.
    fn download_bounded(&self, id: &str) -> Result<CommandOutcome, CoreError> {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| { let _ = tx.send(self.download(id)); });
            match rx.recv_timeout(ANSWER_WITHIN) {
                Ok(r) => r,
                Err(_) => {
                    self.gate.hold.store(false, Ordering::SeqCst);
                    panic!("DownloadAttachment did not answer within {ANSWER_WITHIN:?} while the server held its bytes: the command waits for the network instead of answering at once");
                }
            }
        })
    }
    fn events(&self) -> Vec<(String, String, AttachmentTransfer)> { self.events.0.lock().unwrap().clone() }
    fn part_fetches(&self) -> usize { self.srv.part_fetches().len() }
}

fn json(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap_or_else(|e| panic!("not json ({e}): {s}")) }

/// (1) A message with a file that is not on this Mac: the command returns at once, the state path reads downloading while the server is slow, then
/// on_this_mac, the event says so, one part was fetched, and the bytes are stored: they are read back after a restart of the core.
/// There is no moving figure: the contract's state has none (the Swift side shows "Fetching" with no percent).
/// Mutants: `download-answers-after-the-fetch` (run() waits for the fetch: red on AT_ONCE); `no-event` (the end emits nothing); `transfer-state-not-served`
/// (state() returns None); `file-not-kept` (fetch() ends Ok but the path is cleared).
#[test]
fn a_file_not_on_this_mac_downloads_in_the_background_and_is_there_after_a_restart() {
    let r = Rig::start();
    let pdf = r.pdf(8);
    assert_eq!(json(&r.transfer(&pdf.id)), json("{\"state\":\"not_on_this_mac\"}"), "before anything is asked");
    r.gate.hold.store(true, Ordering::SeqCst);
    let t0 = Instant::now();
    let out = r.download_bounded(&pdf.id).expect("the command is accepted");
    let took = t0.elapsed();
    assert!(took < AT_ONCE, "DownloadAttachment waited {took:?}");
    assert_eq!((out.undo_entry_id, out.created_id), (None, None));
    assert_eq!(json(&r.transfer(&pdf.id)), json("{\"state\":\"downloading\"}"), "the server is slow: coming now");
    assert!(r.events().is_empty(), "nothing has landed yet: {:?}", r.events());
    assert!(r.core.attachments(r.id(8)).iter().all(|a| a.local_path.is_none()));
    r.gate.hold.store(false, Ordering::SeqCst);
    wait_until("the file is on this Mac", WAIT, || json(&r.transfer(&pdf.id))["state"] == "on_this_mac");
    wait_until("the event", WAIT, || !r.events().is_empty());
    assert_eq!(r.events(), vec![(pdf.id.clone(), r.id(8), AttachmentTransfer::OnThisMac)]);
    let parts = r.srv.part_fetches();
    assert_eq!(parts.len(), 1, "{parts:?}");
    assert!(parts[0].contains("BODY.PEEK[2]"), "that one file's part only: {parts:?}");
    let here = r.file(8, |n| n.starts_with("Facture"));
    assert_eq!(std::fs::read(here.local_path.as_ref().unwrap()).unwrap(), r.original(&pdf.file_name), "the file on this Mac is the original, byte for byte");
    // the same command again: it is a file on this Mac, nothing starts
    r.download(&pdf.id).expect("idempotent");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!((r.part_fetches(), r.events().len()), (1, 1), "nothing was fetched or announced the second time");
    // a restart of the core: the file and its place are still there, read with no server at all
    let Rig { _rt, dir, core, files, account, srv: _srv, .. } = r;
    drop(core);
    let (core, _events) = open_core(dir.path());
    let id = format!("{account}:INBOX:1:8");
    let back = core.attachments(id.clone()).into_iter().find(|a| a.file_name == pdf.file_name).unwrap();
    assert_eq!(back.local_path, here.local_path, "the same place");
    let original = files.iter().find(|f| f.name == pdf.file_name).unwrap().bytes.clone();
    assert_eq!(std::fs::read(back.local_path.as_ref().unwrap()).unwrap(), original, "the bytes survive the restart");
    assert_eq!(json(&core.state(format!("attachment/{}/transfer", pdf.id)).unwrap()), json("{\"state\":\"on_this_mac\"}"));
    drop(core);
}

/// (2) A second command while one runs does not fetch twice: eight commands at once on one file, the server holding its answer, give one fetch, one event.
/// Mutants: `running-ignored` (plan_download_attachment is given `false` for "already coming": two tasks, two events); `two-commands-two-claims` (the claim is
/// taken after the plan outside the lock: same).
#[test]
fn a_second_command_while_one_runs_does_not_fetch_twice() {
    let r = Rig::start();
    let pdf = r.pdf(8);
    r.gate.hold.store(true, Ordering::SeqCst);
    std::thread::scope(|s| {
        for _ in 0..8 { s.spawn(|| { r.download(&pdf.id).expect("accepted"); }); }
    });
    assert_eq!(json(&r.transfer(&pdf.id)), json("{\"state\":\"downloading\"}"));
    r.download(&pdf.id).expect("a ninth, while it still runs, is not an error");
    r.gate.hold.store(false, Ordering::SeqCst);
    wait_until("the file lands", WAIT, || json(&r.transfer(&pdf.id))["state"] == "on_this_mac");
    wait_until("the event", WAIT, || !r.events().is_empty());
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(r.part_fetches(), 1, "one fetch for nine commands: {:?}", r.srv.part_fetches());
    assert_eq!(r.events().len(), 1, "one event for nine commands: {:?}", r.events());
}

/// A file over ATTACHMENT_KEEP_MAX_BYTES is refused in words before any fetch, and an unknown file is NotFound (the contract's plan).
/// Mutants: `size-cap-ignored` (plan skipped for size); `unknown-file-ok` (an unknown id answers Ok).
#[test]
fn a_file_over_the_limit_is_refused_in_words_and_an_unknown_file_is_not_found() {
    let r = Rig::start();
    let pdf = r.pdf(8);
    let id = pdf.id.clone();
    r.core.store().write(move |ctx| { ctx.tx.execute("UPDATE attachment SET size_bytes = ?2 WHERE id = ?1", rusqlite::params![id, ATTACHMENT_KEEP_MAX_BYTES as i64 + 1])?; Ok(()) }).unwrap();
    match r.download(&pdf.id) {
        Err(CoreError::Invalid { field, reason_words }) => { assert_eq!(field, "attachment_too_large_to_keep"); assert!(reason_words.contains("too large"), "{reason_words}"); }
        other => panic!("expected the refusal in words, got {other:?}"),
    }
    assert_eq!(r.part_fetches(), 0, "nothing was fetched");
    assert!(matches!(r.download("no-such:att:000"), Err(CoreError::NotFound { .. })));
    assert_eq!(r.core.state("attachment/no-such:att:000/transfer".into()), None, "no state for a file this core does not know");
}

/// (3a) A server that fails (it holds the message but not that part) ends in a failed state in words, the event says so, no file is kept, and a retry
/// works once the server has the part.
/// Mutants: `failure-not-recorded` (the end of the task forgets the words: the state reads not_on_this_mac); `running-kept-after-failure` (running is not
/// cleared on failure: the retry answers "already coming" and nothing starts).
#[test]
fn a_server_that_fails_ends_in_a_failed_state_in_words_and_a_retry_works() {
    let r = Rig::start();
    let pdf = r.pdf(8);
    let good = r.srv.files.lock().unwrap().get(&8).cloned().unwrap();
    let mut broken = good.clone();
    broken.sections.retain(|(k, _)| k != "2");
    r.srv.put(8, broken);
    r.download(&pdf.id).expect("accepted: the failure comes later, in the state");
    wait_until("the failed state", WAIT, || json(&r.transfer(&pdf.id))["state"] == "failed");
    let failed = json(&r.transfer(&pdf.id));
    let words = failed["reason_words"].as_str().expect("words").to_string();
    assert!(words.trim().len() > 10, "in words: {words:?}");
    assert!(!words.contains("Error") && !words.contains("{"), "no raw error dump: {words:?}");
    wait_until("the event", WAIT, || !r.events().is_empty());
    assert_eq!(r.events(), vec![(pdf.id.clone(), r.id(8), AttachmentTransfer::Failed { reason_words: words.clone() })]);
    assert!(r.file(8, |n| n.starts_with("Facture")).local_path.is_none(), "no file is kept");
    assert_eq!(json(&r.transfer(&pdf.id)), failed, "a read changes nothing: still failed, same words");
    // the server has the part again: asking again works (it is a new fetch: the old failure is hidden while it runs)
    r.srv.put(8, good);
    r.gate.hold.store(true, Ordering::SeqCst);
    r.download(&pdf.id).expect("the retry is accepted");
    assert_eq!(json(&r.transfer(&pdf.id)), json("{\"state\":\"downloading\"}"), "a new fetch hides the last failure");
    r.gate.hold.store(false, Ordering::SeqCst);
    wait_until("on this Mac", WAIT, || json(&r.transfer(&pdf.id))["state"] == "on_this_mac");
    wait_until("the second event", WAIT, || r.events().len() == 2);
    assert_eq!(r.events()[1], (pdf.id.clone(), r.id(8), AttachmentTransfer::OnThisMac));
    let here = r.file(8, |n| n.starts_with("Facture"));
    assert_eq!(std::fs::read(here.local_path.unwrap()).unwrap(), r.original(&pdf.file_name));
}

/// (3b) A server that goes silent (the relay holds every byte back) ends in a failed state in words once the adapter's own limit runs out
/// (IMAP_COMMAND_TIMEOUT_MS, 30 s: this test takes about that long), and a retry works when the server answers again.
/// Mutant: `fetch-never-ends` (the task awaits `pending()` instead of the fetch: the state stays downloading and this test is red at its wait).
#[test]
fn a_server_that_goes_silent_ends_in_a_failed_state_in_words_and_a_retry_works() {
    let r = Rig::start();
    let csv = r.file(8, |n| n.ends_with(".csv"));
    r.gate.hold.store(true, Ordering::SeqCst);
    let t0 = Instant::now();
    r.download_bounded(&csv.id).expect("accepted at once");
    assert!(t0.elapsed() < AT_ONCE, "waited {:?}", t0.elapsed());
    assert_eq!(json(&r.transfer(&csv.id)), json("{\"state\":\"downloading\"}"));
    wait_until("the failed state after the silence", SILENCE_WAIT, || json(&r.transfer(&csv.id))["state"] == "failed");
    let words = json(&r.transfer(&csv.id))["reason_words"].as_str().unwrap().to_string();
    assert!(words.contains("did not answer") || words.contains("server"), "the words say the server was silent: {words:?}");
    assert!(r.core.attachments(r.id(8)).iter().all(|a| a.local_path.is_none()));
    // the server answers again: a retry lands the file
    r.gate.hold.store(false, Ordering::SeqCst);
    r.download(&csv.id).expect("retry accepted");
    wait_until("the retry lands", WAIT, || json(&r.transfer(&csv.id))["state"] == "on_this_mac");
    let here = r.file(8, |n| n.ends_with(".csv"));
    assert_eq!(std::fs::read(here.local_path.unwrap()).unwrap(), r.original(&csv.file_name), "the csv (quoted-printable) decoded as the sender made it");
}

/// (4) An inline logo is not listed as a file to download: the row reads inline (from its column) with its content id, `gallery_attachments` leaves it out,
/// and downloading everything the gallery lists fetches the two real files only, never part 1.2.
/// Mutant: `inline-flag-lost` (put_rows writes inline 0 and JSON false: the logo is listed, and its download is attempted).
#[test]
fn an_inline_logo_is_not_listed_as_a_file_to_download() {
    let r = Rig::start();
    let all = r.core.attachments(r.id(8));
    assert_eq!(all.len(), 3);
    let logo = all.iter().find(|a| a.file_name == "logo.png").unwrap();
    assert!(logo.inline && logo.content_id.as_deref() == Some("logo@files.example"), "{logo:?}");
    let listed = gallery_attachments(all.clone());
    assert_eq!(listed.iter().map(|a| a.file_name.clone()).collect::<Vec<_>>(), all.iter().filter(|a| a.file_name != "logo.png").map(|a| a.file_name.clone()).collect::<Vec<_>>());
    for f in &listed { r.download(&f.id).expect("accepted"); }
    wait_until("both files", WAIT, || r.core.attachments(r.id(8)).iter().filter(|a| a.local_path.is_some()).count() == 2);
    let parts = r.srv.part_fetches();
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert!(parts.iter().all(|l| !l.contains("BODY.PEEK[1.2]")), "the logo was never fetched: {parts:?}");
    assert!(r.core.attachments(r.id(8)).iter().find(|a| a.file_name == "logo.png").unwrap().local_path.is_none());
}

/// (5a) The four attachment columns are WRITTEN as columns (part_path, transfer_encoding, content_id, inline), and the JSON in `remote_id` still holds them.
/// Mutant: `columns-not-written` (put_rows writes the JSON only, as before this lane).
#[test]
fn the_four_columns_are_written_as_columns() {
    let r = Rig::start();
    let id = r.id(8);
    let rows: Vec<(String, Option<String>, Option<String>, Option<String>, i64, Option<String>)> = r.core.store().write(move |ctx| {
        let mut st = ctx.tx.prepare("SELECT file_name, part_path, transfer_encoding, content_id, inline, json_extract(remote_id, '$.part') FROM attachment WHERE message_id = ?1 ORDER BY id")?;
        let v = st.query_map([&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }).unwrap();
    // what the fixture's author wrote: the logo is part 1.2 in base64 with its content id, the pdf part 2 in base64, the csv part 3 quoted-printable
    let want: Vec<(&str, &str, &str, Option<&str>, i64)> = vec![("logo.png", "1.2", "base64", Some("logo@files.example"), 1), ("Facture \u{e9}t\u{e9} n\u{b0}12.pdf", "2", "base64", None, 0), ("Rapport d\u{e9}c.csv", "3", "quoted-printable", None, 0)];
    assert_eq!(rows.len(), 3, "{rows:?}");
    for (got, w) in rows.iter().zip(&want) {
        assert_eq!((got.0.as_str(), got.1.as_deref(), got.2.as_deref(), got.3.as_deref(), got.4), (w.0, Some(w.1), Some(w.2), w.3, w.4), "the columns of {}", w.0);
        assert_eq!(got.5.as_deref(), Some(w.1), "the JSON of {} still holds the part", w.0);
    }
}

/// (5b) An old row (written before the columns: the four only as JSON in `remote_id`, the columns empty) still reads: its part and encoding for the fetch,
/// its inline flag and content id for the gallery, and its file downloads.
/// Mutant: `no-json-fallback` (the reads take the columns only).
#[test]
fn an_old_row_with_the_json_only_still_reads_and_downloads() {
    let r = Rig::start();
    let (fx, _) = files_mail();
    r.srv.put(9, fx.clone());
    r.core.store().insert_messages(vec![row(&r.account, 9, fx.raw.len() as u64)]).unwrap();
    let m9 = r.id(9);
    r.core.store().write(move |ctx| {
        for (n, (name, mime, size, json)) in [
            ("logo.png", "image/png", 72, "{\"part\":\"1.2\",\"enc\":\"base64\",\"cid\":\"logo@files.example\",\"inline\":true}"),
            ("old.pdf", "application/pdf", 40000, "{\"part\":\"2\",\"enc\":\"base64\",\"cid\":null,\"inline\":false}"),
        ].into_iter().enumerate() {
            ctx.tx.execute("INSERT INTO attachment (id, message_id, remote_id, file_name, mime_type, size_bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![format!("{m9}:att:{n:03}"), m9, json, name, mime, size])?;
        }
        Ok(())
    }).unwrap();
    let files = r.core.attachments(r.id(9));
    assert_eq!(files.len(), 2);
    assert!(files[0].inline && files[0].content_id.as_deref() == Some("logo@files.example"), "{:?}", files[0]);
    assert!(!files[1].inline && files[1].content_id.is_none(), "{:?}", files[1]);
    assert_eq!(gallery_attachments(files.clone()).len(), 1, "the old logo is left out of the gallery");
    r.download(&files[1].id).expect("accepted");
    wait_until("the old row's file lands", WAIT, || json(&r.transfer(&files[1].id))["state"] == "on_this_mac");
    let parts = r.srv.part_fetches();
    assert!(parts.len() == 1 && parts[0].contains("BODY.PEEK[2]"), "fetched by the part the JSON names: {parts:?}");
    let got = r.core.attachments(r.id(9)).into_iter().find(|a| a.file_name == "old.pdf").unwrap();
    assert_eq!(std::fs::read(got.local_path.unwrap()).unwrap().len(), 40_000, "decoded with the JSON's encoding");
}

// ------------------------------------------------------------------------------------------------------------------------------------------------------
// round 2: the text of a downloaded file joins the indexes (`attachment.extracted_text`, `attachment_content`, `chunk` rows `attachment:<id>`)

const PLAIN_PDF: &[u8] = include_bytes!("../../attachment-index/tests/fixtures/plain.pdf");
const SCAN_PDF: &[u8] = include_bytes!("../../attachment-index/tests/fixtures/scan.pdf");
/// What the attachment-index lane's own test says `plain.pdf` reads as (tests/pdf_text.rs): the fixture author's words, not this code's.
const PLAIN_PDF_TEXT: &str = "Quarterly brochure quote\nHeavy matte stock, 170 g, two colours\nUnit price 0.84 EUR for 12,000 copies";

fn wrap76(b64: &str) -> String { b64.as_bytes().chunks(76).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join("\r\n") }

/// A message with a text body (part 1) and these files (parts 2, 3, ...): (name, media type, transferred body).
fn mail_with(files: &[(&str, &str, String, bool)]) -> server::Fixture {
    let mut raw = String::from("From: Compta <compta@files.example>\r\nTo: owner@example.test\r\nSubject: Devis\r\nMessage-ID: <devis@files.example>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"b\"\r\n\r\n--b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nVoir le devis.\r\n");
    let mut sections = Vec::new();
    for (i, (name, mime, body, b64)) in files.iter().enumerate() {
        let enc = if *b64 { "base64" } else { "7bit" };
        raw += &format!("--b\r\nContent-Type: {mime}; name=\"{name}\"\r\nContent-Transfer-Encoding: {enc}\r\nContent-Disposition: attachment; filename=\"{name}\"\r\n\r\n{body}\r\n");
        sections.push(((i + 2).to_string(), body.clone().into_bytes()));
    }
    raw += "--b--\r\n";
    server::Fixture { raw: raw.into_bytes(), sections }
}

fn b64(bytes: &[u8]) -> String { use base64::Engine; wrap76(&base64::engine::general_purpose::STANDARD.encode(bytes)) }

impl Rig {
    /// Message `uid` held by the server and stored with its preview only, then read by the prefetch (rows, no files).
    fn add_mail(&self, uid: u32, fx: server::Fixture, files: usize) {
        self.srv.put(uid, fx.clone());
        self.core.store().insert_messages(vec![row(&self.account, uid, fx.raw.len() as u64)]).unwrap();
        self.core.prefetch_bodies(&self.account, "INBOX");
        wait_until("the rows of the added mail", WAIT, || self.core.settle_bodies(Duration::from_millis(1)) && self.core.attachments(self.id(uid)).len() == files);
    }
    fn named(&self, uid: u32, name: &str) -> AttachmentFile { self.file(uid, |n| n == name) }
    /// (extracted_text, attachment_content.text) of one file.
    fn texts(&self, id: &str) -> (Option<String>, Option<String>) {
        let id = id.to_string();
        self.core.store().write(move |ctx| {
            let t: Option<String> = ctx.tx.query_row("SELECT extracted_text FROM attachment WHERE id = ?1", [&id], |r| r.get(0))?;
            let c: Option<String> = ctx.tx.query_row("SELECT (SELECT text FROM attachment_content WHERE attachment_id = ?1)", [&id], |r| r.get(0))?;
            Ok((t, c))
        }).unwrap()
    }
    fn chunks_of(&self, id: &str) -> Vec<(i64, String, bool)> { self.core.store().attachment_chunks(id).unwrap() }
    fn log_lines(&self) -> String { std::fs::read_to_string(self.dir.path().join("core.log")).unwrap_or_default() }
}

/// (6) After a download ends, the file's text is stored under its message: `attachment.extracted_text` (the word index reads it), `attachment_content`, and
/// `chunk` rows `attachment:<id>` carrying the message's subject, sender and folder; the body chunks are not touched. Asking again fetches nothing and
/// writes the chunks no second time (same ids). A PDF is read through `extract_pdf_text`.
/// Mutants: `text-not-filed` (the entry is never saved), `chunks-not-written` (only the entry), `second-command-refetches` (a file on this Mac is treated as absent).
#[test]
fn the_text_of_a_downloaded_file_is_stored_under_its_message() {
    let r = Rig::start();
    r.add_mail(11, mail_with(&[("devis.pdf", "application/pdf", b64(PLAIN_PDF), true)]), 1);
    let pdf = r.named(11, "devis.pdf");
    assert_eq!(r.texts(&pdf.id), (None, None), "nothing is read before the download");
    assert!(r.chunks_of(&pdf.id).is_empty());
    r.download_bounded(&pdf.id).expect("accepted");
    wait_until("the chunks of the file", WAIT, || !r.chunks_of(&pdf.id).is_empty());
    let (extracted, content) = r.texts(&pdf.id);
    assert_eq!(extracted.as_deref(), Some(PLAIN_PDF_TEXT), "the text the PDF reader gives, in the word index's column");
    assert_eq!(content.as_deref(), Some(PLAIN_PDF_TEXT), "and in attachment_content");
    let chunks = r.chunks_of(&pdf.id);
    assert_eq!(chunks.len(), 1, "a short text is one chunk: {chunks:?}");
    assert_eq!(chunks[0].1, PLAIN_PDF_TEXT, "cut by the indexer's own rule (a short text is one window)");
    assert!(!chunks[0].2, "no vector yet: embedding is the indexer's");
    let (chunk, account) = r.core.store().background_reads().unwrap().chunk(chunks[0].0).unwrap().expect("the chunk reads back");
    assert_eq!((chunk.message_id.as_str(), account.as_str()), (r.id(11).as_str(), r.account.as_str()));
    assert_eq!(chunk.part, ChunkPart::Attachment { attachment_id: pdf.id.clone() });
    assert_eq!((chunk.subject.as_str(), chunk.folder.as_str()), ("Facture 11", "INBOX"), "the message's own facts ride with it");
    assert!(r.core.store().background_reads().unwrap().body_chunks(&r.id(11)).unwrap().is_empty(), "the body's chunks are not this file's");
    assert_eq!(r.part_fetches(), 1);
    // again: nothing is fetched, nothing is rewritten
    r.download_bounded(&pdf.id).expect("idempotent");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(r.part_fetches(), 1, "not fetched twice: {:?}", r.srv.part_fetches());
    assert_eq!(r.chunks_of(&pdf.id).iter().map(|c| c.0).collect::<Vec<_>>(), chunks.iter().map(|c| c.0).collect::<Vec<_>>(), "the chunks were not written again");
}

/// (7) A file with no text (a scanned PDF, a kind only the OS reads) stores nothing: no text, no content row, no chunk. It says so in core.log in one line, and
/// its transfer state is on_this_mac, never failed.
/// Mutants: `empty-text-filed` (an empty reading is filed as a text), `no-log-line` (the reason is not logged).
#[test]
fn a_file_with_no_text_stores_nothing_and_says_so_in_the_log() {
    let r = Rig::start();
    r.add_mail(11, mail_with(&[("scan.pdf", "application/pdf", b64(SCAN_PDF), true)]), 1);
    let scan = r.named(11, "scan.pdf");
    r.download_bounded(&scan.id).expect("accepted");
    wait_until("the file lands", WAIT, || json(&r.transfer(&scan.id))["state"] == "on_this_mac");
    // the reading ends in the log line, or (wrongly) in a stored text: wait for either, then judge
    wait_until("the reading of the file ends", WAIT, || r.log_lines().contains(&format!("attachment {}: no text stored", scan.id)) || r.texts(&scan.id) != (None, None) || !r.chunks_of(&scan.id).is_empty());
    assert_eq!(r.texts(&scan.id), (None, None), "no text, no content row");
    assert!(r.chunks_of(&scan.id).is_empty(), "no chunk");
    wait_until("the one log line", WAIT, || r.log_lines().contains(&format!("attachment {}: no text stored", scan.id)));
    let lines: Vec<String> = r.log_lines().lines().filter(|l| l.contains(&scan.id)).map(str::to_string).collect();
    assert_eq!(lines.len(), 1, "one line, not a story: {lines:?}");
    assert!(lines[0].contains("PDF"), "it names why: {}", lines[0]);
    assert_eq!(json(&r.transfer(&scan.id)), json("{\"state\":\"on_this_mac\"}"), "no error to the user");
    assert!(r.events().iter().all(|e| e.2 == AttachmentTransfer::OnThisMac), "{:?}", r.events());
}

/// (8) The text of one file is cut at ATTACHMENT_TEXT_MAX_CHARS characters (a file bigger than that is never read whole).
/// Mutant: `cap-ignored` (no cap).
#[test]
fn the_text_of_a_big_file_is_cut_at_the_ledgers_limit() {
    let r = Rig::start();
    let big = "mot ".repeat(ATTACHMENT_TEXT_MAX_CHARS as usize / 4 + 5_000);
    assert!(big.chars().count() > ATTACHMENT_TEXT_MAX_CHARS as usize);
    r.add_mail(11, mail_with(&[("long.txt", "text/plain", big, false)]), 1);
    let f = r.named(11, "long.txt");
    r.download_bounded(&f.id).expect("accepted");
    wait_until("the chunks", WAIT, || !r.chunks_of(&f.id).is_empty());
    let (extracted, content) = r.texts(&f.id);
    assert_eq!(extracted.unwrap().chars().count(), ATTACHMENT_TEXT_MAX_CHARS as usize, "cut at the ledger's number");
    assert_eq!(content.unwrap().chars().count(), ATTACHMENT_TEXT_MAX_CHARS as usize);
    let words: usize = r.chunks_of(&f.id).iter().map(|c| c.1.split_whitespace().count()).sum();
    assert!(words >= ATTACHMENT_TEXT_MAX_CHARS as usize / 4, "the chunks cover the kept text: {words} words");
}

/// (9) The word and the folder progress count body chunks only: a message whose file chunk has no vector is still "indexed" when its body is.
/// Mutant: `progress-counts-file-chunks` (the progress SQL asks every part).
#[test]
fn a_files_unembedded_chunk_does_not_hold_the_folder_progress_back() {
    let r = Rig::start();
    r.add_mail(11, mail_with(&[("long.txt", "text/plain", "un deux trois".to_string(), false)]), 1);
    let id = r.id(11);
    let f = r.named(11, "long.txt");
    // a body with its chunk embedded, then a file chunk with no vector
    r.core.store().write({ let id = id.clone(); move |ctx| { ctx.tx.execute("UPDATE message SET body_text = 'Voir le devis.' WHERE id = ?1", [&id])?; Ok(()) } }).unwrap();
    r.core.store().replace_body_chunks(&id, vec!["Voir le devis.".into()]).unwrap();
    let body_ids: Vec<i64> = r.core.store().background_reads().unwrap().body_chunks(&id).unwrap().iter().map(|c| c.0).collect();
    r.core.store().mark_chunks_embedded(body_ids, 1).unwrap();
    r.core.store().replace_attachment_chunks(&f.id, vec!["un deux trois".into()]).unwrap();
    assert_eq!(r.chunks_of(&f.id).len(), 1);
    let progress = r.core.store().background_reads().unwrap().folder_progress().unwrap();
    let inbox = progress.iter().find(|p| p.account_id == r.account && p.folder == "INBOX").unwrap();
    assert_eq!((inbox.bodies, inbox.indexed), (3, 1), "of the three bodies, the one with its body chunk embedded is indexed though its file chunk has no vector: {inbox:?}");
}
