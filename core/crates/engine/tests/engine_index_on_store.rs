//! index-queue (board item b0b590c7, "Hand every newly stored mail to the search index"): with a search model given, a mail stored by the
//! import of a mailbox, and the stored copy of a mail just sent, are each cut, embedded and found by the meaning search with no restart of
//! the engine (no second model hand-over, no reopen of the store). Before this lane both paths stored the mail's text and told no indexer,
//! so those mails waited for the backfill of the next launch (docs/lanes/stale-fix.md, "Findings outside Owns", point 1).
//! (a) the import fixture and run of import_listed_r2.rs; (b) the send rig of engine_compose_cmds.rs (the sync-imap fake and an SMTP
//! listener of this file, both on 127.0.0.1: nothing leaves this Mac). The embedder is a fake that hashes words (engine_stale_pieces.rs).
//! No mail text is printed beyond the made-up texts of this file.
//! Broken copies (red, docs/lanes/index-queue.md): (a) in import_run.rs `StoreWriter::insert`, the call to `enqueue_for_store` removed;
//! (b) in outbox_tick.rs `join_sent`, the call to `enqueue_for_store` removed.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::FakeImap;
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const APPLE: &str = "apple-mail:~/Library/Mail";
const SUBJECTS: [&str; 3] = ["Imported: quarterly report", "Imported: holiday photos", "Imported: invoice 4471"];
/// A ceiling on the indexer's settling, never a budget: the indexer runs at the OS background priority, and on a loaded Mac it gets
/// little CPU by design (engine_stale_pieces.rs SETTLE, the same reason).
const SETTLE: Duration = Duration::from_secs(120);
/// A ceiling on the import run and the account's first sync (import_listed_r2.rs waits 30 s, engine_compose_cmds.rs 25 s).
const WAIT: Duration = Duration::from_secs(30);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT { if ok() { return; } std::thread::sleep(Duration::from_millis(40)); }
    panic!("never reached within {WAIT:?}: {what}");
}

// ---------------------------------------------------------------- the fake model (engine_stale_pieces.rs)

/// A vector per text: each word (lowercased, letters and digits) adds to one slot, so a query finds the piece holding its words.
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0;
    }
    v
}

