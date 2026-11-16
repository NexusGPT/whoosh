//! The real core, slice 2: the mail reads from the store, and the IMAP sync that fills it. The reads run over a seeded
//! store; the sync runs against sync-imap's in-process IMAP fake (clear text on 127.0.0.1, no outside network). Each test
//! was seen red on a mutant (docs/lanes/contracts.md "## Real core, slice 2").
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, SyncPhase, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace, SecretKind, StoreSecret, Secret};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
/// How long a sync against the in-process fake may take before a test calls it stuck (the fake answers in milliseconds).
const SYNC_WAIT: Duration = Duration::from_secs(20);

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn tab(t: &str) -> InboxQuery { InboxQuery { tab_id: t.into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None } }
fn mailbox(m: Mailbox) -> InboxQuery { InboxQuery { mailbox: Some(m), ..tab("primary") } }

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < SYNC_WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within {SYNC_WAIT:?}: {what}");
}

fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn msg(account: &str, folder: &str, n: i64, read: bool) -> NewMessage {
    let id = format!("{account}:{folder}:{n}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: id.clone(), account_id: account.into(), sender_name: format!("Sender {n}"), from_address: format!("sender{n}@other.example"),
            to: vec![USER.into()], subject: format!("Subject {n}"), preview: format!("Preview {n}"), received_at_ms: 1_700_000_000_000 + n * 1000, read, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec!["cc@other.example".into()], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(n.to_string()), body_text: if n == 1 { Some(format!("Full body {n}")) } else { None }, body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}

/// A store with one account holding 3 inbox messages (one read) and 2 in "Sent", and the core over it.
fn seeded() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages(vec![msg(&id, "INBOX", 1, false), msg(&id, "INBOX", 2, true), msg(&id, "INBOX", 3, false), msg(&id, "Sent", 4, true), msg(&id, "Sent", 5, true)]).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), id, dir)
}

#[test]
fn the_inbox_reads_the_stored_rows_newest_first_with_counts() {
    let (core, id, _d) = seeded();
    let rows = core.inbox(tab("primary"));
    assert_eq!(rows.iter().map(|r| r.subject.as_str()).collect::<Vec<_>>(), vec!["Subject 3", "Subject 2", "Subject 1"]);
    assert!(rows.iter().all(|r| r.account_id == id));
    // delta 16: the counts carry Primary then All, so the unread of the inbox is the Primary view's, not a sum of tabs
    let primary = core.counts().tabs.into_iter().find(|t| t.tab_id == "primary").expect("the Primary view's counts");
    assert_eq!(primary.unread, 2, "{:?}", core.counts());
    assert_eq!(core.accounts()[0].unread, 2);
}

#[test]
fn a_mailbox_reads_its_role_folder_and_filters_keep_only_passing_rows() {
    let (core, id, _d) = seeded();
    let sent = core.inbox(mailbox(Mailbox::Sent));
    assert_eq!(sent.iter().map(|r| r.subject.as_str()).collect::<Vec<_>>(), vec!["Subject 5", "Subject 4"]);
    assert!(core.inbox(mailbox(Mailbox::Trash)).is_empty());
    let unread = core.inbox(InboxQuery { filters: vec![InboxFilter::Unread], ..tab("primary") });
    assert_eq!(unread.iter().map(|r| r.subject.as_str()).collect::<Vec<_>>(), vec!["Subject 3", "Subject 1"]);
    let other = core.inbox(InboxQuery { account_ids: vec!["acc-nobody".into()], ..tab("primary") });
    assert!(other.is_empty(), "an account set without this account shows none of its rows");
    let mine = core.inbox(InboxQuery { account_ids: vec![id], ..tab("primary") });
    assert_eq!(mine.len(), 3);
    assert!(core.inbox(InboxQuery { smart_folder_id: Some("sf-1".into()), ..tab("primary") }).is_empty());
}

#[test]
fn one_message_its_thread_and_its_stored_body_read_back() {
    let (core, id, _d) = seeded();
    let mid = format!("{id}:INBOX:1");
    let m = core.message(mid.clone()).expect("the message");
    assert_eq!((m.subject.as_str(), m.cc.clone(), m.folder.as_str(), m.read, m.is_outgoing), ("Subject 1", vec!["cc@other.example".to_string()], "INBOX", false, false));
    assert_eq!(core.thread(mid.clone()).expect("its thread").message_ids, vec![mid.clone()]);
    let b = core.body(mid).expect("its body");
    assert_eq!((b.plain_text.as_str(), b.safe_html), ("Full body 1", None));
    // no body stored and no server to ask: the preview is the body, nothing invented
    assert_eq!(core.body(format!("{id}:INBOX:2")).unwrap().plain_text, "Preview 2");
    assert!(core.message("nope".into()).is_none() && core.body("nope".into()).is_none());
}

