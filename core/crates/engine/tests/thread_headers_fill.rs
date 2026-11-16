//! lane reply-headers (SPEC A144): the fill-in pass that gives the mail already stored its Message-ID and References
//! (engine/src/thread_headers_fill.rs), against a fake `thread_headers_many` source: no socket is opened, no mail server is reached.
//! Not run by this lane (no cargo here): the lead runs `cargo test -p engine`.
//! RED-FIRST (one-line broken copy of engine/src/thread_headers_fill.rs, or bodies.rs, per test):
//!   F1 `fill_folder`: `by_uid.remove(&uid).and_then(from_server)` becomes `None`        -> a_pass_fills_the_answered_rows_and_marks_the_unanswered_once
//!   F2 `fill_folder`: `.map_err(|e| words(&e))?` becomes `.unwrap_or_default()`       -> a_failed_call_changes_nothing_and_the_next_pass_asks_again
//!   F3 `run`: the line `self.write_held();` dropped                                    -> held_arrival_headers_are_written_without_asking_the_server
//!   F4 bodies.rs `folder_synced`: the line `self.headers.folder_synced(account, folder);` dropped -> a_folder_sync_runs_the_pass_in_the_background_on_the_body_hook
//!   F5 `adopt_stored`: the line `Ok(folders) => for f in folders { self.folder_synced(account_id, &f); },` becomes `Ok(_) => {}`
//!                                                                                      -> every_folder_with_mail_stored_before_is_filled_page_after_page
//!   F6 `run`: `Ok(f) if f.asked > 0 => continue,` becomes `Ok(f) if f.asked > 0 => break,` -> every_folder_with_mail_stored_before_is_filled_page_after_page
//!   F7 imap_source.rs `arrival_headers`: `.and_then(store::bracketed)` becomes `.map(str::to_string)`
//!                                                                                      -> the_arrival_and_the_fill_in_store_the_same_bracket_form_and_the_reply_names_it
//!   F8 sync.rs `apply_flag_changes`: `if was_read != Some(seen) {` becomes `if true {`    -> repeated_flags_write_nothing_and_one_changed_seen_writes_once
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::*;
use engine::bodies::thread_headers_fill::{Filled, ThreadHeadersFill, ThreadHeadersSource};
use engine::bodies::Bodies;
use outbox::adapters::reply::StoreReplyContext;
use outbox::ports::ReplyContextPort;
use send::{ParentHeaders, ParentHeadersPort, ParentRef, ThreadKind};
use store::{NewMessage, Store, ThreadHeaders};

const WAIT: Duration = Duration::from_secs(20);

/// The server as the pass sees it: an answer per UID (a UID with none is absent from the result), or a refusal; every ask recorded.
struct FakeServer { answers: HashMap<u32, ParentHeaders>, refuse: bool, asked: Mutex<Vec<(String, String, Vec<u32>)>> }
impl FakeServer {
    fn new(answers: &[(u32, &str, &[&str])], refuse: bool) -> Arc<FakeServer> {
        let answers = answers.iter().map(|(u, m, r)| (*u, ParentHeaders { message_id: m.to_string(), references: r.iter().map(|s| s.to_string()).collect() })).collect();
        Arc::new(FakeServer { answers, refuse, asked: Mutex::new(vec![]) })
    }
    fn asked(&self) -> Vec<(String, String, Vec<u32>)> { self.asked.lock().unwrap().clone() }
}
impl ThreadHeadersSource for FakeServer {
    fn thread_headers_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError> {
        self.asked.lock().unwrap().push((account_id.to_string(), folder.to_string(), uids.to_vec()));
        if self.refuse {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Owner".into(), reason_words: "Server Unavailable. 15".into(), retryable: true, code: None } });
        }
        Ok(uids.iter().filter_map(|u| self.answers.get(u).map(|h| (*u, h.clone()))).collect())
    }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: "owner@example.test".into(), smtp_username: "owner@example.test".into(),
        auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, uid: u32) -> NewMessage { row_in(account, "INBOX", uid) }
