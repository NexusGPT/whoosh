//! The real core runs delta 18 (docs/lanes/engine-import.md): the mail import (scan, start, cancel and the three state reads), the Reminders
//! view with `ClearReplyReminder` and `thread/<id>/reminder`, and the muted-arrival rule. The stores are real (a temp database), the
//! sources are fixture trees the tests write themselves (an Apple Mail tree, an mbox); no real mail, no server, no window.
//!
//! Mutants (single edits of one production file; the runner is _captures/engine-import/run-mutants.py, verbatim results in
//! mutants-engine.out; each names the test that must go red):
//!   E1 import_run.rs `start`: the already-running check is skipped (`running` passed as None)    -> a_second_start_is_refused_and_cancel_stops_the_run_at_its_next_batch
//!   E2 import_run.rs `StoreWriter::insert`: the cancel flag is never looked at                    -> a_second_start_is_refused_and_cancel_stops_the_run_at_its_next_batch
//!   E3 import_run.rs `scan`: Apple Mail is searched under the wrong folder                        -> a_scan_on_a_fixture_tree_lists_apple_mail_first_then_the_picks
//!   E4 import crate error.rs: one word of FULL_DISK_ACCESS_WORDS changed                          -> the_full_disk_access_refusal_is_the_contracts_sentence
//!   E5 import_run.rs `execute`: the writer is handed another source id                            -> a_start_stores_the_messages_with_their_source_and_a_second_run_adds_nothing
//!   E6 reminders.rs `clear`: nothing is dismissed (the refusal check only)                        -> a_fired_reminder_is_a_row_of_the_view_and_clear_dismisses_it
//!   E7 reads.rs: the Reminders arm lists nothing                                                  -> a_fired_reminder_is_a_row_of_the_view_and_clear_dismisses_it
//!   E8 store ops.rs `apply_muted_arrival` never runs                                              -> a_muted_threads_new_mail_is_filed_under_muted_and_never_in_the_inbox
//!   E9 import_run.rs: the flagged messages are not starred                                        -> a_skipped_message_is_named_and_a_flagged_one_is_starred
//!   E10 import_run.rs: a cancelled run ends Failed                                                -> a_second_start_is_refused_and_cancel_stops_the_run_at_its_next_batch
//!   E11 import_run.rs `end`: the "one run at a time" lock is never released                       -> a_second_start_is_refused_and_cancel_stops_the_run_at_its_next_batch
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }

struct Rig {
    core: RustCore,
    home: tempfile::TempDir,
    files: tempfile::TempDir,
    _db: tempfile::TempDir,
}

fn add_account(store: &Store, id: &str, address: &str, order: i64) {
    let (id, address) = (id.to_string(), address.to_string());
    store
        .write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES (?1, ?2, 'imap', ?1, '#336699', ?3, 'manual', 'svc' || char(31) || ?1, 0)",
                rusqlite::params![id, address, order],
            )?;
            Ok(())
        })
        .unwrap();
}

fn rig() -> Rig {
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(&db.path().join("whoosh.sqlite")).unwrap();
    add_account(&store, "acc-a", "anna@example.test", 0);
    add_account(&store, "acc-b", "ben@example.test", 1);
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector(), TestTransport::PlainLoopback);
    let (home, files) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    core.set_import_home(home.path().to_path_buf());
    Rig { core, home, files, _db: db }
}

// ---------------------------------------------------------------- fixtures

fn msg(id: &str, from: &str, to: &str, subject: &str, extra_headers: &str) -> String {
    format!("Message-ID: <{id}>\nFrom: {from}\nTo: {to}\nSubject: {subject}\nDate: Tue, 02 Jan 2024 10:11:12 +0000\n{extra_headers}\nBody of {subject}\n")
}

fn emlx(message: &str) -> Vec<u8> {
    let mut out = format!("{}\n", message.len()).into_bytes();
    out.extend_from_slice(message.as_bytes());
    out.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>date-received</key>\n\t<integer>1704190272</integer>\n\t<key>flags</key>\n\t<integer>1</integer>\n</dict>\n</plist>\n");
    out
}

