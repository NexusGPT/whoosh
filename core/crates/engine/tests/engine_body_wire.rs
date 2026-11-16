//! body-wire (contracts delta 26): the mail content engine (`parse::prepare_in_background`) runs ONCE when a body is fetched, in the
//! background; its result is kept in the store with the body (table `message_prepared`, migration 0019) and read back when the mail opens.
//! Over the production core and the real IMAP adapter against the scripted server (127.0.0.1 only; nothing reaches a real host).
//! The tests of this file share one process-wide counter (`engine::bodies::prepare_calls`): each holds `SERIAL` so no other test of
//! this file runs the engine while it counts.
//!
//! MUTANTS (each turns the test named red; logs in the lane's output folder):
//!   W1 `wire-prepared-not-stored`   bodies.rs `kept`: `prepared: None`                                -> three_fetched_bodies_carry_the_engines_result_after_a_restart
//!   W2 `wire-read-ignores-prepared` rust_core.rs `stored_body`: the kept JSON is not passed (None)     -> three_fetched_bodies_carry_the_engines_result_after_a_restart
//!   W3 `wire-prepare-on-read`       reads.rs `body_from_stored`: the engine runs again on the stored text -> a_stored_mail_is_read_without_running_the_engine
//!   W4 `wire-no-migration`          migrations: 0019 left out of MIGRATIONS                              -> a_store_of_the_previous_version_opens_and_old_bodies_read_with_empty_new_fields
//!   W5 `wire-earlier-empty`         bodies.rs `kept` is given no earlier text                          -> the_threads_earlier_text_reaches_the_engine
//!   W7 `wire-counts-dropped`       reads.rs: the two picture counts are not read back                  -> three_fetched_bodies_carry_the_engines_result_after_a_restart
//!   W8 `wire-large-analyse`        bodies.rs `kept`: the guard for a body over PARSE_BODY_BYTES_MAX removed -> a_body_over_the_size_cap_is_kept_as_text_with_its_warning
//!   (W2 is also the red of a_stored_prepared_form_reads_back_through_rust_core_body)
//!   W9 `wire-no-remake`         bodies.rs `prefetch_run`: the line `self.remake(&account, &folder).await;` dropped        -> a_stored_body_without_a_whole_record_gets_one_at_the_next_pass
//!   W6 `wire-deep-analyse`         bodies.rs `kept`: the TooDeep guard removed (the old `parse::analyse` runs on it) -> a_body_nested_past_the_cap_is_kept_quickly_as_text
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/body_r2_server/mod.rs"]
mod server;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use server::{Fixture, Server};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(30);
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> { SERIAL.lock().unwrap_or_else(|e| e.into_inner()) }

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

fn row(account: &str, uid: u32, thread: &str, subject: &str, at_s: i64, body_text: Option<&str>) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:INBOX:1:{uid}"), thread_id: format!("{account}:<{thread}>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "sender@example.test".into(), to: vec![USER.into()], subject: subject.into(), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + at_s * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None,
            trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: body_text.map(str::to_string), body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