struct Events(Mutex<Vec<CoreEvent>>);
impl EventSink for Events { fn on_event(&self, e: CoreEvent) { self.0.lock().unwrap().push(e); } }

/// The fake with 3 inbox messages and 1 sent, an SMTP fake for the sign-in check, and the account added through the core.
fn added(core_never_stores: bool) -> (RustCore, String, FakeImap, tokio::runtime::Runtime, Arc<Events>, tempfile::TempDir) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 3);
    imap.seed("Sent", 1);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let core = if core_never_stores { RustCore::with_parts_never_storing(store, kc(), detector(), TestTransport::PlainLoopback) }
               else { RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback) };
    let events = Arc::new(Events(Mutex::new(vec![])));
    core.subscribe(StreamKind::SyncStatus, events.clone());
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    (core, out.created_id.unwrap(), imap, rt, events, dir)
}

#[test]
fn an_added_account_syncs_its_folders_into_the_store() {
    let (core, id, _imap, _rt, events, _d) = added(false);
    wait_until("three inbox rows", || core.inbox(tab("primary")).len() == 3);
    let rows = core.inbox(tab("primary"));
    assert!(rows.iter().all(|r| r.account_id == id && r.unread));
    assert_eq!(rows[0].subject, "Message 3", "newest first");
    let primary = core.counts().tabs.into_iter().find(|t| t.tab_id == "primary").expect("the Primary view's counts");
    assert_eq!(primary.unread, 3);
    wait_until("the sent row", || core.inbox(mailbox(Mailbox::Sent)).len() == 1);
    wait_until("history fetched", || core.sync_states().iter().any(|s| s.account_id == id && s.history_fetched && s.last_error.is_none()));
    assert!(events.0.lock().unwrap().iter().any(|e| matches!(e, CoreEvent::SyncStateChanged { .. })), "SyncStatus events reach the subscriber");
    assert_eq!(core.sync_states().iter().find(|s| s.account_id == id).unwrap().messages_indexed, 4);
}

#[test]
fn opening_a_synced_message_fetches_its_body_once() {
    let (core, id, imap, _rt, _e, _d) = added(false);
    wait_until("listening (the history is in, so no page fetch runs beside the opens)", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    let mid = core.inbox(tab("primary"))[0].message_id.clone();
    let b = core.body(mid.clone()).expect("a body");
    assert!(b.plain_text.contains("Body of message 3 is here."), "{:?}", b.plain_text);
    imap.clear_log();
    let again = core.body(mid).unwrap();
    assert_eq!(again.plain_text, b.plain_text);
    assert!(!imap.log().iter().any(|l| l.to_ascii_uppercase().contains("FETCH")), "the second open reads the kept body: {:?}", imap.log());
}

#[test]
fn new_mail_arrives_by_idle_after_the_history() {
    let (core, id, imap, _rt, _e, _d) = added(false);
    wait_until("listening", || core.sync_phase(&id) == Some(SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    imap.add_message("INBOX");
    wait_until("the fourth inbox row", || core.inbox(tab("primary")).len() == 4);
}

#[test]
fn a_refused_login_lands_on_the_sync_state_in_the_servers_words() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| s.refuse_login = Some("Too many sign-ins from this address".into()));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let keychain = kc();
    let r = StoreSecret::execute(keychain.as_ref(), SecretKind::AccountCredential, USER, &Secret::new(PW.as_bytes().to_vec())).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, &format!("{}\u{1f}{}", r.service, r.account), false, config(imap.port, 1), 1).unwrap();
    let core = RustCore::with_parts(store, keychain, detector(), TestTransport::PlainLoopback);
    core.start_stored_accounts();
    wait_until("the failure on the sync state", || core.sync_states().iter().any(|s| s.account_id == id && s.last_error.is_some()));
    let f = core.sync_states().into_iter().find(|s| s.account_id == id).unwrap().last_error.unwrap();
    assert!(f.reason_words.contains("Too many sign-ins from this address"), "{}", f.reason_words);
    assert_eq!(core.accounts()[0].health.as_ref().map(|h| h.reason_words.clone()), Some(f.reason_words));
    assert!(core.inbox(tab("primary")).is_empty());
}

