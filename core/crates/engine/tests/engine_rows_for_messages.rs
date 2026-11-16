//! rows-engine (SPEC A141): the search ranks every stored mail in scope, and the screen draws each hit with
//! `WhooshCore::rows_for_messages(hit ids)`. Through the rig's production core (`open_production(_, rig = true)`) on a fresh data
//! folder, no model (one list, words then date): every hit id comes back as a row, in the hits' order, the Sent mail, the archived
//! mail and the inbox mail beyond the first INBOX_PAGE_ROWS included. The account points at 127.0.0.1 (loopback only).
//! Failure messages name message ids only, never a subject or a preview.
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::{NewMessage, INBOX_PAGE_ROWS};

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work (the indexer, the search thread), never a budget.
const WAIT: Duration = Duration::from_secs(60);

const SENT: &str = "s-consultants-reply";
const ARCHIVED: &str = "a-consultant-archived";
const INBOX_HIT: &str = "e-consultants";
const JUNK: &str = "j-junk";
/// Inbox mails with no word in common with the query, more than a page of them.
const FILLER: usize = INBOX_PAGE_ROWS + 15;

struct Lists(Mutex<Vec<(String, Vec<SearchHit>)>>);
impl EventSink for Lists {
    fn on_event(&self, e: CoreEvent) { if let CoreEvent::SearchResults { query, hits } = e { self.0.lock().unwrap().push((query, hits)); } }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: &str, thread: &str, folder: &str, subject: &str, body: Option<&str>, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.unwrap_or("").into(), received_at_ms: at, read: false, archived: folder == "Archive",
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: body.map(String::from), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

fn filler_id(i: usize) -> String { format!("f{i:03}") }

/// A search's hit ids all come back as rows, in the hits' order, the Sent mail included.
/// Broken copy (red): core/crates/engine/src/rust_core.rs reads
/// `fn rows_for_messages(&self, message_ids: Vec<String>) -> Vec<InboxRow> { self.store.rows_for(&message_ids).unwrap_or_default() }`
/// (the smart-folder read, which keeps only what the inbox shows).
#[test]
fn every_hit_of_a_search_comes_back_as_a_row_the_sent_mail_included() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let base = 1_700_000_000_000i64;
    let mut batch = vec![
        mail(&a, INBOX_HIT, "t-cons", "INBOX", "Consultants on site", Some("The consultants arrive Monday."), base + 500_000),
        mail(&a, SENT, "t-cons", "Sent", "Re: Consultants on site", Some("Thanks, the consultants can use room 4."), base + 600_000),
        mail(&a, ARCHIVED, "t-arch", "Archive", "Weekly notes", Some("Our consultant team met today."), base + 1_000),
        mail(&a, JUNK, "t-junk", "Junk", "Consultants cheap", Some("Cheap consultants for you."), base + 700_000),
    ];
    // the filler is newer than the archived mail and older than the two inbox/sent hits; f000 is the oldest filler
    batch.extend((0..FILLER).map(|i| mail(&a, &filler_id(i), &format!("t-f{i:03}"), "INBOX", "Parking notice", None, base + 10_000 + i as i64 * 1_000)));
    store.insert_messages(batch).unwrap();
    let bg = core.background();
    bg.reindex(&[INBOX_HIT, SENT, ARCHIVED, JUNK].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert!(bg.settle_index(WAIT), "the indexer finished");

    // premise: the inbox read (all tab) lists neither the Sent mail, nor the archived one, nor the oldest filler
    let page = store.inbox(&InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    assert_eq!(page.len(), INBOX_PAGE_ROWS, "premise: a full page");
    for id in [SENT, ARCHIVED, "f000"] { assert!(!page.iter().any(|r| r.message_id == id), "premise: {id} is not on the inbox page"); }

    let lists = Arc::new(Lists(Mutex::new(vec![])));
    core.subscribe(StreamKind::Search, lists.clone());
    assert!(core.run(Command::Search { query: "consultant".into() }).is_ok(), "the search is answered");
    let t0 = Instant::now();
    while lists.0.lock().unwrap().is_empty() {
        assert!(t0.elapsed() < WAIT, "no list on the Search stream within {WAIT:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    let (query, hits) = lists.0.lock().unwrap()[0].clone();
    assert_eq!(query, "consultant");
    let hit_ids: Vec<String> = hits.iter().map(|h| h.message_id.clone()).collect();
    for id in [SENT, ARCHIVED, INBOX_HIT, "f000"] { assert!(hit_ids.iter().any(|h| h == id), "premise: the search lists {id}: {hit_ids:?}"); }
    assert!(!hit_ids.iter().any(|h| h == JUNK), "premise: the Junk mail is out of the search's scope");

    let t = Instant::now();
    let rows = core.rows_for_messages(hit_ids.clone());
    eprintln!("rows_for_messages: {} hit ids, {} rows in {:?}", hit_ids.len(), rows.len(), t.elapsed());
    let row_ids: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect();
    assert_eq!(row_ids, hit_ids, "one row per hit, in the hits' order");
    let sent = rows.iter().find(|r| r.message_id == SENT).expect("the Sent mail has its row");
    assert_eq!((sent.thread_id.as_str(), sent.thread_count, sent.account_id.as_str()), ("t-cons", 2, a.as_str()), "the Sent mail's own thread and count");
    let inbox_hit = rows.iter().find(|r| r.message_id == INBOX_HIT).unwrap();
    // read again now: the core's background work may have filed the mail (its category) since the premise's read
    let page_now = store.inbox(&InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }).unwrap();
    let from_page = page_now.iter().find(|r| r.message_id == INBOX_HIT).expect("the inbox hit is on the page");
    assert!(inbox_hit == from_page, "an inbox mail's row equals its inbox row field for field");
}

/// An id the store does not hold (an outbox draft id, a deleted mail) gives no row; the core never errors on it.
/// Broken copy (red): rust_core.rs reads `fn rows_for_messages(&self, message_ids: Vec<String>) -> Vec<InboxRow> { self.store.rows_for_messages(&message_ids[..message_ids.len().min(1)]).unwrap_or_default() }`.
#[test]
fn an_unknown_id_gives_no_row_through_the_core() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![
        mail(&a, "x1", "t-x1", "INBOX", "One", None, 1_700_000_000_000),
        mail(&a, "x2", "t-x2", "Sent", "Two", None, 1_700_000_001_000),
    ]).unwrap();
    let rows = core.rows_for_messages(vec!["gone".into(), "x2".into(), "x1".into()]);
    assert_eq!(rows.iter().map(|r| r.message_id.as_str()).collect::<Vec<_>>(), ["x2", "x1"]);
}

/// Owner's store, 14 Nov 2026: a tab of 178 conversations showed 50 and Down stopped on the 50th. `inbox` serves the first page;
/// `inbox_rows` serves as many rows as the list asks for, the same rows first, in the same order.
/// Broken copy (red): rust_core.rs `inbox_rows` passes INBOX_PAGE_ROWS to the read whatever `rows` says.
#[test]
fn the_list_is_served_past_its_first_page() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    let base = 1_700_000_000_000i64;
    store.insert_messages((0..FILLER).map(|i| mail(&a, &filler_id(i), &format!("t-f{i:03}"), "INBOX", "Parking notice", None, base + i as i64 * 1_000)).collect()).unwrap();
    let q = || InboxQuery { tab_id: "all".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None };
    let ids = |rows: Vec<InboxRow>| rows.into_iter().map(|r| r.message_id).collect::<Vec<_>>();
    let page = ids(core.inbox(q()));
    assert_eq!(page.len(), INBOX_PAGE_ROWS, "the first page");
    let more = ids(core.inbox_rows(q(), (INBOX_PAGE_ROWS + 10) as u32));
    assert_eq!(more.len(), INBOX_PAGE_ROWS + 10, "ten rows past the page");
    assert_eq!(more[..INBOX_PAGE_ROWS], page[..], "the first page comes first, in its order");
    let all = ids(core.inbox_rows(q(), 10_000));
    assert_eq!(all.len(), FILLER, "every conversation of the tab");
    assert_eq!(all.last().map(String::as_str), Some("f000"), "the oldest is the last row");
    assert_eq!(ids(core.inbox_rows(q(), INBOX_PAGE_ROWS as u32)), page, "the page's own count gives the page");
}