fn open_core(dir: &std::path::Path) -> RustCore { RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens") }

// ---------------------------------------------------------------- the three messages (shapes from the parse crate's corpus)

/// A written reply with quoted history: Gmail's shape (the parse corpus `gmail-reply.case`), plain and HTML parts.
fn reply_raw() -> Vec<u8> {
    let text = "I added the two charts you asked for, see slides 6 and 7.\r\n\r\nOn Mon, Nov 9, 2026 at 5:10 PM Victor Lund <victor@example.se> wrote:\r\n> Could you add the sales charts to the deck before Thursday?\r\n";
    let html = "<div dir=\"ltr\">I added the two charts you asked for, see slides 6 and 7.</div><br><div class=\"gmail_quote\"><div dir=\"ltr\" class=\"gmail_attr\">On Mon, Nov 9, 2026 at 5:10 PM Victor Lund &lt;<a href=\"mailto:victor@example.se\">victor@example.se</a>&gt; wrote:<br></div><blockquote class=\"gmail_quote\" style=\"margin:0px 0px 0px 0.8ex;border-left:1px solid rgb(204,204,204);padding-left:1ex\"><div dir=\"ltr\">Could you add the sales charts to the deck before Thursday?</div></blockquote></div>";
    format!("From: Ulla Virtanen <ulla@example.fi>\r\nTo: owner@example.test\r\nSubject: Re: Slides for Thursday\r\nMessage-ID: <reply@example.fi>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"alt\"\r\n\r\n--alt\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{text}\r\n--alt\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n--alt--\r\n").into_bytes()
}

/// A designed newsletter with a style block (the parse corpus `newsletter-style-block.case`).
fn newsletter_raw() -> Vec<u8> {
    let html = "<html><head><style>\nbody { background-color: #f3efe6; margin: 0; }\n.wrap { background-color: #f3efe6; width: 100%; }\n.card { background-color: #ffffff; width: 600px; }\nh1 { font-family: Georgia, serif; color: #2f4f2f; font-size: 28px; }\np { font-family: Helvetica, Arial, sans-serif; color: #333333; line-height: 1.5; }\n.button { background-color: #2f4f2f; color: #ffffff; padding: 12px 20px; }\n.footer p { font-size: 11px; color: #888888; }\n</style></head><body><table class=\"wrap\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\"><tr><td align=\"center\"><table class=\"card\" width=\"600\" cellpadding=\"0\" cellspacing=\"0\"><tr><td><img src=\"https://news.example.com/header.png\" width=\"600\" alt=\"The Garden Letter\"></td></tr><tr><td><h1>November in the garden</h1><p>November in the garden: plant your tulip bulbs now for a spring full of colour.</p><img src=\"https://news.example.com/tulips.jpg\" width=\"600\" alt=\"\"><p>Our autumn workshop on pruning fruit trees opens for booking this week.</p><a class=\"button\" href=\"https://news.example.com/book\">Book a place</a></td></tr><tr><td class=\"footer\"><p>You receive this letter because you subscribed at our shop. <a href=\"https://news.example.com/unsubscribe\">Unsubscribe</a> or <a href=\"https://news.example.com/web\">view it in your browser</a>.</p></td></tr></table></td></tr></table><img src=\"https://track.example.com/open.gif\" width=\"1\" height=\"1\"></body></html>";
    format!("From: The Garden Letter <letter@news.example.com>\r\nTo: owner@example.test\r\nSubject: The Garden Letter, November\r\nMessage-ID: <letter@news.example.com>\r\nMIME-Version: 1.0\r\n\
Content-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n").into_bytes()
}

/// A plain-text mail: two paragraphs, a kept line break and a link.
fn plain_raw() -> Vec<u8> {
    b"From: Ana Silva <ana@example.pt>\r\nTo: owner@example.test\r\nSubject: Minutes\r\nMessage-ID: <minutes@example.pt>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\
The minutes of Monday are at https://docs.example.com/minutes for everyone.\r\n\r\nThe second paragraph has one line\r\nand a kept break.\r\n".to_vec()
}

/// The rows, the data folder, and the scripted server with its runtime (kept alive while the test reads).
#[allow(dead_code)]
struct Setup { dir: tempfile::TempDir, account: String, ids: Vec<String>, rt: tokio::runtime::Runtime, srv: Server }

/// The core opens on a fresh data folder, the three rows are stored without a body, the scripted server holds the three messages;
/// each body is asked for (as the reader opens it) and waited for in the store. Returns the engine runs counted during the fetch.
fn fetch_three() -> (Setup, u64) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    for (uid, raw) in [(11, reply_raw()), (12, newsletter_raw()), (13, plain_raw())] { srv.put(uid, Fixture { raw, sections: vec![] }); }
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![
        row(&account, 11, "reply@example.fi", "Re: Slides for Thursday", 1, None),
        row(&account, 12, "letter@news.example.com", "The Garden Letter, November", 2, None),
        row(&account, 13, "minutes@example.pt", "Minutes", 3, None),
    ]).unwrap();
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let ids: Vec<String> = [11, 12, 13].iter().map(|u| format!("{account}:INBOX:1:{u}")).collect();
    let before = engine::bodies::prepare_calls();
    for id in &ids { let _ = core.body(id.clone()); }
    for id in &ids { wait_until(&format!("{id}: its body and its prepared form are stored"), || store.body_parts(id).unwrap().unwrap().prepared.is_some()); }
    assert!(core.settle_bodies(WAIT));
    let runs = engine::bodies::prepare_calls() - before;
    drop(core);
    (Setup { dir, account, ids, rt, srv }, runs)
}