fn row_in(account: &str, folder: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:{folder}:1:{uid}"), thread_id: format!("{account}:t{uid}"), account_id: account.into(), sender_name: "S".into(), from_address: "s@ext.example".into(),
            to: vec!["owner@example.test".into()], subject: format!("Subject {uid}"), preview: String::new(), received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000,
            read: false, archived: false, folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![],
            list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// A store with one account and INBOX UIDs 1, 2, 3 (3 the newest), none with headers yet (mail stored before 0023).
fn stored(dir: &tempfile::TempDir) -> (Arc<Store>, String) {
    let store = Arc::new(Store::open(&dir.path().join("whoosh.sqlite")).unwrap());
    let account = store.add_account("owner@example.test", "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages((1..=3).map(|u| row(&account, u)).collect()).unwrap();
    (store, account)
}
fn id(account: &str, uid: u32) -> String { format!("{account}:INBOX:1:{uid}") }
fn runtime() -> tokio::runtime::Runtime { tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap() }

#[test]
fn a_pass_fills_the_answered_rows_and_marks_the_unanswered_once() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored(&dir);
    let rt = runtime();
    let fill = ThreadHeadersFill::new(store.clone(), rt.handle().clone());
    // UID 3 answers without angle brackets (stored with them), UID 2 with its chain, UID 1 is gone from the server (absent)
    let server = FakeServer::new(&[(3, "m3@x.example", &[]), (2, "<m2@x.example>", &["<root@x.example>"])], false);
    fill.set_source(&account, server.clone());
    assert_eq!(fill.fill_folder(&account, "INBOX").unwrap(), Filled { asked: 3, found: 2, marked: 1 });
    assert_eq!(server.asked(), vec![(account.clone(), "INBOX".to_string(), vec![3, 2, 1])], "one call, newest first");
    assert_eq!(store.thread_headers(&id(&account, 3)).unwrap(), Some(ThreadHeaders { message_id: "<m3@x.example>".into(), references: vec![] }));
    assert_eq!(store.thread_headers(&id(&account, 2)).unwrap(), Some(ThreadHeaders { message_id: "<m2@x.example>".into(), references: vec!["<root@x.example>".into()] }));
    assert_eq!(store.thread_headers(&id(&account, 1)).unwrap(), None, "not answered");
    assert_eq!(fill.fill_folder(&account, "INBOX").unwrap(), Filled::default(), "a second pass has nothing to ask");
    assert_eq!(server.asked().len(), 1, "the unanswered UID is not asked on every pass");
}

#[test]
fn a_failed_call_changes_nothing_and_the_next_pass_asks_again() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored(&dir);
    let rt = runtime();
    let fill = ThreadHeadersFill::new(store.clone(), rt.handle().clone());
    fill.set_source(&account, FakeServer::new(&[], true));
    assert!(fill.fill_folder(&account, "INBOX").is_err(), "the refusal is reported to the run's log line");
    assert_eq!(store.thread_headers_unknown(&account, "INBOX", 10).unwrap().len(), 3, "a refused call marks nothing");
    let server = FakeServer::new(&[(1, "<m1@x.example>", &[])], false);
    fill.set_source(&account, server.clone());
    assert_eq!(fill.fill_folder(&account, "INBOX").unwrap(), Filled { asked: 3, found: 1, marked: 2 });
    assert_eq!(store.thread_headers(&id(&account, 1)).unwrap().map(|h| h.message_id), Some("<m1@x.example>".to_string()));
}

