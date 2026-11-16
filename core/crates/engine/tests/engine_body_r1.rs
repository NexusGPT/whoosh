//! body-r1: a message's body is downloaded in the background, never while the reader waits; a preview is decoded text, never
//! raw MIME. (b) and (c) run over the production core of the hidden test copy (`open_production(_, rig = true)`), so the mutant
//! hook `WHOOSH_CORE_MUTANT=body-fetch-sync` reaches them; the body source is a fake that answers late or refuses
//! (`RustCore::set_body_source`). (d) runs the real sync against sync-imap's in-process IMAP fake. Each test was seen red on its
//! mutant (docs/lanes/body-r1.md).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use contracts::constants::{BODY_PREFETCH_BATCH, BODY_PREFETCH_COUNT};
use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::bodies::BodySource;
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const WAIT: Duration = Duration::from_secs(20);
/// The adapter's answer time in (b): a Microsoft 365 body fetch on the owner's Mac costs hundreds of ms (SELECT + FETCH).
const SLOW: Duration = Duration::from_millis(500);
/// What the reader may spend in `body()` (b): well under one frame budget, far under one server round trip.
const AT_ONCE: Duration = Duration::from_millis(50);

/// The owner's sweetgreen newsletter (uid 143411 on main56) as the server holds it: multipart/alternative, quoted-printable.
fn sweetgreen(tag: &str) -> Vec<u8> {
    format!("From: sweetgreen <hello@sweetgreen.example>\r\nTo: {USER}\r\nSubject: Omega Glow Bowl {tag}\r\nMessage-ID: <{tag}@sweetgreen.example>\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/alternative; boundary=\"--d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04\"\r\n\r\n\
----d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n\
The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almo=\r\nnds ({tag}).\r\n\r\n\
----d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04\r\nContent-Transfer-Encoding: quoted-printable\r\nContent-Type: text/html; charset=us-ascii\r\n\r\n\
<html><body><p>The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almo=\r\nnds ({tag}).</p></body></html>\r\n\
----d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04--\r\n").into_bytes()
}

/// What the sync stored as the preview of that message before body-r1 (the owner's database, 36 of 52 rows like it).
const RAW_PREVIEW: &str = "----d58e6804b42fcd7ff1ee0a7a0a61d95ea7fedb6f4ba88fd5e2a798310c04 Content-Transfer-Encoding: quoted-printable Content-Type: text/plain; charset=us-ascii The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almo= nds";

/// A body source that answers after `delay`, with a sweetgreen-shaped body per UID, or refuses every call.
struct FakeSource { delay: Duration, refuse: bool, calls: AtomicUsize, asked: Mutex<Vec<Vec<u32>>> }
impl FakeSource {
    fn new(delay: Duration, refuse: bool) -> Arc<Self> { Arc::new(FakeSource { delay, refuse, calls: AtomicUsize::new(0), asked: Mutex::new(vec![]) }) }
    fn calls(&self) -> usize { self.calls.load(Ordering::SeqCst) }
}
#[async_trait]
impl BodySource for FakeSource {
    async fn fetch_bodies(&self, _folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push(uids.to_vec());
        tokio::time::sleep(self.delay).await;
        if self.refuse {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Owner".into(), reason_words: "Server Unavailable. 15".into(), retryable: true, code: None } });
        }
        Ok(uids.iter().map(|u| (*u, sweetgreen(&format!("uid {u}")))).collect())
    }
}