/// Done-when (2): after the core restarts, a mail that opens carries what the engine found at fetch: the reply folds its history and
/// reads written, the newsletter is designed with its style rules inlined, the plain mail has paragraphs and its link.
/// Red on W1 (nothing stored) and W2 (the read drops the kept result).
#[test]
fn three_fetched_bodies_carry_the_engines_result_after_a_restart() {
    let _g = serial();
    let (s, runs) = fetch_three();
    assert_eq!(runs, 3, "the engine ran once per fetched body");
    let core = open_core(s.dir.path());
    // the values the lane note quotes for the mail view's builder (printed, never asserted here)
    for id in &s.ids {
        let b = core.body(id.clone()).unwrap();
        let cut = |h: &Option<String>| h.as_deref().map(|x| x.chars().take(110).collect::<String>());
        eprintln!("FIELDS {}: layout={:?} fold_from={:?} layout_reasons={:?} body_warnings={:?} display_html={:?} written_html={:?}",
                  id.rsplit(':').next().unwrap(), b.layout, b.fold_from, b.layout_reasons, b.body_warnings, cut(&b.display_html), cut(&b.written_html));
        for (i, g) in b.segments.iter().enumerate() {
            eprintln!("  SEGMENT {i}: kind={:?} visibility={:?} confidence={} reasons={:?} start={} end={} text={:?} safe_html={:?}",
                      g.kind, g.visibility, g.confidence, g.reasons, g.start, g.end, g.text.chars().take(70).collect::<String>(), g.safe_html.chars().take(70).collect::<String>());
        }
    }
    let reply = core.body(s.ids[0].clone()).expect("the reply is stored");
    assert_eq!(reply.layout, Some(BodyLayout::Written), "{reply:?}");
    assert_eq!(reply.fold_from, Some(1), "{:?}", reply.segments);
    let kinds: Vec<(SegmentKind, SegmentVisibility)> = reply.segments.iter().map(|g| (g.kind, g.visibility)).collect();
    assert_eq!(kinds, vec![(SegmentKind::NewContent, SegmentVisibility::Show), (SegmentKind::ReplyHistory, SegmentVisibility::Hide)]);
    assert!(reply.segments[0].text.contains("I added the two charts"), "{:?}", reply.segments[0].text);
    assert!(reply.segments[1].reasons.iter().any(|r| r == "client_quote" || r == "attribution_line" || r == "client_attribution"), "{:?}", reply.segments[1].reasons);
    let written = reply.written_html.as_deref().expect("a written mail has its written document");
    assert!(written.contains("I added the two charts"), "{written}");
    assert!(reply.display_html.as_deref().is_some_and(|h| h.contains("I added the two charts")));
    assert!(reply.plain_text.contains("I added the two charts"), "the reader's existing fields are as before");

    let letter = core.body(s.ids[1].clone()).expect("the newsletter is stored");
    assert_eq!(letter.layout, Some(BodyLayout::Designed), "{:?}", letter.layout_reasons);
    assert!(!letter.layout_reasons.is_empty() && !letter.layout_reasons.iter().any(|r| r == "no_design_signal"), "{:?}", letter.layout_reasons);
    let shown = letter.display_html.as_deref().expect("a designed mail has its display document");
    assert!(shown.contains("color:#2f4f2f") && shown.contains("font-size:28px"), "the style block's rules are inlined: {shown}");
    assert!(!shown.contains("<style"), "the style element itself is gone: {shown}");
    assert_eq!(letter.written_html, None, "a designed mail keeps its paper");
    // engine round 6: two pictures (stated 600 px wide) and one 1x1 pixel; a picture keeps its address only in data-whoosh-src
    assert_eq!((letter.pictures_blocked, letter.pixel_trackers), (2, 1), "pictures and trackers counted apart");
    assert!(shown.contains("data-whoosh-src=\"https://news.example.com/tulips.jpg\""), "the blocked picture keeps its address: {shown}");
    assert!(!shown.contains("src=\"https://news.example.com/tulips.jpg\" ") && !shown.contains(" src=\"https://news.example.com/tulips.jpg\""), "never as src: {shown}");
    assert!(!shown.contains("track.example.com"), "a tracker keeps no address");
    assert!(letter.segments.iter().any(|g| g.kind == SegmentKind::NewContent && g.text.contains("plant your tulip bulbs")));

    let plain = core.body(s.ids[2].clone()).expect("the plain mail is stored");
    assert_eq!(plain.layout, Some(BodyLayout::Written));
    let h = plain.display_html.as_deref().expect("a plain mail is made HTML");
    // edited after its first run: the engine's link carries `rel="noopener noreferrer"` (20-green-scratch.log), the first version wanted the bare tag
    assert!(h.contains("<a href=\"https://docs.example.com/minutes\"") && h.contains(">https://docs.example.com/minutes</a>"), "{h}");
    assert_eq!(h.matches("<p>").count(), 2, "two paragraphs: {h}");
    assert!(h.contains("one line<br>and a kept break"), "the line break is kept: {h}");
    assert!(plain.written_html.is_some());
    assert_eq!(plain.fold_from, None);
    assert!(plain.segments.iter().all(|g| g.visibility == SegmentVisibility::Show), "{:?}", plain.segments);
    drop(core);
    drop(s);
}