#[test]
fn held_arrival_headers_are_written_without_asking_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored(&dir);
    let rt = runtime();
    let fill = Arc::new(ThreadHeadersFill::new(store.clone(), rt.handle().clone()));
    let server = FakeServer::new(&[(2, "<m2@x.example>", &[]), (3, "<m3@x.example>", &[])], false);
    fill.set_source(&account, server.clone());
    // the page the sync engine just stored carried UID 1's headers (ImapSource::page holds them)
    fill.hold([(id(&account, 1), ThreadHeaders { message_id: "<held@x.example>".into(), references: vec!["<r@x.example>".into()] })]);
    fill.folder_synced(&account, "INBOX");
    fill.kick(&account);
    assert!(fill.settle(WAIT), "the pass ends");
    assert_eq!(store.thread_headers(&id(&account, 1)).unwrap(), Some(ThreadHeaders { message_id: "<held@x.example>".into(), references: vec!["<r@x.example>".into()] }));
    assert_eq!(server.asked().iter().flat_map(|(_, _, u)| u.clone()).collect::<Vec<_>>(), vec![3, 2], "the held row is not asked");
    assert_eq!(store.thread_headers_unknown(&account, "INBOX", 10).unwrap(), vec![]);
}

#[test]
fn a_folder_sync_runs_the_pass_in_the_background_on_the_body_hook() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored(&dir);
    let rt = runtime();
    let sinks: engine::sync::Sinks = Arc::new(Mutex::new((0, HashMap::new())));
    let bodies = Arc::new(Bodies::new(store.clone(), sinks, rt.handle().clone()));
    let server = FakeServer::new(&[(1, "<m1@x.example>", &[]), (2, "<m2@x.example>", &[]), (3, "<m3@x.example>", &[])], false);
    bodies.headers.set_source(&account, server.clone());
    // what ImapSource::page and sync::apply_changes call after a folder's page is fetched and stored
    bodies.folder_synced(&account, "INBOX");
    bodies.kick(&account);
    assert!(bodies.headers.settle(WAIT), "the pass ends");
    assert_eq!(store.thread_headers_unknown(&account, "INBOX", 10).unwrap(), vec![], "every stored mail has its headers");
    assert_eq!(server.asked().len(), 1);
}

/// The server answers every UID asked, with `<uid-<n>@x.example>`.
struct EveryUid { asked: Mutex<Vec<(String, usize)>> }
impl ThreadHeadersSource for EveryUid {
    fn thread_headers_many(&self, _account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError> {
        self.asked.lock().unwrap().push((folder.to_string(), uids.len()));
        Ok(uids.iter().map(|u| (*u, ParentHeaders { message_id: format!("<uid-{u}@x.example>"), references: vec![] })).collect())
    }
}

#[test]
fn every_folder_with_mail_stored_before_is_filled_page_after_page() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("whoosh.sqlite")).unwrap());
    let account = store.add_account("owner@example.test", "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    // mail stored before this build: more than one page in Archive, a few in INBOX; no sync names either folder afterwards
    let page = engine::bodies::thread_headers_fill::FILL_PAGE as u32;
    let mut rows: Vec<NewMessage> = (1..=page + 5).map(|u| row_in(&account, "Archive", u)).collect();
    rows.extend((1..=3).map(|u| row_in(&account, "INBOX", u)));
    store.insert_messages(rows).unwrap();
    let rt = runtime();
    let fill = Arc::new(ThreadHeadersFill::new(store.clone(), rt.handle().clone()));
    let server = Arc::new(EveryUid { asked: Mutex::new(vec![]) });
    fill.set_source(&account, server.clone());
    // what `set_adapter` does once the account's adapter is registered
    fill.adopt_stored(&account);
    assert!(fill.settle(WAIT), "the run ends");
    assert_eq!(store.thread_headers_unknown(&account, "Archive", 10).unwrap(), vec![], "every Archive row, past the first page");
    assert_eq!(store.thread_headers_unknown(&account, "INBOX", 10).unwrap(), vec![]);
    let mut asked = server.asked.lock().unwrap().clone();
    asked.sort();
    assert_eq!(asked, vec![("Archive".to_string(), 5), ("Archive".to_string(), page as usize), ("INBOX".to_string(), 3)], "a page per call, each row once");
}

