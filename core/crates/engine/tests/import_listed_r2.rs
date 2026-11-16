//! Fix round r2 (docs/lanes/import-r2.md): what an import stores must be what the inbox list shows. The check's own fixture
//! (three mails to an address no account has, so all land under Imported) is imported into a core, and the list a screen asks for is read.
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::Store;

const APPLE: &str = "apple-mail:~/Library/Mail";
const SUBJECTS: [&str; 3] = ["Imported: quarterly report", "Imported: holiday photos", "Imported: invoice 4471"];

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

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

/// Every event the core hands a screen's sink, kept in order.
#[derive(Default)]
struct Tape(Mutex<Vec<CoreEvent>>);
impl EventSink for Tape {
    fn on_event(&self, event: CoreEvent) { self.0.lock().unwrap().push(event); }
}

fn import_into_empty_core() -> (RustCore, tempfile::TempDir, tempfile::TempDir) { import_with(None) }

fn import_with(tape: Option<Arc<Tape>>) -> (RustCore, tempfile::TempDir, tempfile::TempDir) {
    let db = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fixture(home.path());
    let store = Store::open(&db.path().join("whoosh.sqlite")).unwrap();
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    core.set_import_home(home.path().to_path_buf());
    if let Some(t) = tape {
        core.subscribe(StreamKind::Inbox, t.clone());
        core.subscribe(StreamKind::Counts, t);
    }
    core.run(Command::ImportScan { picked_paths: vec![] }).unwrap();
    let run = core.run(Command::ImportStart { source_id: APPLE.into(), into: ImportInto::Imported }).unwrap().created_id.unwrap();
    let t0 = Instant::now();
    while core.state(format!("import/{run}/result")).is_none() {
        assert!(t0.elapsed() < Duration::from_secs(30), "the run never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
    (core, db, home)
}

fn subjects(core: &RustCore, q: InboxQuery) -> Vec<String> {
    let mut s: Vec<String> = core.inbox(q).into_iter().map(|r| r.subject).collect();
    s.sort();
    s
}

fn want() -> Vec<String> { let mut w: Vec<String> = SUBJECTS.iter().map(|s| s.to_string()).collect(); w.sort(); w }

fn primary(account_id: Option<String>) -> InboxQuery {
    InboxQuery { tab_id: "primary".into(), account_id, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}

#[test]
fn the_imported_messages_are_in_the_primary_list_of_all_accounts() {
    let (core, _db, _home) = import_into_empty_core();
    assert_eq!(subjects(&core, primary(None)), want());
}

#[test]
fn the_imported_messages_are_in_the_list_of_the_imported_account() {
    let (core, _db, _home) = import_into_empty_core();
    let imported = core.accounts().into_iter().find(|a| a.address == "imported@whoosh.local").expect("the Imported account").id;
    assert_eq!(subjects(&core, primary(Some(imported))), want());
}

#[test]
fn the_imported_messages_are_in_the_all_view() {
    let (core, _db, _home) = import_into_empty_core();
    let mut all = primary(None);
    all.tab_id = "all".into();
    assert_eq!(subjects(&core, all), want());
}

/// A screen that lists the inbox is told when rows arrive on the Inbox and Counts streams (it cannot poll the core): an import run that
/// stores three messages says so on both, with the rows themselves on the Inbox stream.
#[test]
fn an_import_run_announces_its_rows_on_the_inbox_and_counts_streams() {
    let tape = Arc::new(Tape::default());
    let (_core, _db, _home) = import_with(Some(tape.clone()));
    let events = tape.0.lock().unwrap();
    let mut announced: Vec<String> = events.iter().flat_map(|e| match e { CoreEvent::InboxChanged { rows, .. } => rows.iter().map(|r| r.subject.clone()).collect(), _ => vec![] }).collect();
    announced.sort();
    announced.dedup();
    assert_eq!(announced, want(), "the Inbox stream's rows");
    assert!(events.iter().any(|e| matches!(e, CoreEvent::CountsChanged { .. })), "the Counts stream said nothing");
}