/// Done-when (3): reading a stored mail never runs the engine: after the restart, each of the three opens five times and the count of
/// engine runs does not move. Red on W3 (the read runs it again).
#[test]
fn a_stored_mail_is_read_without_running_the_engine() {
    let _g = serial();
    let (s, _) = fetch_three();
    let core = open_core(s.dir.path());
    let before = engine::bodies::prepare_calls();
    for _ in 0..5 {
        for id in &s.ids {
            let b = core.body(id.clone()).expect("stored");
            assert!(b.layout.is_some(), "the kept result is read: {id}");
        }
    }
    assert_eq!(engine::bodies::prepare_calls() - before, 0, "15 opens of stored mail ran the engine");
    drop(core);
    drop(s);
}

/// Done-when (4): a store of the previous schema (version 18, before `message_prepared`) with stored bodies opens under this core; the old
/// bodies read back as before with every delta-26 field empty, never an error, and the engine does not run for them. Red on W4.
#[test]
fn a_store_of_the_previous_version_opens_and_old_bodies_read_with_empty_new_fields() {
    let _g = serial();
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("whoosh.sqlite");
    let id = {
        let store = Store::open(&db).unwrap();
        let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
        let mut r = row(&account, 21, "old@example.test", "Old mail", 1, Some("An old body, stored before the engine.\n\nOn Mon, 9 Nov 2026, Bob wrote:\n> earlier"));
        r.body_html = Some("<p>An old body, stored before the engine.</p>".into());
        store.insert_messages(vec![r]).unwrap();
        format!("{account}:INBOX:1:21")
    };
    {
        let mut c = rusqlite::Connection::open(&db).unwrap();
        store::migrations::revert_to(&mut c, 18).expect("back to the previous version");
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 18);
        let t: i64 = c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'message_prepared'", [], |r| r.get(0)).unwrap();
        assert_eq!(t, 0, "a version-18 store has no prepared table");
    }
    let before = engine::bodies::prepare_calls();
    let core = open_core(dir.path());
    let b = core.body(id.clone()).expect("an old body reads back");
    assert_eq!(b.plain_text, "An old body, stored before the engine.\n\nOn Mon, 9 Nov 2026, Bob wrote:\n> earlier");
    assert_eq!(b.safe_html.as_deref(), Some("<p>An old body, stored before the engine.</p>"));
    assert_eq!((b.segments.len(), b.fold_from, b.layout, b.layout_reasons.len(), b.display_html.clone(), b.written_html.clone(), b.body_warnings.len()),
               (0, None, None, 0, None, None, 0), "every delta-26 field is empty");
    assert_eq!(engine::bodies::prepare_calls() - before, 0, "an old stored body is not prepared on open");
    let c = rusqlite::Connection::open(&db).unwrap();
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
    assert_eq!(v, store::migrations::latest_version());
    // a kept form that cannot be read (cut short) is empty fields too, never an error
    c.execute("INSERT INTO message_prepared (message_id, prepared) VALUES (?2, ?1)", rusqlite::params![r#"{"v":1,"segments":[{"kind":"NEW_CONTENT","text":"cut"#, id]).unwrap();
    let b = core.body(id.clone()).expect("still reads");
    assert_eq!((b.segments.len(), b.layout, b.display_html.clone()), (0, None, None));
    assert_eq!(b.plain_text, "An old body, stored before the engine.\n\nOn Mon, 9 Nov 2026, Bob wrote:\n> earlier");
    drop(core);
}

/// The thread's earlier stored text reaches the engine at fetch: a reply that pastes the earlier message with no quote marks and no
/// attribution is history only by the thread match (the parse corpus `pasted-previous-no-marks` shape), so it collapses. Red on W5.
#[test]
fn the_threads_earlier_text_reaches_the_engine() {
    let _g = serial();
    let earlier = "Can you send me the signed contract for the warehouse lease before Friday so legal can file it?";
    let raw = format!("From: Bo Berg <bo@example.se>\r\nTo: owner@example.test\r\nSubject: Re: Lease\r\nMessage-ID: <lease2@example.se>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\
Here it is, signed on every page.\r\n\r\n{earlier}\r\n").into_bytes();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    srv.put(32, Fixture { raw, sections: vec![] });
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, 31, "lease@example.se", "Lease", 1, Some(earlier)), row(&account, 32, "lease@example.se", "Re: Lease", 2, None)]).unwrap();
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let id = format!("{account}:INBOX:1:32");
    let _ = core.body(id.clone());
    wait_until("the reply is stored with its prepared form", || store.body_parts(&id).unwrap().unwrap().prepared.is_some());
    assert!(core.settle_bodies(WAIT));
    let b = core.body(id).unwrap();
    let kinds: Vec<(SegmentKind, SegmentVisibility)> = b.segments.iter().map(|g| (g.kind, g.visibility)).collect();
    assert_eq!(kinds, vec![(SegmentKind::NewContent, SegmentVisibility::Show), (SegmentKind::ReplyHistory, SegmentVisibility::Collapse)], "{:?}", b.segments);
    assert!(b.segments[1].reasons.iter().any(|r| r.starts_with("thread_")), "{:?}", b.segments[1].reasons);
    drop(core);
}