/// `<home>/Library/Mail/V10/<account>/<mailbox>.mbox/GUID/Data/1/Messages/<n>.emlx`, one file per message.
fn apple_mailbox(home: &Path, mailbox: &str, messages: &[String]) {
    let dir = home.join("Library/Mail/V10/acct").join(format!("{mailbox}.mbox")).join("GUID/Data/1/Messages");
    fs::create_dir_all(&dir).unwrap();
    for (i, m) in messages.iter().enumerate() {
        fs::write(dir.join(format!("{}.emlx", i + 1)), emlx(m)).unwrap();
    }
}

fn mbox(messages: &[String]) -> Vec<u8> {
    let mut out = String::new();
    for m in messages {
        out.push_str("From MAILER-DAEMON Tue Jan  2 10:11:12 2024\n");
        out.push_str(m);
        out.push('\n');
    }
    out.into_bytes()
}

fn write_mbox(dir: &Path, name: &str, messages: &[String]) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, mbox(messages)).unwrap();
    p
}

/// A value of docs/CONSTANTS.md, read the way the store's tests read it (the ledger is the only home of a number).
fn constant(name: &str) -> u64 {
    let ledger = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/CONSTANTS.md");
    let text = fs::read_to_string(&ledger).unwrap();
    let line = text.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |"))).unwrap_or_else(|| panic!("no row {name}"));
    let digits: String = line.split('|').nth(2).unwrap().trim().chars().take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
    digits.parse().unwrap()
}

// ---------------------------------------------------------------- reading state

fn state(core: &RustCore, path: &str) -> Option<String> { core.state(path.to_string()) }