struct Events(Mutex<Vec<CoreEvent>>);
impl EventSink for Events { fn on_event(&self, e: CoreEvent) { self.0.lock().unwrap().push(e); } }
impl Events {
    fn threads(&self, id: &str) -> usize { self.0.lock().unwrap().iter().filter(|e| matches!(e, CoreEvent::ThreadChanged { thread } if thread.id == id)).count() }
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// One stored INBOX message with its preview only, as the sync keeps it before the body arrives.
fn row(account: &str, uid: u32, preview: &str) -> NewMessage {
    let id = format!("{account}:INBOX:1:{uid}");
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{account}:<{uid}@thread>"), account_id: account.into(), sender_name: "sweetgreen".into(), from_address: "hello@sweetgreen.example".into(),
            to: vec![USER.into()], subject: format!("Omega Glow Bowl {uid}"), preview: preview.into(), received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

/// The hidden test copy's production core over a fresh data folder, one account, `uids` stored with their preview only, and a
/// subscription to the Thread stream. The mutant hook is read from the environment, as the rig copy reads it.
fn production(uids: &[u32], preview: &str, source: Arc<FakeSource>) -> (RustCore, String, Arc<Events>, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    store.insert_messages(uids.iter().map(|u| row(&account, *u, preview)).collect()).unwrap();
    core.set_body_source(&account, source);
    let events = Arc::new(Events(Mutex::new(vec![])));
    core.subscribe(StreamKind::Thread, events.clone());
    (core, account, events, dir)
}

/// (b) The reader never waits on the network: `body()` of a message without a body answers in under 50 ms while the server takes
/// 500 ms; the body is stored after, its preview rewritten, and ONE ThreadChanged announces it. Mutants: the hook
/// `body-fetch-sync` (body() waits again) and no event emitted.
#[test]
fn b_body_answers_at_once_and_the_downloaded_body_lands_with_one_thread_event() {
    let source = FakeSource::new(SLOW, false);
    let (core, account, events, _d) = production(&[143411], "The Omega Glow Bowl: miso glazed salmon", source.clone());
    let id = format!("{account}:INBOX:1:143411");
    let thread = format!("{account}:<143411@thread>");
    let t0 = Instant::now();
    let first = core.body(id.clone()).expect("a body answer");
    let took = t0.elapsed();
    assert!(took < AT_ONCE, "body() waited {took:?} for a server that answers in {SLOW:?}");
    assert_eq!(first.plain_text, "The Omega Glow Bowl: miso glazed salmon", "the decoded preview while the body downloads");
    wait_until("the downloaded body is stored", || core.store().body_parts(&id).unwrap().unwrap().body_text.is_some());
    let parts = core.store().body_parts(&id).unwrap().unwrap();
    assert!(parts.body_text.as_deref().unwrap().contains("toasted almonds (uid 143411)"), "{:?}", parts.body_text);
    assert!(parts.preview.starts_with("The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almonds"), "{:?}", parts.preview);
    wait_until("the thread is announced", || events.threads(&thread) >= 1);
    assert!(core.settle_bodies(WAIT));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(events.threads(&thread), 1, "one ThreadChanged for the landed body");
    let again = core.body(id).unwrap();
    assert!(again.plain_text.contains("toasted almonds") && again.safe_html.is_some(), "the next read is the stored body");
    assert_eq!(source.calls(), 1, "downloaded once");
}

/// (e) A downloaded body is kept in whoosh.sqlite and served after a relaunch with no network: the body lands, the core is dropped
/// (its body source with it), the same data folder is opened again, and `body()` answers the stored body at once with no download
/// asked (the new core's source counts zero calls, `core.log` holds no line for the message). Mutant: body() ignores the stored body.
#[test]
fn e_a_downloaded_body_is_served_after_a_relaunch_without_the_network() {
    let source = FakeSource::new(Duration::from_millis(5), false);
    let (core, account, _events, dir) = production(&[143411], "The Omega Glow Bowl: miso glazed salmon", source.clone());
    let id = format!("{account}:INBOX:1:143411");
    let _ = core.body(id.clone());
    wait_until("the downloaded body is stored", || core.store().body_parts(&id).unwrap().unwrap().body_text.is_some());
    assert!(core.settle_bodies(WAIT));
    drop(core);
    drop(source);
    // the relaunch: the same data folder, a fresh core; the rig keychain is in memory, so the account's sync cannot start (no network)
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens again");
    let offline = FakeSource::new(SLOW, true);
    core.set_body_source(&account, offline.clone());
    let t0 = Instant::now();
    let b = core.body(id.clone()).expect("the stored body");
    let took = t0.elapsed();
    assert!(took < AT_ONCE, "body() took {took:?} after a relaunch");
    assert!(b.plain_text.contains("toasted almonds (uid 143411)"), "the stored text: {:?}", b.plain_text);
    assert!(b.safe_html.as_deref().is_some_and(|h| h.contains("Omega Glow Bowl")), "the stored html: {:?}", b.safe_html);
    assert!(core.settle_bodies(Duration::from_millis(1)), "no download runs");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(offline.calls(), 0, "no download asked for a stored body");
    let log = std::fs::read_to_string(dir.path().join("core.log")).unwrap_or_default();
    assert!(!log.contains(&format!("body {id}")), "no download attempt logged: {log:?}");
}

/// (c) A download that failed is not tried again within BODY_FETCH_RETRY_S, however often the reader shows the message, and
/// its reason is kept in `<data_dir>/core.log`. Mutant: retry on every visit.
#[test]
fn c_a_failed_download_is_not_tried_again_within_the_retry_time() {
    let source = FakeSource::new(Duration::from_millis(50), true);
    let (core, account, events, dir) = production(&[7, 8], "Your order is ready", source.clone());
    let id = format!("{account}:INBOX:1:7");
    assert_eq!(core.body(id.clone()).unwrap().plain_text, "Your order is ready");
    wait_until("the first attempt ended", || source.calls() == 1 && core.settle_bodies(Duration::from_millis(1)));
    for _ in 0..5 {
        assert_eq!(core.body(id.clone()).unwrap().plain_text, "Your order is ready", "the preview while the body is missing");
    }
    assert!(core.settle_bodies(WAIT));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(source.calls(), 1, "a failed body is not downloaded again before the retry time: {:?}", source.asked.lock().unwrap());
    // a message that did not fail is still downloaded
    let _ = core.body(format!("{account}:INBOX:1:8"));
    wait_until("the other message's attempt", || source.calls() == 2);
    let log = std::fs::read_to_string(dir.path().join("core.log")).expect("core.log is written");
    assert!(log.contains(&format!("body {id}: the server did not send it: Server Unavailable. 15")), "{log:?}");
    assert_eq!(events.threads(&format!("{account}:<7@thread>")), 0, "nothing landed, nothing announced");
    assert!(core.store().body_parts(&id).unwrap().unwrap().body_text.is_none());
}

/// (d, store side) The prefetch downloads the newest BODY_PREFETCH_COUNT bodies of a folder that have none, BODY_PREFETCH_BATCH
/// UIDs per server call, newest first, and rewrites their previews: a preview a sync before body-r1 stored raw is repaired.
/// The reader shows nothing rather than a raw preview while such a body downloads.
#[test]
fn d_the_prefetch_stores_the_newest_bodies_in_batches_and_repairs_raw_previews() {
    let source = FakeSource::new(Duration::from_millis(5), false);
    let uids: Vec<u32> = (1..=60).collect();
    let (core, account, _events, _d) = production(&uids, RAW_PREVIEW, source.clone());
    assert_eq!(core.body(format!("{account}:INBOX:1:1")).unwrap().plain_text, "", "a raw preview is never shown as the body");
    core.prefetch_bodies(&account, "INBOX");
    wait_until("the prefetch ran", || source.calls() >= 1);
    assert!(core.settle_bodies(WAIT));
    let store = core.store();
    // the reader's own ask for uid 1 is one call; every other call is the prefetch's
    let asked: Vec<Vec<u32>> = source.asked.lock().unwrap().iter().filter(|b| b.as_slice() != [1]).cloned().collect();
    let prefetched: Vec<u32> = asked.iter().flatten().copied().collect();
    assert!(asked.iter().all(|b| b.len() <= BODY_PREFETCH_BATCH as usize), "batches of at most {BODY_PREFETCH_BATCH}: {asked:?}");
    assert_eq!(prefetched.first(), Some(&60), "newest first: {asked:?}");
    let has_body = |u: u32| store.body_parts(&format!("{account}:INBOX:1:{u}")).unwrap().unwrap().body_text.is_some();
    let kept: Vec<u32> = uids.iter().copied().filter(|u| has_body(*u)).collect();
    let newest: Vec<u32> = (60 - BODY_PREFETCH_COUNT as u32 + 1..=60).collect();
    assert!(newest.iter().all(|u| kept.contains(u)), "the newest {BODY_PREFETCH_COUNT} have their body: kept {kept:?}");
    assert!((2..=10).all(|u| !has_body(u)), "older messages wait for their open: kept {kept:?}");
    let p = store.body_parts(&format!("{account}:INBOX:1:60")).unwrap().unwrap().preview;
    assert!(p.starts_with("The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almonds") && !p.contains("Content-") && !p.contains("--"), "{p:?}");
    let row = core.inbox(InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None })
        .into_iter().find(|r| r.message_id.ends_with(":60")).expect("the row");
    assert!(!row.preview.contains("Content-Type"), "the list row reads the repaired preview: {:?}", row.preview);
}

/// (d, sync side) After the real sync fetches a folder, the newest bodies are already stored before anyone opens them, asked
/// BODY_PREFETCH_BATCH UIDs per `UID FETCH ... (UID BODY.PEEK[])`; the oldest wait. The preview the sync stored is decoded.
#[test]
fn d_after_a_fetch_the_newest_bodies_arrive_unasked() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 85);
    {
        let mut g = imap.state.lock().unwrap();
        let inbox = g.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        let newest = inbox.msgs.iter_mut().max_by_key(|m| m.uid).unwrap();
        newest.raw = sweetgreen("newest");
    }
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector, TestTransport::PlainLoopback);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let account = out.created_id.unwrap();
    let store = core.store();
    wait_until("the 85 rows", || store.uids_in_folder(&account, "INBOX").map(|v| v.len()).unwrap_or(0) == 85);
    wait_until("the newest bodies, unasked", || store.bodies_missing(&account, "INBOX", BODY_PREFETCH_COUNT as usize).map(|m| m.is_empty()).unwrap_or(false));
    assert!(core.settle_bodies(WAIT));
    // sync-full-r1 (owner 2026-11-10: "are all messages downloaded stored and embedded automatically in the background too ?"): the newest
    // BODY_PREFETCH_COUNT came first, unasked (above); the crawl then keeps the older ones too, in calls the loop below checks
    wait_until("every body, the crawl's too", || store.bodies_missing(&account, "INBOX", 85).map(|m| m.is_empty()).unwrap_or(false));
    assert!(core.background().settle_crawl(WAIT));
    let body_fetches: Vec<String> = imap.log().into_iter().filter(|l| l.to_ascii_uppercase().contains("BODY.PEEK[])")).collect();
    assert!(!body_fetches.is_empty());
    for l in &body_fetches {
        let set = l.split_whitespace().skip_while(|w| !w.eq_ignore_ascii_case("FETCH")).nth(1).unwrap_or("");
        let n: usize = set.split(',').map(|r| match r.split_once(':') { Some((a, b)) => b.parse::<usize>().unwrap() - a.parse::<usize>().unwrap() + 1, None => 1 }).sum();
        assert!(n <= BODY_PREFETCH_BATCH as usize, "one call asks at most {BODY_PREFETCH_BATCH} bodies: {l}");
    }
    let (newest_uid, newest_id) = store.uids_in_folder(&account, "INBOX").unwrap().into_iter().max().unwrap();
    assert_eq!(newest_uid, 85);
    let parts = store.body_parts(&newest_id).unwrap().unwrap();
    assert!(parts.body_text.as_deref().unwrap_or("").contains("toasted almonds (newest)"), "{:?}", parts.body_text);
    assert!(parts.preview.starts_with("The Omega Glow Bowl: miso glazed salmon, sweet corn salsa, and toasted almonds"), "{:?}", parts.preview);
}