/// body-r2 finding 2: the older `parse::analyse` (still run at fetch for `safe_html` and `plain_text`) has no nesting cap. A body nested
/// 20,000 deep is kept within 10 s as the engine's escaped text with its warning, and the fetch is free again. Red on W6.
#[test]
fn a_body_nested_past_the_cap_is_kept_quickly_as_text() {
    let _g = serial();
    let html = format!("{}deep words at the bottom{}", "<div>".repeat(20_000), "</div>".repeat(20_000));
    let raw = format!("From: Deep <deep@example.test>\r\nTo: owner@example.test\r\nSubject: Deep\r\nMessage-ID: <deep@example.test>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n").into_bytes();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    srv.put(41, Fixture { raw, sections: vec![] });
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, 41, "deep@example.test", "Deep", 1, None)]).unwrap();
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let id = format!("{account}:INBOX:1:41");
    let t0 = Instant::now();
    let _ = core.body(id.clone());
    while t0.elapsed() < Duration::from_secs(10) && store.body_parts(&id).unwrap().unwrap().prepared.is_none() { std::thread::sleep(Duration::from_millis(20)); }
    let took = t0.elapsed();
    let parts = store.body_parts(&id).unwrap().unwrap();
    assert!(parts.prepared.is_some(), "not kept within 10 s (waited {took:?})");
    eprintln!("deep body kept in {took:?}");
    let b = core.body(id).unwrap();
    assert!(b.plain_text.contains("deep words at the bottom"), "{:?}", &b.plain_text[..b.plain_text.len().min(80)]);
    assert!(b.body_warnings.iter().any(|w| w.contains("nested deeper than the cap")), "{:?}", b.body_warnings);
    assert_eq!(b.safe_html, None, "no HTML was parsed for it");
    if !core.settle_bodies(Duration::from_secs(10)) { panic!("the fetch is still busy 10 s later"); }
    drop(core);
}