#[test]
fn a_sync_that_never_stores_leaves_the_list_empty() {
    // the seam the hidden check's mutant uses (WHOOSH_CORE_MUTANT=sync-never-stores): proves the check can see it
    let (core, id, _imap, _rt, _e, _d) = added(true);
    wait_until("the engine finished", || core.sync_states().iter().any(|s| s.account_id == id && s.history_fetched));
    assert!(core.inbox(tab("primary")).is_empty());
}

#[test]
fn every_other_command_is_still_refused_in_words() {
    let (core, _id, _d) = seeded();
    // fix-blockforget: BlockSender is served now (block_cmds.rs): one row of the block table, no undo entry (it was refused "does not do this yet")
    let out = core.run(Command::BlockSender { address: "someone@example.test".into() }).expect("BlockSender is served");
    assert_eq!((out.undo_entry_id, core.blocks().len()), (None, 1));
}

// ---------------------------------------------------------------- delta 16: Primary and All are views, not labels

fn filed(account: &str, n: i64, read: bool, category: &str) -> NewMessage { NewMessage { tab_id: Some(category.into()), ..msg(account, "INBOX", n, read) } }
fn subjects(rows: &[InboxRow]) -> Vec<String> { rows.iter().map(|r| r.subject.clone()).collect() }
fn tab_counts(core: &RustCore, id: &str) -> TabCounts { core.counts().tabs.into_iter().find(|t| t.tab_id == id).unwrap_or_else(|| panic!("no counts for {id}: {:?}", core.counts())) }

/// One account: 3 unfiled inbox messages (2 unread), one unread filed in Updates, one read filed in Promotions, one unread in
/// Junk, one unread in Trash, one in Sent. Primary = the 3 unfiled; All = the 5 inbox rows; Junk, Trash and Sent in neither.
fn seeded_views() -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages(vec![
        msg(&id, "INBOX", 1, false), msg(&id, "INBOX", 2, true), msg(&id, "INBOX", 3, false),
        filed(&id, 6, false, "updates"), filed(&id, 9, true, "promotions"),
        msg(&id, "Junk", 7, false), msg(&id, "Trash", 8, false), msg(&id, "Sent", 4, true),
    ]).unwrap();
    (RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback), id, dir)
}

#[test]
fn the_all_view_lists_every_inbox_row_of_every_category_and_never_junk_or_trash() {
    let (core, _id, _d) = seeded_views();
    let all = core.inbox(tab("all"));
    assert_eq!(subjects(&all), ["Subject 9", "Subject 6", "Subject 3", "Subject 2", "Subject 1"], "every inbox row, newest first");
    assert!(!subjects(&all).iter().any(|s| s == "Subject 7" || s == "Subject 8" || s == "Subject 4"), "Junk, Trash and Sent are not in All");
    assert_eq!(subjects(&core.inbox(tab("primary"))), ["Subject 3", "Subject 2", "Subject 1"], "Primary is the unfiled mail only");
    assert_eq!(subjects(&core.inbox(tab("updates"))), ["Subject 6"]);
    // Unread is a filter on any tab, a view included
    let unread_all = core.inbox(InboxQuery { filters: vec![InboxFilter::Unread], ..tab("all") });
    assert_eq!(subjects(&unread_all), ["Subject 6", "Subject 3", "Subject 1"]);
    assert!(core.inbox(tab("primary")).len() <= all.len(), "All >= Primary");
}

#[test]
fn the_counts_carry_primary_then_all_then_the_tabs_and_all_is_never_below_primary() {
    let (core, _id, _d) = seeded_views();
    let tabs = core.counts().tabs;
    assert_eq!(tabs.iter().take(2).map(|t| t.tab_id.as_str()).collect::<Vec<_>>(), ["primary", "all"], "{tabs:?}");
    assert!(tabs.iter().skip(2).all(|t| inbox_view_from_id(t.tab_id.clone()).is_none()), "a view id appears once, first: {tabs:?}");
    let (primary, all) = (tab_counts(&core, "primary"), tab_counts(&core, "all"));
    assert_eq!(primary.unread, 2, "the unfiled unread");
    assert_eq!(all.unread, 3, "every category's unread (2 unfiled + 1 in Updates), Junk and Trash excluded");
    assert!(all.unread >= primary.unread && all.waiting >= primary.waiting, "All >= Primary: {all:?} {primary:?}");
    assert_eq!(all.waiting, 5, "every inbox thread awaits a reply; the Junk and Trash ones are not counted");
}