/// (d2, sync side, the crawl held) With the background crawl held, only the prefetch stores bodies after a fetch: exactly the newest
/// BODY_PREFETCH_COUNT of the folder, and the oldest wait. Test (d) lets the crawl finish the rest, so a prefetch that asked for none
/// (8ee25032 m1: `bodies_missing(.., 0)`) left it green; here nothing else can fill the gap.
#[test]
fn d2_with_the_crawl_held_the_prefetch_alone_stores_the_newest_bodies() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 85);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let detector: Box<dyn engine::Detect> = Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default()));
    let core = RustCore::with_parts(store, Arc::new(MemoryKeychain::new(Namespace::rig())), detector, TestTransport::PlainLoopback);
    core.background().hold_crawl(true);
    let out = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added");
    let account = out.created_id.unwrap();
    let store = core.store();
    wait_until("the 85 rows", || store.uids_in_folder(&account, "INBOX").map(|v| v.len()).unwrap_or(0) == 85);
    let left = BODY_PREFETCH_COUNT as usize;
    wait_until("the newest bodies, unasked", || store.bodies_missing(&account, "INBOX", 85).map(|m| m.len() <= 85 - left).unwrap_or(false));
    // (no settle: a prefetch run that stays queued behind the held crawl never settles, which is what this test holds on to)
    std::thread::sleep(Duration::from_millis(1_000));
    let missing = store.bodies_missing(&account, "INBOX", 85).unwrap();
    assert_eq!(missing.len(), 85 - left, "exactly the newest {left} are stored by the prefetch, the oldest wait for the crawl");
    assert!(missing.iter().all(|(uid, _)| (*uid as usize) <= 85 - left), "the ones still waiting are the oldest: {missing:?}");
}
