//! body-r2: two faults in message text seen on the owner's installed app, proven over the production core of the hidden test copy.
//! (1) Stored rows hold HTML character references ("Bonjour&nbsp;Assem"): they are repaired at core start, once, with no download.
//! (2) One large newsletter's body was reported as "the server sent no body for UID 143411": a 600 KB body is fetched and stored over
//! the real adapter (a scripted server, in every legal answer shape), and a body the server never sends is logged with its case.
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/body_r2_server/mod.rs"]
mod server;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use server::{raw, Server, Shape};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(20);

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

fn row(account: &str, uid: u32, preview: &str, body_text: Option<&str>) -> NewMessage {
    let id = format!("{account}:INBOX:1:{uid}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{account}:<{uid}@thread>"), account_id: account.into(), sender_name: "Wellness".into(), from_address: "noreply@mywellness.example".into(),
            to: vec![USER.into()], subject: format!("Rappel {uid}"), preview: preview.into(), received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: body_text.map(str::to_string), body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

fn open_core(dir: &std::path::Path) -> RustCore { RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens") }

fn stored(dir: &std::path::Path, account: &str, uid: u32) -> (Option<String>, String) {
    let store = Store::open(&dir.join("whoosh.sqlite")).unwrap();
    let p = store.body_parts(&format!("{account}:INBOX:1:{uid}")).unwrap().unwrap();
    (p.body_text, p.preview)
}

/// (1) The owner's database before this lane: 9 of 58 rows held `&nbsp;`. The rows are seeded before the core starts; the core's
/// start repairs them in place with no server, once: a row written after that (a text that really says `&nbsp;`) is never decoded.
/// Mutants: the repair never runs; the marker is not honoured (the second start decodes again).
#[test]
fn a_stored_row_with_references_is_repaired_at_start_and_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let account = {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![
            row(&account, 1, "Bonjour&nbsp;Assem, Ceci est un rappel&nbsp;&nbsp;:", Some("Bonjour&nbsp;Assem,\nCeci est un rappel&nbsp;: votre s&eacute;ance")),
            row(&account, 2, "Tom &amp; Jerry l&#39;&eacute;t&eacute;", Some("Tom &amp; Jerry &#x2014; l&#39;&eacute;t&eacute;")),
            row(&account, 3, "Write &amp;nbsp; for a space", Some("Write &amp;nbsp; for a space")),
            row(&account, 4, "AT&T and 5 < 6", Some("AT&T and 5 < 6; ?a=1&copy=2")),
            row(&account, 5, "Rappel&nbsp;: votre s&eacute;ance", None),
        ]).unwrap();
        account
    };
    // the core starts: the repair runs off the caller's thread
    let core = open_core(dir.path());
    let read_log = || std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    // the log line is written after the repair's commit
    wait_until("the repair ran", || read_log().contains("text repair:"));
    assert!(dir.path().join("body-text-repaired").exists());
    let (t1, p1) = stored(dir.path(), &account, 1);
    assert_eq!(t1.as_deref(), Some("Bonjour Assem,\nCeci est un rappel : votre s\u{e9}ance"));
    assert_eq!(p1, "Bonjour Assem, Ceci est un rappel :", "a preview is decoded and its spaces collapsed");
    let (t2, p2) = stored(dir.path(), &account, 2);
    assert_eq!((t2.as_deref(), p2.as_str()), (Some("Tom & Jerry \u{2014} l'\u{e9}t\u{e9}"), "Tom & Jerry l'\u{e9}t\u{e9}"));
    let (t3, p3) = stored(dir.path(), &account, 3);
    assert_eq!((t3.as_deref(), p3.as_str()), (Some("Write &nbsp; for a space"), "Write &nbsp; for a space"), "decoded once: the text of a mail about HTML");
    let (t4, p4) = stored(dir.path(), &account, 4);
    assert_eq!((t4.as_deref(), p4.as_str()), (Some("AT&T and 5 < 6; ?a=1&copy=2"), "AT&T and 5 < 6"), "a row with no reference is left as it is");
    let (t5, p5) = stored(dir.path(), &account, 5);
    assert_eq!((t5, p5.as_str()), (None, "Rappel : votre s\u{e9}ance"), "a row with no body: its preview is repaired");
    // the reader shows the repaired preview of row 5 (no body stored, no server)
    assert_eq!(core.body(format!("{account}:INBOX:1:5")).unwrap().plain_text, "Rappel : votre s\u{e9}ance");
    let log = read_log();
    assert!(log.contains("text repair: 4 stored rows had HTML character references decoded"), "{log:?}");
    drop(core);
    // a body stored later whose text really says `&nbsp;`, then a second start: the marker keeps the repair from running again
    {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        store.set_body(&format!("{account}:INBOX:1:4"), "Write &nbsp; for a space &amp; more".into(), None, 0, "Write &nbsp; for a space".into()).unwrap();
    }
    let core = open_core(dir.path());
    std::thread::sleep(Duration::from_millis(500));
    let (t4, p4) = stored(dir.path(), &account, 4);
    assert_eq!((t4.as_deref(), p4.as_str()), (Some("Write &nbsp; for a space &amp; more"), "Write &nbsp; for a space"), "the second start repaired nothing");
    drop(core);
}

