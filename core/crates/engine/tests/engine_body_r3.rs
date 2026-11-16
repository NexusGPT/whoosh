//! body-r3: the files of a received message. Over the production core of the hidden test copy and the scripted IMAP server (tests/body_r2_server:
//! 127.0.0.1 only): when a body is fetched and parsed each attachment part becomes an `attachment` row; the reader's own fetch keeps the files, the
//! background fetches keep rows only; a file not on this Mac is fetched by its part number on demand (`Bodies::attachment_bytes`, the one function);
//! mail stored before is read again once; a message takes its rows (and, at the next start, its files) with it.
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/body_r2_server/mod.rs"]
mod server;

#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
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
use server::{files_mail, FixtureFile, Server};
use store::{AttachmentIn, NewMessage, Store};

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(20);
const AT_ONCE: Duration = Duration::from_millis(50);

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, uid: u32, size: u64, body_text: Option<&str>) -> NewMessage {
    let id = format!("{account}:INBOX:1:{uid}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{account}:<{uid}@thread>"), account_id: account.into(), sender_name: "Compta".into(), from_address: "compta@files.example".into(),
            to: vec![USER.into()], subject: format!("Facture {uid}"), preview: "Voir la facture".into(), received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: body_text.map(str::to_string), body_html: None, has_list_header: false, size_bytes: Some(size as i64), tab_id: None,
    }
}

struct Rig { _rt: tokio::runtime::Runtime, srv: Server, dir: tempfile::TempDir, core: RustCore, account: String, files: Vec<FixtureFile> }