#[test]
fn hiding_the_primary_split_tab_does_not_move_the_default_category() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let id = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages(vec![msg(&id, "INBOX", 1, false), msg(&id, "INBOX", 2, false)]).unwrap();
    // a store seeded before delta 16 holds a "primary" split_tab row; hidden, it must not hand the unfiled mail to the next tab
    store.write(|ctx| ctx.tx.execute("UPDATE split_tab SET enabled = 0 WHERE id = 'primary'", []).map(|_| ()).map_err(Into::into)).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    assert_eq!(core.inbox(tab("primary")).len(), 2, "the unfiled mail stays in Primary");
    assert!(core.inbox(tab("needs-reply")).is_empty(), "and does not move to the first enabled tab");
    assert_eq!(tab_counts(&core, "primary").unread, 2);
}

#[test]
fn the_label_reads_serve_no_view_row_and_the_strip_starts_with_the_two_views() {
    let (core, _id, _d) = seeded_views();
    // the store's seed keeps its built-in "primary" label row (pinned); it is a view, so neither read serves it
    assert!(core.labels().iter().all(|l| inbox_view_from_id(l.id.clone()).is_none()), "{:?}", core.labels().iter().map(|l| &l.id).collect::<Vec<_>>());
    assert!(core.pinned_labels().iter().all(|l| l.id != "primary"), "{:?}", core.pinned_labels().iter().map(|l| &l.id).collect::<Vec<_>>());
    assert!(core.labels().iter().any(|l| l.id == "updates"), "the real built-in labels are still served");
    let strip = core.state("inbox/tabs".into()).expect("the strip");
    assert!(strip.starts_with(r#"[{"id":"primary","kind":"view","name":"Primary","unread":2,"waiting":3},{"id":"all","kind":"view","name":"All","unread":3,"waiting":5},"#), "{strip}");
    assert_eq!(strip.matches(r#""id":"primary""#).count(), 1, "Primary is never shown twice: {strip}");
    assert!(strip.contains(r#"{"id":"updates","kind":"label","name":"Updates","unread":1,"waiting":1}"#), "a pinned label follows the views: {strip}");
}

#[test]
fn the_view_state_paths_answer_as_the_fixture_does() {
    let (core, _id, _d) = seeded_views();
    let st = |p: &str| core.state(p.into());
    assert_eq!(st("view/primary/name").as_deref(), Some(r#""Primary""#));
    assert_eq!(st("view/all/name").as_deref(), Some(r#""All""#));
    assert_eq!(st("view/primary/count").as_deref(), Some("3"));
    assert_eq!(st("view/all/count").as_deref(), Some("5"), "every inbox row, Junk and Trash excluded");
    assert_eq!(st("view/all/unread").as_deref(), Some("3"));
    assert_eq!(st("view/primary/unread").as_deref(), Some("2"));
    assert_eq!(st("view/all/waiting").as_deref(), Some("5"));
    assert_eq!(st("view/primary/is_label").as_deref(), Some("false"));
    assert_eq!(st("view/updates/name"), None, "a label is not a view");
    assert_eq!(st("view/all/colour"), None);
}

#[test]
fn a_view_refuses_pin_and_every_label_door_in_words_and_a_real_label_passes_the_door() {
    let (core, _id, _d) = seeded_views();
    let refusal = |c: Command| match core.run(c) {
        Err(CoreError::Invalid { field, reason_words }) => (field, reason_words),
        other => panic!("not a view refusal: {other:?}"),
    };
    let (field, words) = refusal(Command::PinLabel { label_id: "primary".into(), pinned: true });
    assert_eq!(field, VIEW_REFUSAL_CODE);
    assert_eq!(words, "Primary is a view of the inbox, not a label, so it cannot be pinned: the views are always the first tabs.");
    assert!(refusal(Command::PinLabel { label_id: "all".into(), pinned: false }).1.starts_with("All is a view of the inbox"));
    assert_eq!(refusal(Command::DeleteLabel { label_id: "all".into() }).0, VIEW_REFUSAL_CODE);
    assert_eq!(refusal(Command::CreateLabel { name: "primary".into(), parent_id: None }).0, VIEW_REFUSAL_CODE);
    // the door lets a real label through to the core's own answer; real-labels: this core pins now (label_cmds.rs), so the answer is Ok
    // and the label reads back pinned (it was "does not do this yet" before the label commands were served)
    let pinned = core.run(Command::PinLabel { label_id: "updates".into(), pinned: true });
    assert!(pinned.is_ok(), "a real label is not refused: {pinned:?}");
    assert_eq!(core.state("label/updates/pinned".into()).as_deref(), Some("true"), "label updates pinned");
}