/// The read alone (lead's grant, 2026-11-11): a body stored with a prepared form (written here by hand, no fetch, no engine) is read back
/// through `RustCore::body` with every field. Red on W2 (`stored_body` drops the kept form).
#[test]
fn a_stored_prepared_form_reads_back_through_rust_core_body() {
    let _g = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, 51, "kept@example.test", "Kept", 1, None)]).unwrap();
    let id = format!("{account}:INBOX:1:51");
    let json = r#"{"v":1,"segments":[{"kind":"NEW_CONTENT","text":"Hello \"you\"\n","safe_html":"<p>Hello &quot;you&quot;</p>","confidence":0.9,"visibility":"show","reasons":["no_history_signal"],"start":0,"end":12},{"kind":"REPLY_HISTORY","text":"> old \u00e9\n","safe_html":"<blockquote>old é</blockquote>","confidence":0.97,"visibility":"hide","reasons":["attribution_line"],"start":12,"end":21}],"fold_from":1,"layout":"written","layout_reasons":["no_design_signal"],"display_html":"<p>Hello</p><img src=\"cid:whoosh-blocked\" data-whoosh-src=\"https://p.example/a.jpg\">","written_html":"<p>Hello</p>","warnings":["w1"],"pictures_blocked":1,"pixel_trackers":2}"#;
    let parts = store::BodyParts { message_id: id.clone(), account_id: String::new(), folder: String::new(), remote_id: None, preview: "Hello".into(),
                                   body_text: Some("Hello \"you\"\n\n> old é".into()), body_html: Some("<p>Hello</p>".into()), trackers_blocked: 3, prepared: Some(json.into()) };
    assert_eq!(store.set_bodies(vec![parts]).unwrap(), 1);
    let before = engine::bodies::prepare_calls();
    let b = core.body(id).expect("stored");
    assert_eq!(engine::bodies::prepare_calls(), before, "the read runs no engine");
    assert_eq!(b.layout, Some(BodyLayout::Written));
    assert_eq!(b.fold_from, Some(1));
    assert_eq!(b.segments.len(), 2, "{:?}", b.segments);
    assert_eq!((b.segments[0].kind, b.segments[0].visibility, b.segments[0].text.as_str(), b.segments[0].start, b.segments[0].end), (SegmentKind::NewContent, SegmentVisibility::Show, "Hello \"you\"\n", 0, 12));
    assert_eq!((b.segments[1].kind, b.segments[1].visibility, b.segments[1].text.as_str(), b.segments[1].reasons.clone()), (SegmentKind::ReplyHistory, SegmentVisibility::Hide, "> old é\n", vec!["attribution_line".to_string()]));
    assert!((b.segments[1].confidence - 0.97).abs() < 1e-6);
    assert_eq!(b.display_html.as_deref(), Some("<p>Hello</p><img src=\"cid:whoosh-blocked\" data-whoosh-src=\"https://p.example/a.jpg\">"), "the address is served as stored");
    assert_eq!(b.written_html.as_deref(), Some("<p>Hello</p>"));
    assert_eq!((b.layout_reasons.clone(), b.body_warnings.clone()), (vec!["no_design_signal".to_string()], vec!["w1".to_string()]));
    assert_eq!((b.pictures_blocked, b.pixel_trackers, b.trackers_blocked), (1, 2, 3));
    drop(core);
}