struct Hashing;
impl Embedder for Hashing {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

/// The Mac's 1-minute load average (getloadavg); 0 when the OS does not say.
fn load_1min() -> f64 {
    extern "C" { fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32; }
    let mut l = [0f64; 3];
    // SAFETY: the buffer holds the 3 values asked for and lives across the call.
    if unsafe { getloadavg(l.as_mut_ptr(), 3) } >= 1 { l[0] } else { 0.0 }
}

/// The core's model is handed over once, here, before the mail is stored; the backfill of that hand-over (an empty walk) settles.
fn give_model(core: &RustCore, test: &str) {
    core.background().set_embedder(Some(Arc::new(Hashing)));
    settled(core, test, "the indexer settled after the model was given");
}

/// The core's indexer settles within SETTLE; the time it took and the machine's load are printed (counts and times only).
fn settled(core: &RustCore, test: &str, what: &str) {
    let t0 = Instant::now();
    let ok = core.background().settle_index(SETTLE);
    eprintln!("engine_index_on_store: {test}: {what}: {ok} after {:?}, load {:.1}", t0.elapsed(), load_1min());
    assert!(ok, "{what}");
}

/// One mail's body chunks, the engine's own read: (id, text, has its vector).
fn chunks(store: &Store, id: &str) -> Vec<(i64, String, bool)> { store.background_reads().unwrap().body_chunks(id).unwrap() }

/// The ids of the mails the meaning search answers for `query`, best first.
fn found(core: &RustCore, query: &str) -> Vec<String> {
    core.background().search_meaning(query, 5).expect("the meaning search answers").into_iter().map(|h| h.chunk.message_id).collect()
}

// ---------------------------------------------------------------- (a) the import (import_listed_r2.rs)

fn fixture(home: &Path) {
    let dir = home.join("Library/Mail/V10/acct/Inbox.mbox/GUID/Data/1/Messages");
    fs::create_dir_all(&dir).unwrap();
    for (n, (subject, sender)) in SUBJECTS.iter().zip(["boss@example.test", "pat@example.test", "kim@example.test"]).enumerate() {
        let m = format!("Message-ID: <import-{}@whoosh-check.invalid>\nFrom: {sender}\nTo: stranger@whoosh-check.invalid\nSubject: {subject}\nDate: Tue, 02 Jan 2024 10:11:12 +0000\n\nBody of {subject}.\n", n + 1);
        let mut out = format!("{}\n", m.len()).into_bytes();
        out.extend_from_slice(m.as_bytes());
        out.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>date-received</key>\n\t<integer>1704190272</integer>\n\t<key>flags</key>\n\t<integer>1</integer>\n</dict>\n</plist>\n");
        fs::write(dir.join(format!("{}.emlx", n + 1)), out).unwrap();
    }
}

/// import_listed_r2.rs `import_with`, with the model given to the core before the import starts.
fn import_with_model(test: &str) -> (RustCore, tempfile::TempDir, tempfile::TempDir) {
    let db = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fixture(home.path());
    let store = Store::open(&db.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    core.set_import_home(home.path().to_path_buf());
    give_model(&core, test);
    core.run(Command::ImportScan { picked_paths: vec![] }).unwrap();
    let run = core.run(Command::ImportStart { source_id: APPLE.into(), into: ImportInto::Imported }).unwrap().created_id.unwrap();
    let result = {
        let t0 = Instant::now();
        loop {
            if let Some(r) = core.state(format!("import/{run}/result")) { break r; }
            assert!(t0.elapsed() < WAIT, "the run never ended");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let r: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(r["stored"].as_u64(), Some(3), "the import stored the three mails: {result}");
    (core, db, home)
}

fn all_view() -> InboxQuery {
    InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}

/// (a) A model is given, then a mailbox is imported: once the indexer settles, every imported mail holds body pieces, each with its vector
/// mark, and the meaning search finds a mail by the words of its text, with no second hand-over and no reopen. Broken copy: in
/// import_run.rs `StoreWriter::insert`, the call to `enqueue_for_store` removed (the mails keep no piece until the next launch's backfill).
#[test]
fn an_imported_mail_is_found_by_meaning_with_no_restart() {
    const T: &str = "import";
    let (core, _db, _home) = import_with_model(T);
    let rows: Vec<(String, String)> = core.inbox(all_view()).into_iter().map(|r| (r.message_id, r.subject)).collect();
    assert_eq!(rows.len(), 3, "the three imported mails are listed: {:?}", rows.iter().map(|r| r.1.clone()).collect::<Vec<_>>());
    settled(&core, T, "the indexer settled after the import");
    let store = core.store();
    for (id, subject) in &rows {
        let c = chunks(&store, id);
        assert!(!c.is_empty(), "the imported mail {subject:?} holds body pieces (it holds {})", c.len());
        assert!(c.iter().all(|p| p.2), "every piece of {subject:?} carries its vector mark: {:?}", c.iter().map(|p| (p.0, p.2)).collect::<Vec<_>>());
    }
    let (invoice, _) = rows.iter().find(|r| r.1 == "Imported: invoice 4471").expect("the invoice mail");
    let hits = found(&core, "Body of Imported: invoice 4471.");
    assert_eq!(hits.first(), Some(invoice), "the meaning search finds the imported invoice first: {hits:?}");
}

// ---------------------------------------------------------------- (b) the send (engine_compose_cmds.rs)

fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// An SMTP server on 127.0.0.1 that accepts mail and keeps each message (engine_compose_cmds.rs `smtp`).
struct Smtp { port: u16, got: Arc<Mutex<Vec<String>>> }
fn smtp() -> Smtp {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let got = Arc::new(Mutex::new(Vec::new()));
    let g = got.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(s) = s else { break };
            let g = g.clone();
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                let mut r = BufReader::new(s);
                let _ = w.write_all(b"220 accept.test ESMTP\r\n");
                let mut line = String::new();
                loop {
                    line.clear();
                    if r.read_line(&mut line).unwrap_or(0) == 0 { return; }
                    let up = line.trim_end().to_ascii_uppercase();
                    let reply: &[u8] = if up.starts_with("EHLO") || up.starts_with("HELO") { b"250-accept.test\r\n250-AUTH PLAIN LOGIN\r\n250 8BITMIME\r\n" }
                        else if up.starts_with("AUTH") { b"235 2.7.0 ok\r\n" }
                        else if up.starts_with("DATA") {
                            let _ = w.write_all(b"354 go\r\n");
                            let mut body = String::new();
                            loop { let mut l2 = String::new(); if r.read_line(&mut l2).unwrap_or(0) == 0 { return; } if l2 == ".\r\n" { break; } body.push_str(&l2); }
                            g.lock().unwrap().push(body);
                            b"250 2.0.0 queued\r\n"
                        }
                        else if up.starts_with("QUIT") { let _ = w.write_all(b"221 bye\r\n"); return; }
                        else { b"250 ok\r\n" };
                    if w.write_all(reply).is_err() { return; }
                }
            });
        }
    });
    Smtp { port, got }
}