/// The number after the LAST `"key":` of a state value (sorted keys: a top-level number such as `stored` comes after the lists that repeat it).
fn num(json: &str, key: &str) -> u64 {
    let at = json.rfind(&format!("\"{key}\":")).unwrap_or_else(|| panic!("no {key} in {json}")) + key.len() + 3;
    json[at..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or_else(|_| panic!("{key} is not a number in {json}"))
}

fn wait_for(core: &RustCore, run: &str, states: &[&str]) -> String {
    let t0 = Instant::now();
    loop {
        if let Some(p) = state(core, &format!("import/{run}/progress")) {
            if states.iter().any(|s| p.contains(&format!("\"state\":\"{s}\""))) { return p; }
        }
        assert!(t0.elapsed() < Duration::from_secs(30), "the run {run} never reached {states:?}: {:?}", state(core, &format!("import/{run}/progress")));
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn invalid(r: Result<CommandOutcome, CoreError>) -> (String, String) {
    match r { Err(CoreError::Invalid { field, reason_words }) => (field, reason_words), other => panic!("expected a refusal in words, got {other:?}") }
}

fn scan(core: &RustCore, picks: &[&Path]) -> Result<CommandOutcome, CoreError> {
    core.run(Command::ImportScan { picked_paths: picks.iter().map(|p| p.display().to_string()).collect() })
}

fn start(core: &RustCore, source_id: &str, into: ImportInto) -> Result<String, CoreError> {
    core.run(Command::ImportStart { source_id: source_id.to_string(), into }).map(|o| o.created_id.expect("a run id"))
}

fn rows(core: &RustCore, sql: &'static str) -> i64 { core.store().write(move |c| Ok(c.tx.query_row(sql, [], |r| r.get::<_, i64>(0))?)).unwrap() }

const APPLE: &str = "apple-mail:~/Library/Mail";

// ---------------------------------------------------------------- import: the scan

#[test]
fn a_scan_on_a_fixture_tree_lists_apple_mail_first_then_the_picks() {
    let r = rig();
    // before any scan: nothing
    assert_eq!(state(&r.core, "import/sources").as_deref(), Some("[]"));
    // Apple Mail has never run here: nothing to list for it, and the pick still is
    let one = write_mbox(r.files.path(), "Clients.mbox", &[msg("p1@x.test", "carol@far.test", "anna@example.test", "one", "")]);
    scan(&r.core, &[&one]).unwrap();
    let only_pick = state(&r.core, "import/sources").unwrap();
    assert!(only_pick.starts_with(r#"[{"access":"readable","id":"mbox:"#) && only_pick.contains(r#""name":"Clients.mbox""#), "{only_pick}");
    assert!(only_pick.contains(r#""message_count":null"#) && only_pick.contains(r#""imported_count":0"#) && only_pick.contains(r#""last_run_ms":null"#), "{only_pick}");
    // an Apple Mail tree of two mailboxes (3 + 2 messages), a missing pick
    apple_mailbox(r.home.path(), "Inbox", &(0..3).map(|i| msg(&format!("a{i}@x.test"), "carol@far.test", "anna@example.test", &format!("inbox {i}"), "")).collect::<Vec<_>>());
    apple_mailbox(r.home.path(), "Clients", &(0..2).map(|i| msg(&format!("c{i}@x.test"), "carol@far.test", "ben@example.test", &format!("clients {i}"), "")).collect::<Vec<_>>());
    let gone = r.files.path().join("Gone.mbox");
    scan(&r.core, &[&one, &gone]).unwrap();
    let all = state(&r.core, "import/sources").unwrap();
    let apple_at = all.find(APPLE).expect("Apple Mail is listed");
    let clients_at = all.find("Clients.mbox").expect("the first pick is listed");
    let gone_at = all.find("Gone.mbox").expect("the second pick is listed");
    assert!(apple_at < clients_at && clients_at < gone_at, "Apple Mail first, then the picks in the order picked: {all}");
    let apple = &all[..all.find(r#"},{"#).unwrap()];
    assert!(apple.contains(r#""message_count":5"#) && apple.contains(r#""access":"readable""#) && apple.contains(r#""kind":"apple-mail""#), "{apple}");
    assert!(apple.contains(r#""location":"~/Library/Mail""#) && apple.contains(r#""words":"""#), "{apple}");
    assert!(all.contains(&format!(r#""access":"not_found""#)) && all.contains(&import_access_words(ImportAccess::NotFound)), "a missing file keeps its row and its words: {all}");
    // a scan is safe to repeat: the same rows
    scan(&r.core, &[&one, &gone]).unwrap();
    assert_eq!(state(&r.core, "import/sources").unwrap(), all);
    // a picked file of another kind refuses the whole scan in words and leaves the last scan as it was
    let note = r.files.path().join("notes.txt");
    let (code, words) = invalid(scan(&r.core, &[&one, &note]));
    assert_eq!((code.as_str(), words.as_str()), (IMPORT_BAD_PICK_CODE, "Pick an Outlook archive (.olm) or a Thunderbird or Unix mailbox file (.mbox)."));
    assert_eq!(state(&r.core, "import/sources").unwrap(), all);
    // no mail changed by any of it
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message"), 0);
}

#[test]
fn the_full_disk_access_refusal_is_the_contracts_sentence() {
    use std::os::unix::fs::PermissionsExt;
    let r = rig();
    let mail = r.home.path().join("Library/Mail");
    fs::create_dir_all(mail.join("V10")).unwrap();
    // the import crate's constant and the contract's sentence are one sentence
    assert_eq!(::import::FULL_DISK_ACCESS_WORDS, import_access_words(ImportAccess::NeedsFullDiskAccess));
    // macOS answers EPERM to a program with no right to list the folder; a folder with no permission answers EACCES, which the crate maps the same way
    fs::set_permissions(&mail, fs::Permissions::from_mode(0o000)).unwrap();
    let scanned = scan(&r.core, &[]);
    let sources = state(&r.core, "import/sources");
    let refused = start(&r.core, APPLE, ImportInto::Imported);
    fs::set_permissions(&mail, fs::Permissions::from_mode(0o755)).unwrap();
    scanned.unwrap();
    let sources = sources.unwrap();
    let words = import_access_words(ImportAccess::NeedsFullDiskAccess);
    assert!(sources.contains(r#""access":"needs_full_disk_access""#) && sources.contains(&format!(r#""words":{}"#, json(&words))), "the row says it BEFORE a start: {sources}");
    let (code, said) = invalid(refused.map(|id| CommandOutcome { undo_entry_id: None, created_id: Some(id) }));
    assert_eq!((code.as_str(), said.as_str()), (IMPORT_FULL_DISK_ACCESS_CODE, words.as_str()), "a start throws the same code and words");
    assert!(said.contains("Full Disk Access") && said.contains("skip this step"), "it says how to grant it AND that skipping is fine: {said}");
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message"), 0, "a refused start creates no run and stores nothing");
}

fn json(s: &str) -> String { format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")) }

// ---------------------------------------------------------------- import: a run

#[test]
fn a_start_stores_the_messages_with_their_source_and_a_second_run_adds_nothing() {
    let r = rig();
    let mut inbox: Vec<String> = (0..3).map(|i| msg(&format!("a{i}@x.test"), "carol@far.test", "anna@example.test", &format!("for Anna {i}"), "")).collect();
    inbox.push(msg("b0@x.test", "carol@far.test", "ben@example.test", "for Ben", ""));
    inbox.push(msg("n0@x.test", "carol@far.test", "stranger@far.test", "for nobody here", ""));
    apple_mailbox(r.home.path(), "Inbox", &inbox);
    scan(&r.core, &[]).unwrap();
    let before = now();
    let run = start(&r.core, APPLE, ImportInto::Imported).unwrap();
    let progress = wait_for(&r.core, &run, &["finished"]);
    assert_eq!((num(&progress, "done"), num(&progress, "stored"), num(&progress, "skipped"), num(&progress, "total")), (5, 5, 0, 5), "{progress}");
    assert!(progress.contains(r#""fraction":1"#) && progress.contains(r#""refusal":null"#) && progress.contains(&format!(r#""run_id":"{run}""#)), "{progress}");
    let result = state(&r.core, &format!("import/{run}/result")).expect("a finished run has its result");
    assert_eq!((num(&result, "seen"), num(&result, "stored"), num(&result, "already_there")), (5, 5, 0), "{result}");
    assert!(result.contains(r#""skipped":[]"#) && result.contains(r#""state":"finished""#) && result.contains(r#""refusal":null"#), "{result}");
    // by receiving address, else the Imported account; the sum is what was stored
    let imported_id = r.core.accounts().into_iter().find(|a| a.address == "imported@whoosh.local").expect("the Imported account exists").id;
    for want in [format!(r#"{{"account_id":"acc-a","stored":3}}"#), format!(r#"{{"account_id":"acc-b","stored":1}}"#), format!(r#"{{"account_id":"{imported_id}","stored":1}}"#)] {
        assert!(result.contains(&want), "{want} in {result}");
    }
    // every row names its source and the time of the run
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message WHERE source = 'imported' AND import_source_id = 'apple-mail:~/Library/Mail' AND imported_at_ms IS NOT NULL"), 5);
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message WHERE source <> 'imported' OR import_source_id IS NULL"), 0);
    assert!(rows(&r.core, "SELECT min(imported_at_ms) FROM message") >= before, "the import time is the run's, not the message's");
    let anna = r.core.store().message("imp:acc-a:a0@x.test").unwrap().expect("a message of the run, under the account its address names");
    assert_eq!((anna.account_id.as_str(), anna.folder.as_str()), ("acc-a", "INBOX"));
    // the source's history is derived: a new scan reads it
    scan(&r.core, &[]).unwrap();
    let again = state(&r.core, "import/sources").unwrap();
    assert!(again.contains(r#""imported_count":5"#) && !again.contains(r#""last_run_ms":null"#), "{again}");
    // a second run of the same source: every message is already there, nothing new, no per-account figure
    let second = start(&r.core, APPLE, ImportInto::Imported).unwrap();
    wait_for(&r.core, &second, &["finished"]);
    let second_result = state(&r.core, &format!("import/{second}/result")).unwrap();
    assert_eq!((num(&second_result, "seen"), num(&second_result, "stored"), num(&second_result, "already_there")), (5, 0, 5), "{second_result}");
    assert!(second_result.contains(r#""per_account":[]"#), "{second_result}");
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message"), 5);
    // one account for the whole run: every message of a source goes to it
    let into_one = r.files.path().join("All.mbox");
    fs::write(&into_one, mbox(&[msg("z0@x.test", "carol@far.test", "anna@example.test", "to the chosen one", "")])).unwrap();
    scan(&r.core, &[&into_one]).unwrap();
    let id = import_source_id(ImportSourceKind::Mbox, into_one.display().to_string());
    let third = start(&r.core, &id, ImportInto::Account { account_id: "acc-b".into() }).unwrap();
    wait_for(&r.core, &third, &["finished"]);
    assert_eq!(r.core.store().message("imp:acc-b:z0@x.test").unwrap().expect("under the chosen account").account_id, "acc-b");
}

fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }

#[test]
fn a_skipped_message_is_named_and_a_flagged_one_is_starred() {
    let r = rig();
    let flagged = msg("f0@x.test", "carol@far.test", "anna@example.test", "flagged", "X-Status: F\n");
    let dead = msg("d0@x.test", "carol@far.test", "anna@example.test", "deleted in its program", "X-Mozilla-Status: 0009\n");
    let plain = msg("p0@x.test", "carol@far.test", "anna@example.test", "plain", "");
    let file = write_mbox(r.files.path(), "Mixed.mbox", &[flagged, dead, plain]);
    scan(&r.core, &[&file]).unwrap();
    let id = import_source_id(ImportSourceKind::Mbox, file.display().to_string());
    let run = start(&r.core, &id, ImportInto::Imported).unwrap();
    wait_for(&r.core, &run, &["finished"]);
    let result = state(&r.core, &format!("import/{run}/result")).unwrap();
    assert_eq!((num(&result, "seen"), num(&result, "stored"), num(&result, "already_there")), (3, 2, 0), "{result}");
    assert!(result.contains("is marked deleted in the mail program it came from") && result.contains("Mixed.mbox message 2"), "the skipped message is named with its reason: {result}");
    let starred: Vec<String> = r.core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: Some(Mailbox::Starred), account_ids: vec![], filters: vec![], smart_folder_id: None }).into_iter().map(|x| x.message_id).collect();
    assert_eq!(starred, vec!["imp:acc-a:f0@x.test".to_string()]);
}

#[test]
fn a_second_start_is_refused_and_cancel_stops_the_run_at_its_next_batch() {
    let r = rig();
    let batch = constant("IMPORT_BATCH_MESSAGES");
    let total = 2 * batch + 50;
    let messages: Vec<String> = (0..total).map(|i| msg(&format!("m{i}@x.test"), "carol@far.test", "stranger@far.test", &format!("message {i}"), "")).collect();
    let file = write_mbox(r.files.path(), "Big.mbox", &messages);
    scan(&r.core, &[&file]).unwrap();
    let id = import_source_id(ImportSourceKind::Mbox, file.display().to_string());
    // the run is held after its first batch is written, so the test cancels at a known point
    let (reached_tx, reached_rx) = channel::<()>();
    let (release_tx, release_rx) = channel::<()>();
    let (reached_tx, release_rx) = (Mutex::new(reached_tx), Mutex::new(release_rx));
    let first = AtomicBool::new(true);
    r.core.set_import_pace(Some(Arc::new(move |_p: &ImportRunProgress| {
        if first.swap(false, Ordering::SeqCst) {
            reached_tx.lock().unwrap().send(()).ok();
            let _ = release_rx.lock().unwrap().recv_timeout(Duration::from_secs(30));
        }
    })));
    let run = start(&r.core, &id, ImportInto::Imported).unwrap();
    reached_rx.recv_timeout(Duration::from_secs(30)).expect("the first batch was written");
    let held = state(&r.core, &format!("import/{run}/progress")).unwrap();
    assert!(held.contains(r#""state":"running""#) && num(&held, "stored") == batch, "{held}");
    assert!(state(&r.core, &format!("import/{run}/result")).is_none(), "no result while the run runs");
    // one run at a time, in words, and no second run exists
    let (code, words) = invalid(start(&r.core, &id, ImportInto::Imported).map(|i| CommandOutcome { undo_entry_id: None, created_id: Some(i) }));
    assert_eq!((code.as_str(), words.as_str()), (IMPORT_ALREADY_RUNNING_CODE, "An import is already running. Wait for it to end or cancel it."));
    // cancel: accepted now, the run ends at its next batch
    r.core.run(Command::ImportCancel { run_id: run.clone() }).unwrap();
    release_tx.send(()).unwrap();
    let ended = wait_for(&r.core, &run, &["cancelled", "finished", "failed", "refused"]);
    assert!(ended.contains(r#""state":"cancelled""#), "{ended}");
    let result = state(&r.core, &format!("import/{run}/result")).unwrap();
    assert!(result.contains(r#""state":"cancelled""#) && result.contains(r#""refusal":null"#), "{result}");
    assert_eq!((num(&result, "stored"), num(&result, "seen"), num(&result, "already_there")), (batch, batch, 0), "what was stored before the cancel stays: {result}");
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message") as u64, batch, "the database holds exactly the first batch");
    assert!(result.contains(&format!(r#""stored":{batch}}}]"#)), "per account adds up to what was stored: {result}");
    // the run is over: cancelling it again, or an unknown one, is refused in words
    for run_id in [run.clone(), "no-such-run".to_string()] {
        let (code, words) = invalid(r.core.run(Command::ImportCancel { run_id }));
        assert_eq!((code.as_str(), words.as_str()), (IMPORT_NOT_RUNNING_CODE, "That import is not running."));
    }
    // a re-run skips what is stored (one row per message) and stores the rest
    r.core.set_import_pace(None);
    let again = start(&r.core, &id, ImportInto::Imported).unwrap();
    wait_for(&r.core, &again, &["finished"]);
    let rest = state(&r.core, &format!("import/{again}/result")).unwrap();
    assert_eq!((num(&rest, "stored"), num(&rest, "already_there"), num(&rest, "seen")), (total - batch, batch, total), "{rest}");
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message") as u64, total);
}

#[test]
fn a_start_for_a_source_or_account_that_does_not_exist_is_refused_in_words() {
    let r = rig();
    let gone = r.files.path().join("Gone.mbox");
    scan(&r.core, &[&gone]).unwrap();
    let gone_id = import_source_id(ImportSourceKind::Mbox, gone.display().to_string());
    let (code, words) = invalid(start(&r.core, "mbox:never-scanned", ImportInto::Imported).map(|i| CommandOutcome { undo_entry_id: None, created_id: Some(i) }));
    assert_eq!((code.as_str(), words.as_str()), (IMPORT_UNKNOWN_SOURCE_CODE, "That source is not in the list. Scan again."));
    let (code, _) = invalid(start(&r.core, &gone_id, ImportInto::Imported).map(|i| CommandOutcome { undo_entry_id: None, created_id: Some(i) }));
    assert_eq!(code, IMPORT_NOT_FOUND_CODE);
    let there = write_mbox(r.files.path(), "There.mbox", &[msg("t0@x.test", "carol@far.test", "anna@example.test", "t", "")]);
    scan(&r.core, &[&there]).unwrap();
    let there_id = import_source_id(ImportSourceKind::Mbox, there.display().to_string());
    let (code, words) = invalid(start(&r.core, &there_id, ImportInto::Account { account_id: "acc-nope".into() }).map(|i| CommandOutcome { undo_entry_id: None, created_id: Some(i) }));
    assert_eq!((code.as_str(), words.as_str()), (IMPORT_BAD_TARGET_CODE, "That account is not on this Mac. Pick an account from the list, or Imported."));
    assert_eq!(rows(&r.core, "SELECT count(*) FROM message"), 0);
}

// ---------------------------------------------------------------- the Reminders view and the muted arrival

const OWNER: &str = "anna@example.test";

fn stored(core: &RustCore, thread: &str, id: &str, from: &str, at: i64, folder: &str) {
    let outgoing = folder == "Sent";
    let m = NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: "acc-a".into(), sender_name: from.into(), from_address: from.into(), to: vec!["x@ext.test".into()],
            subject: format!("Subject {thread}"), preview: format!("Preview {id}"), received_at_ms: at, read: outgoing, archived: false, folder: folder.into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: outgoing,
        },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    };
    core.store().insert_messages(vec![m]).unwrap();
}

fn remind(core: &RustCore, thread: &str, sent: &str, sent_at: i64, remind_at: i64) {
    stored(core, thread, sent, OWNER, sent_at, "Sent");
    let (rid, sid) = (format!("rr-{sent}"), sent.to_string());
    core.store().write(move |c| { c.tx.execute("INSERT INTO reminder (id, message_id, when_ms, note, updated_at_ms, if_no_reply) VALUES (?1, ?2, ?3, '', 1, 1)", rusqlite::params![rid, sid, remind_at])?; Ok(()) }).unwrap();
}

fn reminders_view(core: &RustCore) -> Vec<String> {
    core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: Some(Mailbox::Reminders), account_ids: vec![], filters: vec![], smart_folder_id: None }).into_iter().map(|r| r.message_id).collect()
}

#[test]
fn a_fired_reminder_is_a_row_of_the_view_and_clear_dismisses_it() {
    let r = rig();
    let long_ago = 5_000;
    let far = now() + 86_400_000;
    remind(&r.core, "t-fired", "s-fired", 1_000, long_ago); // time came, nobody replied
    remind(&r.core, "t-wait", "s-wait", 1_000, far); // time not yet
    remind(&r.core, "t-answered", "s-answered", 1_000, long_ago);
    stored(&r.core, "t-answered", "r-answered", "them@ext.test", 3_000, "INBOX"); // a reply in time
    assert_eq!(reminders_view(&r.core), vec!["s-fired".to_string()], "only the fired one is a row");
    assert_eq!(state(&r.core, "thread/t-fired/reminder").unwrap(), r#"{"at_ms":5000,"draft_id":"s-fired","state":"fired"}"#);
    assert_eq!(state(&r.core, "thread/s-fired/reminder").unwrap(), r#"{"at_ms":5000,"draft_id":"s-fired","state":"fired"}"#, "a message of the thread reads the same");
    assert!(state(&r.core, "thread/t-wait/reminder").unwrap().ends_with(r#""state":"waiting"}"#));
    assert_eq!(state(&r.core, "thread/t-answered/reminder").unwrap(), r#"{"at_ms":5000,"draft_id":"s-answered","state":"answered"}"#);
    assert_eq!(state(&r.core, "thread/t-none/reminder").unwrap(), r#"{"at_ms":null,"draft_id":null,"state":"none"}"#);
    // the firing is written once, for the fired one only
    assert_eq!(r.core.fire_due_reminders(), vec!["s-fired".to_string()]);
    assert!(r.core.fire_due_reminders().is_empty());
    // dismissing a row: the row goes, the state reads none, a second dismissal is refused in words
    r.core.run(Command::ClearReplyReminder { draft_id: "s-fired".into() }).unwrap();
    assert!(reminders_view(&r.core).is_empty());
    assert_eq!(state(&r.core, "thread/t-fired/reminder").unwrap(), r#"{"at_ms":null,"draft_id":null,"state":"none"}"#);
    let (code, words) = invalid(r.core.run(Command::ClearReplyReminder { draft_id: "s-fired".into() }));
    assert_eq!((code.as_str(), words.as_str()), (REMINDER_NOTHING_TO_CLEAR_CODE, "There is no reminder to clear."));
    // a waiting and an answered reminder are cleared too
    r.core.run(Command::ClearReplyReminder { draft_id: "t-wait".into() }).unwrap();
    r.core.run(Command::ClearReplyReminder { draft_id: "s-answered".into() }).unwrap();
}

#[test]
fn a_muted_threads_new_mail_is_filed_under_muted_and_never_in_the_inbox() {
    let r = rig();
    stored(&r.core, "t-muted", "m1", "x@ext.test", 1_000, "INBOX");
    stored(&r.core, "t-calm", "c1", "y@ext.test", 1_100, "INBOX");
    r.core.store().set_muted(vec!["m1".into()], true).unwrap();
    stored(&r.core, "t-muted", "m2", "x@ext.test", 2_000, "INBOX");
    stored(&r.core, "t-calm", "c2", "y@ext.test", 2_100, "INBOX");
    let list = |mailbox: Option<Mailbox>, tab: &str| -> Vec<String> {
        r.core.inbox(InboxQuery { tab_id: tab.into(), account_id: None, label_id: None, mailbox, account_ids: vec![], filters: vec![], smart_folder_id: None }).into_iter().map(|x| x.message_id).collect()
    };
    assert_eq!(list(Some(Mailbox::Muted), "primary"), vec!["m2".to_string(), "m1".to_string()], "the new mail is filed under Muted");
    let inbox = list(None, "primary");
    assert!(!inbox.contains(&"m2".to_string()) && !inbox.contains(&"m1".to_string()) && inbox.contains(&"c2".to_string()), "{inbox:?}");
    assert!(r.core.message("m2".into()).unwrap().read, "it arrives read");
    assert!(!r.core.message("c2".into()).unwrap().read, "another thread's mail arrives unread");
    let unread = r.core.counts().tabs.iter().find(|t| t.tab_id == "primary").map(|t| t.unread).unwrap();
    assert_eq!(unread, 2, "the muted thread adds nothing to the counts");
}