/// Engine round 5: a body over PARSE_BODY_BYTES_MAX (8 MiB) fetched over the IMAP fake is kept as the engine's escaped text with the
/// engine's warning, and the old sanitiser is never run on it (no safe_html). Red on W8.
#[test]
fn a_body_over_the_size_cap_is_kept_as_text_with_its_warning() {
    let _g = serial();
    let para = "<p>A long line of a very large newsletter, repeated to pass the size cap of the engine.</p>\r\n";
    let html = format!("<html><body><p>first words of the big one</p>{}</body></html>", para.repeat(parse::BODY_BYTES_MAX / para.len() + 10));
    assert!(html.len() > parse::BODY_BYTES_MAX);
    let raw = format!("From: Big <big@example.test>\r\nTo: owner@example.test\r\nSubject: Big\r\nMessage-ID: <big@example.test>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n{html}\r\n").into_bytes();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    srv.put(61, Fixture { raw, sections: vec![] });
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, 61, "big@example.test", "Big", 1, None)]).unwrap();
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    let id = format!("{account}:INBOX:1:61");
    let _ = core.body(id.clone());
    wait_until("the big body is stored with its prepared form", || store.body_parts(&id).unwrap().unwrap().prepared.is_some());
    assert!(core.settle_bodies(WAIT));
    let b = core.body(id).unwrap();
    assert!(b.body_warnings.iter().any(|w| w.contains("larger than the cap")), "{:?}", b.body_warnings);
    assert!(b.plain_text.chars().take(200).collect::<String>().contains("first words of the big one"), "{:?}", &b.plain_text[..b.plain_text.len().min(80)]);
    assert_eq!(b.safe_html, None, "the old sanitiser did not run on it");
    drop(core);
}

/// Owner's picture 210 (13 Nov 2026): a body stored without a whole engine record (kept before the engine was wired, or stopped by its
/// time limit) gets its record made again from the server's copy at the folder's next pass; the stored text is not touched. Red on W9.
#[test]
fn a_stored_body_without_a_whole_record_gets_one_at_the_next_pass() {
    let _g = serial();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let srv = rt.block_on(Server::start(&[], 1));
    for uid in [31, 32, 33] { srv.put(uid, Fixture { raw: newsletter_raw(), sections: vec![] }); }
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    let stored = |uid: u32, at: i64| {
        let mut r = row(&account, uid, "letter@news.example.com", "The Garden Letter, November", at, Some("The stored text"));
        r.body_html = Some("<p>The stored page</p>".into());
        r
    };
    store.insert_messages(vec![stored(31, 1), stored(32, 2), stored(33, 3)]).unwrap();
    let ids: Vec<String> = [31, 32, 33].iter().map(|u| format!("{account}:INBOX:1:{u}")).collect();
    // 31: no record (stored before the engine); 32: a record the time limit stopped; 33: a whole record, with a mark this test reads back
    let stopped = engine::bodies::prepare_within(&newsletter_raw(), Vec::new(), Duration::ZERO, Duration::ZERO);
    assert!(stopped.degraded);
    let json = engine::reads::prepared_json(&engine::bodies::prepare_within(&newsletter_raw(), Vec::new(), WAIT, Duration::ZERO));
    let whole = format!("{}\"pixel_trackers\":7}}", &json[..json.rfind("\"pixel_trackers\":").expect("the last field")]);
    assert_eq!(store.set_prepared_quiet(vec![(ids[1].clone(), engine::reads::prepared_json(&stopped)), (ids[2].clone(), whole)]).unwrap().len(), 2);
    assert_eq!(core.body(ids[0].clone()).unwrap().layout, None);
    assert_eq!(core.body(ids[1].clone()).unwrap().layout, None, "a stopped record is not served while the page is stored");
    core.set_body_source(&account, Arc::new(server::adapter(&srv, imap_fakes::limits())));
    core.background().folder_synced(&account, "INBOX");
    core.background().kick(&account);
    for id in &ids[..2] {
        wait_until(&format!("{id}: the engine's record is made again"), || core.body(id.clone()).is_some_and(|b| b.layout == Some(BodyLayout::Designed)));
        let b = core.body(id.clone()).unwrap();
        assert!(b.body_warnings.is_empty() && b.display_html.is_some_and(|h| h.contains("<table")), "the designed page, no warning");
        assert_eq!(b.plain_text, "The stored text", "the stored text is not touched");
    }
    assert!(core.settle_bodies(WAIT));
    assert_eq!(core.body(ids[2].clone()).unwrap().pixel_trackers, 7, "a whole record is not made again");
    assert_eq!(store.prepared_missing(&account, "INBOX", 50).unwrap().len(), 0);
    drop(core);
    drop(rt);
}