/// A ParentHeadersPort that fails the test when it is called: the reply must read the store.
struct NoServer;
impl ParentHeadersPort for NoServer {
    fn execute(&self, parent: &ParentRef) -> Result<ParentHeaders, CoreError> { panic!("the server was asked for {parent:?}") }
}

fn fetched(account: &str, uid: u32, message_id_header: Option<&str>) -> sync_imap::FetchedMessage {
    let n = row(account, uid);
    sync_imap::FetchedMessage {
        uid, uid_validity: 1, modseq: None, flags: vec![], message_id_header: message_id_header.map(str::to_string), size: None, gmail: None,
        message: n.message, authentication_results: vec![], reply_to: vec![], reply_to_named: vec![], html_head: None, references: vec!["<root@x>".to_string()],
    }
}

#[test]
fn the_arrival_and_the_fill_in_store_the_same_bracket_form_and_the_reply_names_it() {
    // the same mail, uid 1, in two stores: one stored at arrival (mail-parser's id, no brackets), one filled in from the server (brackets)
    let (d1, d2) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (arrived, account1) = stored(&d1);
    let f = fetched(&account1, 1, Some("a@x"));
    let held = engine::imap_source::arrival_headers(&f).expect("a Message-ID was read");
    arrived.write(move |ctx| ctx.set_thread_headers(&[held])).unwrap();
    let (filled, account2) = stored(&d2);
    let rt = runtime();
    let fill = ThreadHeadersFill::new(filled.clone(), rt.handle().clone());
    fill.set_source(&account2, FakeServer::new(&[(1, "<a@x>", &["<root@x>"])], false));
    fill.fill_folder(&account2, "INBOX").unwrap();
    let want = Some(ThreadHeaders { message_id: "<a@x>".into(), references: vec!["<root@x>".into()] });
    assert_eq!(arrived.thread_headers(&id(&account1, 1)).unwrap(), want, "arrival: the bracket form");
    assert_eq!(filled.thread_headers(&id(&account2, 1)).unwrap(), want, "fill-in: the same value");
    // the reply to it, from the store, names it in the bracket form
    let reply = StoreReplyContext::new(arrived.clone(), Arc::new(NoServer)).execute(&format!("{account1}:t1"), ThreadKind::Reply).unwrap();
    assert_eq!(reply.in_reply_to(), Some("<a@x>"));
    assert_eq!(reply.references(), vec!["<root@x>".to_string(), "<a@x>".to_string()]);
}

#[test]
fn repeated_flags_write_nothing_and_one_changed_seen_writes_once() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = stored(&dir);
    let by_uid: HashMap<u32, String> = store.uids_in_folder(&account, "INBOX").unwrap().into_iter().collect();
    let unread = |u: u32| sync_imap::FlagChange { uid: u, flags: vec![], modseq: None, gmail_msg_id: None };
    // a server with no change list: every known UID's flags again, all as stored (unread, not starred)
    let same: Vec<_> = (1..=3).map(unread).collect();
    let applied = engine::sync::apply_flag_changes(&store, &account, "INBOX", &by_uid, same).unwrap();
    assert_eq!((applied.writes, applied.flipped.len()), (0, 0), "nothing differs: no write, nothing refiled");
    // the same list with UID 2 read on the server
    let mut one = (1..=3).map(unread).collect::<Vec<_>>();
    one[1].flags = vec!["\\Seen".to_string()];
    let applied = engine::sync::apply_flag_changes(&store, &account, "INBOX", &by_uid, one).unwrap();
    assert_eq!(applied, engine::sync::FlagsApplied { writes: 1, flipped: vec![id(&account, 2)] });
    assert!(store.message(&id(&account, 2)).unwrap().unwrap().read, "the changed value is stored");
    assert!(!store.message(&id(&account, 1)).unwrap().unwrap().read);
}