/// (2a) A 600 KB newsletter is fetched over the real adapter and stored whole, whatever legal shape the server answers in: the reader
/// shows its text and the row's preview is its first words.
#[test]
fn a_600_kb_body_is_fetched_and_stored_in_every_legal_answer_shape() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    for shape in [Shape::Whole, Shape::Chunked, Shape::Origin, Shape::Rfc822, Shape::UidLast, Shape::Split, Shape::SectionsOnly] {
        let srv = rt.block_on(Server::start(&[(143411, shape)], 600));
        let dir = tempfile::tempdir().unwrap();
        let core = open_core(dir.path());
        let store = core.store();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![row(&account, 143411, "Omega Glow Bowl", None)]).unwrap();
        core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
        let id = format!("{account}:INBOX:1:143411");
        let _ = core.body(id.clone());
        wait_until(&format!("{shape:?}: the 600 KB body is stored"), || store.body_parts(&id).unwrap().unwrap().body_text.is_some());
        let parts = store.body_parts(&id).unwrap().unwrap();
        let text = parts.body_text.unwrap();
        assert!(text.len() > 590_000, "{shape:?}: the whole body is stored, {} bytes", text.len());
        assert!(parts.preview.starts_with("The Omega Glow Bowl: miso glazed salmon"), "{shape:?}: {:?}", parts.preview);
        assert!(core.settle_bodies(WAIT));
        let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
        assert!(!log.contains("the server sent no body"), "{shape:?}: nothing logged: {log:?}");
        assert_eq!(core.body(id).unwrap().plain_text.len(), text.len(), "{shape:?}: the reader reads the stored body");
    }
}

/// (2b) A body the server never sends: the failure in core.log says which case it was: what came back (its keys) and the size the
/// server named, as the owner's Mac will print them for UID 143411. Mutant: the engine's log line drops the case.
#[test]
fn a_body_the_server_never_sends_is_logged_with_its_case() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    for (shape, case) in [(Shape::Nil, "BODY[] came back NIL"), (Shape::Empty, "BODY[] came back as an empty literal, 0 bytes"), (Shape::NoBodyKey, "no BODY[] in the answer"), (Shape::NoAnswer, "no FETCH for this UID")] {
        let srv = rt.block_on(Server::start(&[(143411, shape)], 600));
        let dir = tempfile::tempdir().unwrap();
        let core = open_core(dir.path());
        let store = core.store();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![row(&account, 143411, "Omega Glow Bowl", None)]).unwrap();
        core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
        let id = format!("{account}:INBOX:1:143411");
        let _ = core.body(id.clone());
        wait_until(&format!("{shape:?}: the failure is logged"), || std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default().contains("the server sent no body"));
        assert!(core.settle_bodies(WAIT));
        let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap();
        let line = log.lines().find(|l| l.contains("the server sent no body")).unwrap();
        assert!(line.contains(&format!("body {id}: the server sent no body for UID 143411: {case}")), "{shape:?}: {line:?}");
        assert!(line.contains("asked alone; tried by sections"), "{shape:?}: {line:?}");
        if !matches!(shape, Shape::NoAnswer) { assert!(line.contains(&format!("the server named size {}", raw(143411, 600).len())), "{shape:?}: {line:?}"); }
        assert!(store.body_parts(&id).unwrap().unwrap().body_text.is_none(), "{shape:?}: nothing is made up");
    }
}