impl Rig {
    /// A core with one account whose messages 7 and 8 are stored with their preview only; the server holds `files_mail` under both.
    fn start() -> Rig {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let srv = rt.block_on(Server::start(&[], 1));
        let (fx, files) = files_mail();
        srv.put(7, fx.clone());
        srv.put(8, fx.clone());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("WHOOSH_ATTACH_AUTO_FETCH", "off"); // this file proves the fetch ON A CLICK: the automatic fetch (attachment_auto.rs) is off here
        let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
        let store = core.store();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![row(&account, 7, fx.raw.len() as u64, None), row(&account, 8, fx.raw.len() as u64, None)]).unwrap();
        core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
        Rig { _rt: rt, srv, dir, core, account, files }
    }
    fn id(&self, uid: u32) -> String { format!("{}:INBOX:1:{uid}", self.account) }
    fn store(&self) -> Arc<Store> { self.core.store() }
    /// (name, type, size, inline, content id, part) of each row of a message, by id.
    fn rows(&self, uid: u32) -> Vec<(String, String, i64, Option<i64>, Option<String>, Option<String>)> {
        let id = self.id(uid);
        self.store().write(move |ctx| {
            let mut st = ctx.tx.prepare("SELECT file_name, mime_type, size_bytes, json_extract(remote_id, '$.inline'), json_extract(remote_id, '$.cid'), json_extract(remote_id, '$.part') FROM attachment WHERE message_id = ?1 ORDER BY id")?;
            let v = st.query_map([&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(v)
        }).unwrap()
    }
    fn count_rows(&self) -> i64 { self.store().write(|ctx| Ok(ctx.tx.query_row("SELECT count(*) FROM attachment", [], |r| r.get(0))?)).unwrap() }
    fn local_paths(&self, uid: u32) -> Vec<Option<String>> { self.core.attachments(self.id(uid)).into_iter().map(|a| a.local_path).collect() }
}

/// The reader opens a message: two attachments and one inline image give three rows with the right names (accents, spaces), types, sizes, inline
/// flag, content id and part number, in the message's own order, and the reader's fetch keeps all three files on this Mac, byte for byte.
/// Mutants: names not read (placeholder), inline flag lost, part numbers off by one, files not kept for the reader.
#[test]
fn a_fetched_message_gives_one_row_per_file_and_the_readers_fetch_keeps_the_files() {
    let r = Rig::start();
    let _ = r.core.body(r.id(7));
    wait_until("three rows with their files", || r.core.attachments(r.id(7)).len() == 3 && r.local_paths(7).iter().all(Option::is_some));
    let rows = r.rows(7);
    assert_eq!(rows.len(), 3, "{rows:?}");
    for (got, want) in rows.iter().zip(&r.files) {
        assert_eq!((got.0.as_str(), got.1.as_str(), got.2), (want.name, want.mime, want.bytes.len() as i64), "{got:?}");
        assert_eq!(got.3, Some(i64::from(want.inline)), "the inline flag of {}", want.name);
        assert_eq!(got.4.as_deref(), want.cid, "the content id of {}", want.name);
        assert_eq!(got.5.as_deref(), Some(want.section), "the part of {}", want.name);
    }
    let listed = r.core.attachments(r.id(7));
    for (a, want) in listed.iter().zip(&r.files) {
        assert_eq!((a.file_name.as_str(), a.mime_type.as_str(), a.size_bytes), (want.name, want.mime, want.bytes.len() as u64));
        assert_eq!(std::fs::read(a.local_path.as_ref().unwrap()).unwrap(), want.bytes, "the file {} on this Mac is the original", want.name);
    }
    // the reader header's names: read from the same rows, files only (a picture shown in the text is not a file; owner's picture 212)
    let mut names = r.store().message(&r.id(7)).unwrap().unwrap().attachment_names;
    names.sort();
    let mut want: Vec<String> = r.files.iter().filter(|f| !f.inline).map(|f| f.name.to_string()).collect();
    want.sort();
    assert_eq!(names, want);
}

/// A background fetch (the prefetch: nobody is looking at the message) writes the rows and no file. The view's read answers at once with
/// "not on this Mac yet" (no local path); asking for the file in the background fetches ONE part and keeps it; `attachment_bytes` is that same
/// fetch, answering the original bytes, and the second call reads the file (no second part fetch). Mutants: files kept by a background fetch;
/// the fetched file not recorded; the part not decoded.
#[test]
fn a_file_not_on_this_mac_is_fetched_by_its_part_once_and_kept() {
    let r = Rig::start();
    r.core.prefetch_bodies(&r.account, "INBOX");
    wait_until("both bodies and their rows", || r.core.settle_bodies(Duration::from_millis(1)) && r.core.attachments(r.id(8)).len() == 3 && r.core.attachments(r.id(7)).len() == 3);
    assert_eq!(r.local_paths(8), vec![None, None, None], "the prefetch keeps rows only");
    let t0 = Instant::now();
    let listed = r.core.attachments(r.id(8));
    assert!(t0.elapsed() < AT_ONCE, "the read waited {:?}", t0.elapsed());
    assert!(listed.iter().all(|a| a.local_path.is_none()));
    assert!(r.srv.part_fetches().is_empty(), "no part fetched yet: {:?}", r.srv.part_fetches());
    // the view asks in the background: it returns at once, the file lands after
    let pdf = listed.iter().find(|a| a.file_name.starts_with("Facture")).unwrap().clone();
    let t0 = Instant::now();
    r.core.background().request_attachment(&pdf.id);
    assert!(t0.elapsed() < AT_ONCE, "request_attachment waited {:?}", t0.elapsed());
    wait_until("the pdf is on this Mac", || r.local_paths(8).iter().filter(|p| p.is_some()).count() == 1);
    assert_eq!(r.srv.part_fetches().len(), 1, "{:?}", r.srv.part_fetches());
    assert!(r.srv.part_fetches()[0].contains("BODY.PEEK[2]"), "{:?}", r.srv.part_fetches());
    // the one function, for the two others: fetched by part, equal to the original; again: from the file
    let rt = tokio::runtime::Runtime::new().unwrap();
    let csv = listed.iter().find(|a| a.file_name.ends_with(".csv")).unwrap();
    let want = |name: &str| r.files.iter().find(|f| f.name == name).unwrap().bytes.clone();
    assert_eq!(rt.block_on(r.core.background().attachment_bytes(&csv.id)).unwrap(), want(&csv.file_name), "the csv (quoted-printable) as the sender made it");
    assert_eq!(rt.block_on(r.core.background().attachment_bytes(&pdf.id)).unwrap(), want(&pdf.file_name), "the pdf (base64), from its file");
    assert_eq!(r.srv.part_fetches().len(), 2, "the pdf was not fetched again: {:?}", r.srv.part_fetches());
    assert_eq!(rt.block_on(r.core.background().attachment_bytes(&csv.id)).unwrap(), want(&csv.file_name));
    assert_eq!(r.srv.part_fetches().len(), 2, "the csv is a file now: {:?}", r.srv.part_fetches());
    let paths = r.local_paths(8);
    assert_eq!(paths.iter().filter(|p| p.is_some()).count(), 2, "{paths:?}");
    assert!(rt.block_on(r.core.background().attachment_bytes("no-such-attachment")).is_err());
}

/// The same message read again writes no second row, keeps the file's place, and a shorter list drops the extra row only.
/// Mutants: a new id at each reading; the extra row kept.
#[test]
fn a_message_read_again_writes_no_duplicate_row() {
    let r = Rig::start();
    let _ = r.core.body(r.id(7));
    wait_until("rows and files", || r.local_paths(7).len() == 3 && r.local_paths(7).iter().all(Option::is_some));
    let first: Vec<String> = r.core.attachments(r.id(7)).into_iter().map(|a| a.id).collect();
    let before = r.local_paths(7);
    // read again: the reader's own ask, whatever the stored body says
    let parts = r.store().body_parts(&r.id(7)).unwrap().unwrap();
    r.core.background().request(&parts);
    wait_until("the second reading landed", || r.core.settle_bodies(Duration::from_millis(1)));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.core.attachments(r.id(7)).into_iter().map(|a| a.id).collect::<Vec<_>>(), first, "the same three ids");
    assert_eq!(r.count_rows(), 3, "no duplicate");
    assert_eq!(r.local_paths(7), before, "the files' places are kept");
    // a reading with fewer files: the extra row goes, the others keep their file
    let two: Vec<AttachmentIn> = (0..2).map(|i| AttachmentIn { part: format!("{}", i + 1), name: format!("f{i}"), mime_type: "x/y".into(), size_bytes: 1, content_id: None, inline: false, encoding: "none".into() }).collect();
    r.store().put_attachment_rows(vec![(r.id(7), two)], true).unwrap();
    assert_eq!(r.count_rows(), 2);
    assert_eq!(r.local_paths(7), before[..2].to_vec());
}