/// engine_compose_cmds.rs `added()`: the IMAP fake and the account added through the core (the sync is listening when it returns).
fn added() -> (RustCore, String, FakeImap, Smtp, tokio::runtime::Runtime, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 1);
    let smtp = smtp();
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let id = out.created_id.unwrap();
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    (core, id, imap, smtp, rt, dir)
}

/// (b) A model is given, then one mail is sent: once the indexer settles, the stored copy (`sent-<outbox id>`) holds body pieces, each with
/// its vector mark, and the meaning search finds it by the words of its text, with no second hand-over and no reopen. Broken copy: in
/// outbox_tick.rs `join_sent`, the call to `enqueue_for_store` removed (the copy keeps no piece until the next launch's backfill).
#[test]
fn the_copy_of_a_sent_mail_is_found_by_meaning_with_no_restart() {
    const T: &str = "send";
    const BODY: &str = "The marmalade tasting moves to Thursday at the lighthouse.";
    let (core, a, _imap, smtp, _rt, _d) = added();
    give_model(&core, T);
    let draft = core.run(Command::SaveDraft { from_account_id: Some(a.clone()), to: vec!["ann@example.test".into()], cc: vec![], subject: "Tasting".into(), body: BODY.into(), reply_to_thread_id: None }).unwrap().created_id.unwrap();
    let entry = core.run(Command::Send { draft_id: draft }).expect("queued").created_id.unwrap();
    core.run(Command::SendNow { outbox_id: entry.clone() }).expect("sent");
    assert_eq!(smtp.got.lock().unwrap().len(), 1, "the SMTP server received exactly one message");
    let sent_id = format!("sent-{entry}");
    let store = core.store();
    assert_eq!(store.body_parts(&sent_id).unwrap().and_then(|b| b.body_text).as_deref(), Some(BODY), "the sent copy is stored with its text");
    settled(&core, T, "the indexer settled after the send");
    let c = chunks(&store, &sent_id);
    assert!(!c.is_empty(), "the sent copy holds body pieces (it holds {})", c.len());
    assert!(c.iter().all(|p| p.2), "every piece of the sent copy carries its vector mark: {:?}", c.iter().map(|p| (p.0, p.2)).collect::<Vec<_>>());
    let hits = found(&core, "marmalade tasting lighthouse");
    assert_eq!(hits.first(), Some(&sent_id), "the meaning search finds the sent copy first: {hits:?}");
}