/// Owner's store, 14 Nov 2026 (picture 216): 14 stored previews held source (a doctype, a part boundary, page code). At core start, once,
/// each such row gets the preview a download makes today from its stored text; with no stored text it is emptied until the body comes.
#[test]
fn a_stored_preview_that_holds_source_is_repaired_at_start_and_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let code = "<div style=background-color:#fff><table border=0><tr><td>Your request is with our team.<br>Thank you</td></tr></table></div>";
    let account = {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![
            row(&account, 1, "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org", Some("Dear Maria,\n\nYour parcel is on its way.\n")),
            row(&account, 2, "--0000000000001a2b3c Content-Type: text/plain; charset=\"UTF-8\" Hi Maria", Some("Hi Maria,\n\nLunch on Friday?\n")),
            row(&account, 3, "<div style=background-color:#fff><table border=0><tr><td>Your request", Some(code)),
            row(&account, 4, "<!DOCTYPE html><html lang=\"en\"><head>", None),
            row(&account, 5, "<maria@example.com> wrote to the list", None),
            row(&account, 6, "<3 thank you all", Some("<3 thank you all")),
        ]).unwrap();
        account
    };
    let core = open_core(dir.path());
    let read_log = || std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    wait_until("the source preview repair ran", || read_log().contains("stored rows held source in their preview"));
    assert!(dir.path().join("preview-source-dropped").exists());
    assert_eq!(stored(dir.path(), &account, 1).1, "Dear Maria, Your parcel is on its way.");
    assert_eq!(stored(dir.path(), &account, 2).1, "Hi Maria, Lunch on Friday?");
    let p3 = stored(dir.path(), &account, 3).1;
    assert!(p3.starts_with("Your request is with our team.") && !p3.contains('<'), "the words of the page code: {p3}");
    assert_eq!(stored(dir.path(), &account, 4).1, "", "no stored text: emptied until the body comes");
    assert_eq!(stored(dir.path(), &account, 5).1, "<maria@example.com> wrote to the list", "an address in angle brackets is words");
    assert_eq!(stored(dir.path(), &account, 6).1, "<3 thank you all");
    assert!(read_log().contains("preview repair: 4 stored rows held source in their preview; 1 mails with page code as text are indexed again"), "{}", read_log());
    drop(core);
    {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        store.insert_messages(vec![row(&account, 7, "<!DOCTYPE html><html>", None)]).unwrap();
    }
    let _core = open_core(dir.path());
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(stored(dir.path(), &account, 7).1, "<!DOCTYPE html><html>", "once: the marker is honoured");
}

/// Owner's store, 13 Nov 2026: three stored previews held drawn lines (one began with 81 characters of dashes). At core start, once, each
/// such row gets the preview a download makes today: from its stored text when it has one, else its old preview with the lines left out.
/// Mutants: the repair never runs (`repair_stored_previews` not called at start); the marker is not honoured (the second start changes row 5).
#[test]
fn a_stored_preview_with_a_drawn_line_is_repaired_at_start_and_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let forward = "---\n\n---------- Forwarded message ---------\nFrom: Facilities <facilities@example.net>\nSubject: Room booking\n\nRoom 4 is booked for Thursday at ten.\n";
    let account = {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![
            row(&account, 1, "--- ---------- Forwarded message --------- From: Facilities", Some(forward)),
            row(&account, 2, "Hello ________________________________ Team", None),
            row(&account, 3, "Budget 2026 - draft -- v2", Some("Budget 2026 - draft -- v2")),
            row(&account, 4, "???", Some("???")),
        ]).unwrap();
        account
    };
    let core = open_core(dir.path());
    let read_log = || std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    wait_until("the preview repair ran", || read_log().contains("preview repair:"));
    assert!(dir.path().join("preview-lines-dropped").exists());
    let p1 = stored(dir.path(), &account, 1).1;
    assert!(p1.starts_with("Forwarded message From: Facilities") && !p1.contains("---") && p1.contains("Room 4 is booked"), "made again from the stored text: {p1}");
    assert_eq!(stored(dir.path(), &account, 2).1, "Hello Team", "no stored text: the old preview with the line left out");
    assert_eq!(stored(dir.path(), &account, 3).1, "Budget 2026 - draft -- v2", "two dashes between words are not a line");
    assert_eq!(stored(dir.path(), &account, 4).1, "???");
    assert!(read_log().contains("preview repair: 2 stored rows"), "{}", read_log());
    drop(core);
    // once: a row written after the repair is not touched at the next start
    {
        let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
        store.insert_messages(vec![row(&account, 5, "Later _____ row", None)]).unwrap();
    }
    let core = open_core(dir.path());
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(stored(dir.path(), &account, 5).1, "Later _____ row", "the marker stops a second repair");
    drop(core);
}