/// A deleted message leaves no row, and its files go at the next start.
/// Mutants: the rows do not follow the message (the foreign key's cascade removed); the sweep of orphan files off.
#[test]
fn a_deleted_message_leaves_no_row_and_no_file() {
    let r = Rig::start();
    let _ = r.core.body(r.id(7));
    wait_until("rows and files", || r.local_paths(7).len() == 3 && r.local_paths(7).iter().all(Option::is_some));
    let folder = std::path::Path::new(r.local_paths(7)[0].as_ref().unwrap()).parent().unwrap().to_path_buf();
    assert!(folder.exists());
    let id = r.id(7);
    r.store().write(move |ctx| { ctx.tx.execute("DELETE FROM message WHERE id = ?1", [&id])?; Ok(()) }).unwrap();
    assert_eq!(r.count_rows(), 0, "the rows went with the message");
    let Rig { dir, core, .. } = r;
    drop(core);
    std::env::set_var("WHOOSH_ATTACH_AUTO_FETCH", "off"); // this file proves the fetch ON A CLICK: the automatic fetch (attachment_auto.rs) is off here
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens again");
    wait_until("the sweep removed the orphan files", || !folder.exists());
    drop(core);
}

/// Mail stored before this build has a body and no file rows, and no raw message to read them from: it is fetched again ONCE (rows only), the
/// ones that are big enough to carry a file. A small one is left alone; a second start fetches nothing.
/// Mutants: the once-marker ignored; the size gap ignored.
#[test]
fn mail_stored_before_is_read_again_once_for_its_file_rows() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    let (fx, _files) = files_mail();
    srv.put(7, fx.clone());
    srv.put(9, fx.clone());
    let dir = tempfile::tempdir().unwrap();
    let account = {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        // 7 is big against its stored text; 9 claims a small size (a plain mail)
        store.insert_messages(vec![row(&account, 7, fx.raw.len() as u64, Some("Voir la facture ci-jointe.")), row(&account, 9, 1200, Some("Voir la facture ci-jointe."))]).unwrap();
        account
    };
    let fetched = |srv: &Server| srv.fetches().len();
    std::env::set_var("WHOOSH_ATTACH_AUTO_FETCH", "off"); // this file proves the fetch ON A CLICK: the automatic fetch (attachment_auto.rs) is off here
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let id7 = format!("{account}:INBOX:1:7");
    wait_until("the rows of the big one", || core.attachments(id7.clone()).len() == 3);
    assert!(core.settle_bodies(WAIT));
    assert!(core.attachments(format!("{account}:INBOX:1:9")).is_empty(), "the small one is not read again");
    assert!(srv.fetches().iter().all(|l| !l.contains("FETCH 9 ")), "{:?}", srv.fetches());
    assert!(core.attachments(id7.clone()).iter().all(|a| a.local_path.is_none()), "rows only");
    let marker = std::fs::read_dir(dir.path()).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("attachment-backfill-"));
    wait_until("the once-marker", || std::fs::read_dir(dir.path()).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("attachment-backfill-")));
    let _ = marker;
    // a second start, rows gone (as if never written): nothing is fetched again
    let before = fetched(&srv);
    drop(core);
    {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        store.write(|ctx| { ctx.tx.execute("DELETE FROM attachment", [])?; Ok(()) }).unwrap();
    }
    std::env::set_var("WHOOSH_ATTACH_AUTO_FETCH", "off"); // this file proves the fetch ON A CLICK: the automatic fetch (attachment_auto.rs) is off here
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens again");
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    std::thread::sleep(Duration::from_millis(600));
    assert!(core.settle_bodies(WAIT));
    assert_eq!(fetched(&srv), before, "the marker keeps the pass from running again");
    assert!(core.attachments(id7).is_empty());
    drop(core);
}