/// Owner's picture 212 (13 Nov 2026): the record of a forward kept before the parser's rule holds a fold behind a line of dashes. The read
/// serves no such fold (the reader then shows the whole mail); a fold under real words is served as kept.
/// Mutant `stored-fold-behind-a-separator-served` (reads.rs apply_prepared: the `says_something` line removed): red on the first read.
#[test]
fn a_stored_fold_behind_a_line_of_dashes_is_not_served() {
    let _g = serial();
    let dir = tempfile::tempdir().unwrap();
    let core = open_core(dir.path());
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![row(&account, 61, "fwd@example.test", "Fwd: Room", 1, None), row(&account, 62, "fwd@example.test", "Fwd: Room", 2, None)]).unwrap();
    let record = |own: &str| format!(r#"{{"v":1,"segments":[{{"kind":"NEW_CONTENT","text":"{own}\n","safe_html":"<p>{own}</p>","confidence":0.9,"visibility":"show","reasons":["no_history_signal"],"start":0,"end":4}},{{"kind":"FORWARDED_CONTENT","text":"From: Facilities\nRoom 4 is booked.\n","safe_html":"<p>Room 4 is booked.</p>","confidence":0.9,"visibility":"collapse","reasons":["forward_header","forward_subject"],"start":4,"end":40}}],"fold_from":1,"layout":"written","layout_reasons":["no_design_signal"],"display_html":"<p>{own}</p><p>Room 4 is booked.</p>","written_html":"<p>{own}</p><p>Room 4 is booked.</p>","warnings":[],"pictures_blocked":0,"pixel_trackers":0}}"#);
    let parts = |uid: u32, own: &str| store::BodyParts { message_id: format!("{account}:INBOX:1:{uid}"), account_id: String::new(), folder: String::new(), remote_id: None, preview: "Room".into(),
                                                         body_text: Some("Room 4 is booked.".into()), body_html: None, trackers_blocked: 0, prepared: Some(record(own)) };
    assert_eq!(store.set_bodies(vec![parts(61, "---"), parts(62, "FYI")]).unwrap(), 2);
    let dashes = core.body(format!("{account}:INBOX:1:61")).expect("stored");
    assert_eq!((dashes.segments.len(), dashes.fold_from), (2, None), "nothing above the fold says something: the whole mail shows");
    let words = core.body(format!("{account}:INBOX:1:62")).expect("stored");
    assert_eq!((words.segments.len(), words.fold_from), (2, Some(1)), "a fold under the sender's words is served as kept");
}