/// Forward of a mail whose files are not on this Mac (the sync stored its rows, no bytes: the background fetch keeps rows only): sending it
/// fetches ONLY the parts of its files (`BODY.PEEK[1.2]`, `[2]`, `[3]`), never the whole message again, and every file reaches SMTP byte for byte.
/// Mutant: the forward path takes its files out of the whole message again (the call site of draft_files.rs `stored_bytes` switched back).
#[test]
fn a_forward_of_a_mail_whose_files_are_not_on_this_mac_fetches_only_their_parts() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    let (fx, files) = files_mail();
    imap.seed("INBOX", 1);
    let uid = imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(imap_fakes::Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw: fx.raw.clone(), gm: None });
        b.version += 1;
        for (k, bytes) in &fx.sections { s.parts.push((uid, k.clone(), bytes.clone())); }
        uid
    });
    let smtp = FakeSmtp::start(Mode::Xoauth2);
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(&db.path().join("whoosh.sqlite")).unwrap();
    let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector, TestTransport::PlainLoopback);
    let cfg = AccountConfig { imap_port: imap.port, smtp_port: smtp.port, ..config() };
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: cfg, password: SecretText { value: "app-password".into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(SyncPhase::Listening));
    let store = core.store();
    let mid = { let s = store.clone(); let subject = "Facture"; let mut found = None; wait_until("the mail is stored", || { found = s.write(move |c| Ok(c.tx.query_row("SELECT id FROM message WHERE subject = ?1 AND remote_id = ?2", rusqlite::params![subject, uid.to_string()], |r| r.get::<_, String>(0)).ok())).unwrap(); found.is_some() }); found.unwrap() };
    // the sync's own prefetch fetched its body: three rows, no file on this Mac
    wait_until("the rows", || core.attachments(mid.clone()).len() == 3);
    assert!(core.attachments(mid.clone()).iter().all(|a| a.local_path.is_none()));
    assert!(core.settle_bodies(WAIT));
    let wholes = |imap: &FakeImap| imap.log().iter().filter(|l| l.contains("BODY.PEEK[])")).count();
    let before = wholes(&imap);
    let draft = core.run(Command::Forward { message_id: mid.clone() }).expect("forward").created_id.unwrap();
    let (subject, body): (String, String) = { let d = draft.clone(); store.write(move |c| Ok(c.tx.query_row("SELECT subject, body FROM draft WHERE id = ?1", [&d], |r| Ok((r.get(0)?, r.get(1)?)))?)).unwrap() };
    core.run(Command::UpdateDraft { draft_id: draft.clone(), from_account_id: None, to: vec!["boss@example.test".into()], cc: vec![], bcc: vec![], subject, body }).expect("a recipient");
    let entry = core.run(Command::Send { draft_id: draft.clone() }).expect("queued").created_id.unwrap();
    core.run(Command::SendNow { outbox_id: entry }).expect("sent now");
    let got = smtp.received();
    assert_eq!(got.len(), 1, "{:?}", core.outbox().iter().map(|e| format!("{:?}", e.state)).collect::<Vec<_>>());
    for f in &files {
        assert_eq!(send::part_bytes(got[0].data.as_bytes(), f.name, 0).as_deref(), Some(&f.bytes[..]), "{} reached SMTP byte for byte", f.name);
    }
    let parts: Vec<String> = imap.log().into_iter().filter(|l| l.contains("BODY.PEEK[") && !l.contains("BODY.PEEK[]") && !l.contains("HEADER") && !l.contains("TEXT")).collect();
    assert_eq!(parts.len(), 3, "one fetch per file: {parts:?}");
    for sec in ["1.2", "2", "3"] { assert!(parts.iter().any(|l| l.contains(&format!("BODY.PEEK[{sec}]"))), "part {sec} asked: {parts:?}"); }
    assert_eq!(wholes(&imap), before, "the whole message was not fetched again");
    // and the files are on this Mac now
    assert!(core.attachments(mid).iter().all(|a| a.local_path.is_some()));
}
