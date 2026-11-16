//! bg-hooks round 2 (shell hand-over of the meaning model). Helpers copied from engine_sync_full_r1.rs.
//! sync-full-r1: every message of every folder is downloaded, stored and indexed for words and for meaning in the background,
//! without slowing a view or an action. Each test runs over the hidden test copy's production core (`open_production(_, rig =
//! true)`) on a fresh data folder, with a fake body source registered for its account (`RustCore::set_body_source`), and drives the
//! background through `RustCore::background()`. Each test was seen red on a single-edit mutant (docs/lanes/sync-full-r1.md).
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use contracts::*;
use engine::bodies::crawl::Conditions;
use engine::bodies::BodySource;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work, never a budget: the indexer runs at the OS background priority, and on a loaded Mac
/// (load 57 measured in a full run) it gets little CPU by design.
const WAIT: Duration = Duration::from_secs(60);
/// The bar: any view or action answers in under 50 ms ("The objective is <50ms for any operation").
const AT_ONCE: Duration = Duration::from_millis(50);
/// The fake server's answer time in (b): a Microsoft 365 body fetch on the owner's Mac costs hundreds of ms.
const SLOW: Duration = Duration::from_millis(300);
const FOLDERS: [(&str, u32); 3] = [("INBOX", 25), ("Archive", 15), ("Sent", 12)];

/// The body the fake server holds for a UID of a folder. Archive 3 is the March invoice (words: "invoice", "payment"); the others
/// are lunch newsletters (words: "almonds", "salmon").
fn body_of(folder: &str, uid: u32) -> Vec<u8> {
    let text = if folder == "Archive" && uid == 3 {
        "Your invoice for March is ready. The payment is due on the first.".to_string()
    } else {
        format!("The Omega Glow Bowl: miso glazed salmon and toasted almonds ({folder} {uid}).")
    };
    format!("From: sender {uid} <s{uid}@example.test>\r\nTo: {USER}\r\nSubject: {folder} {uid}\r\nMessage-ID: <{folder}{uid}@example.test>\r\n\
MIME-Version: 1.0\r\nContent-Type: text/plain; charset=us-ascii\r\n\r\n{text}\r\n").into_bytes()
}

/// A body source that answers after `delay` with `body_of`, and remembers every call (folder, UIDs).
struct FakeSource { delay: Duration, calls: AtomicUsize, asked: Mutex<Vec<(String, Vec<u32>)>> }
impl FakeSource {
    fn new(delay: Duration) -> Arc<Self> { Arc::new(FakeSource { delay, calls: AtomicUsize::new(0), asked: Mutex::new(vec![]) }) }
    fn calls(&self) -> usize { self.calls.load(Ordering::SeqCst) }
    fn asked(&self) -> Vec<(String, Vec<u32>)> { self.asked.lock().unwrap().clone() }
}
#[async_trait]
impl BodySource for FakeSource {
    async fn fetch_bodies(&self, folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push((folder.to_string(), uids.to_vec()));
        tokio::time::sleep(self.delay).await;
        Ok(uids.iter().map(|u| (*u, body_of(folder, *u))).collect())
    }
}

/// The test embedder: words of one meaning share a direction ("bill" and "invoice"; "nuts" and "almonds"), so a query finds a
/// message by meaning with no word in common. It counts its calls per title (each message's subject).
struct MeaningEmbedder { titles: Mutex<HashMap<String, usize>> }
const MEANINGS: [&[&str]; 4] = [&["invoice", "bill", "payment", "pay"], &["almonds", "nuts", "almond"], &["salmon", "fish"], &["march", "spring"]];
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let slot = match MEANINGS.iter().position(|m| m.contains(&w)) {
            Some(m) => m,
            None => 8 + (w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % (dim - 8)),
        };
        v[slot] += 1.0;
    }
    v
}
impl Embedder for MeaningEmbedder {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        *self.titles.lock().unwrap().entry(title.unwrap_or_default()).or_default() += 1;
        self.embed(texts)
    }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

/// The tests of this file run one at a time: (b) times calls in ms, and the other tests' crawls and indexers would share its CPU.
fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

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

fn id_of(account: &str, folder: &str, uid: u32) -> String { format!("{account}:{folder}:1:{uid}") }

/// One stored message with its preview only (or with a body already kept, `kept`), as the sync stores headers.
fn row(account: &str, folder: &str, uid: u32, kept: Option<(Option<&str>, Option<&str>)>) -> NewMessage {
    let id = id_of(account, folder, uid);
    NewMessage {
        message: Message {
            id: id.clone(), thread_id: format!("{account}:<{folder}{uid}@thread>"), account_id: account.into(), sender_name: format!("sender {uid}"),
            from_address: format!("s{uid}@example.test"), to: vec![USER.into()], subject: format!("{folder} {uid}"), preview: format!("preview {folder} {uid}"),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: folder.into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: kept.and_then(|k| k.0.map(String::from)), body_html: kept.and_then(|k| k.1.map(String::from)),
        has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

/// The rig's production core over `dir`, its account (added once: reopening the same folder finds it), the three folders stored
/// with previews only, and `source` registered as the account's server.
fn production(dir: &std::path::Path, source: Arc<FakeSource>) -> (RustCore, String) {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = match store.account_summaries().unwrap().into_iter().next() {
        Some(a) => a.id,
        None => {
            let a = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
            let rows = FOLDERS.iter().flat_map(|(f, n)| (1..=*n).map(move |u| (f, u))).map(|(f, u)| row(&a, f, u, None)).collect();
            store.insert_messages(rows).unwrap();
            a
        }
    };
    core.set_body_source(&account, source);
    (core, account)
}

fn all_ids(account: &str) -> Vec<String> { FOLDERS.iter().flat_map(|(f, n)| (1..=*n).map(move |u| id_of(account, f, u))).collect() }

fn stored(core: &RustCore, ids: &[String]) -> usize { ids.iter().filter(|id| core.store().body_parts(id).unwrap().unwrap().body_text.is_some()).count() }

/// The scripted embedder: counts its calls (one per message, the subject as title) and never loads anything.
struct Counting { calls: AtomicUsize }
impl Counting { fn new() -> Arc<Self> { Arc::new(Counting { calls: AtomicUsize::new(0) }) } fn calls(&self) -> usize { self.calls.load(Ordering::SeqCst) } }
impl Embedder for Counting {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.calls.fetch_add(1, Ordering::SeqCst); Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}
/// Every test starts from what the shell has said nothing about (the statics are per process).
fn fresh_shell() {
    engine::bodies::crawl::set_background_embedder(None);
        engine::bodies::crawl::set_background_conditions(None, true, false);
}

fn five_notes(store: &store::Store, account: &str) {
    store.insert_messages((1..=5).map(|u| row(account, "Notes", u, Some((Some(&format!("Note number {u} about quarterly invoice {u}.")), None)))).collect()).unwrap();
}

/// (n) The model is handed over at launch and nothing is loaded: with no stored body to embed, the embedder is never called. When five
/// bodies are stored and no search was ever made, the indexer embeds exactly those five, in the background. Mutant: `set_embedder`
/// embeds a probe text at once (a load at launch).
#[test]
fn n_the_model_handed_over_at_launch_is_not_called_until_the_indexer_has_a_body_and_then_embeds_five_with_no_search() {
    let _serial = one_at_a_time();
    fresh_shell();
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = production(dir.path(), FakeSource::new(Duration::ZERO));
    let bg = core.background();
    let model = Counting::new();
    engine::bodies::crawl::set_background_embedder(Some(model.clone()));
    assert!(bg.settle_index(WAIT));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(model.calls(), 0, "handed over at launch, zero calls (zero loads) with nothing to embed");
    five_notes(&core.store(), &account);
    wait_until("the five bodies are visible to the indexer's reads", || bg.progress().unwrap().iter().any(|p| p.folder == "Notes" && p.bodies == 5));
    bg.set_embedder(Some(model.clone()));
    wait_until("the five stored bodies are embedded", || model.calls() >= 5);
    assert!(bg.settle_index(WAIT));
    assert_eq!(model.calls(), 5, "exactly the five stored bodies, once each, and no search was made");
    let done = bg.progress().unwrap().into_iter().find(|p| p.folder == "Notes").unwrap();
    assert_eq!((done.messages, done.bodies, done.indexed), (5, 5, 5), "the Sources figure moved: {done:?}");
    fresh_shell();
}

/// (o) Paused means no embedding: on battery under the floor, and under serious heat, the indexer makes no call; released, it embeds.
/// Mutant: the indexer ignores `set_held`.
#[test]
fn o_a_paused_indexer_does_not_embed_and_resumes_when_released() {
    let _serial = one_at_a_time();
    fresh_shell();
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = production(dir.path(), FakeSource::new(Duration::ZERO));
    let bg = core.background();
    five_notes(&core.store(), &account);
    let model = Counting::new();
    for hot in [(Some(20u8), false, false), (Some(90u8), true, true)] {
        bg.set_conditions(Conditions { battery_percent: hot.0, charging: hot.1, thermal_serious: hot.2 });
        bg.set_embedder(Some(model.clone()));
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(model.calls(), 0, "paused ({hot:?}): no embedding");
    }
    bg.set_conditions(Conditions { battery_percent: Some(90), charging: true, thermal_serious: false });
    wait_until("released: the five are embedded", || model.calls() >= 5);
    assert!(bg.settle_index(WAIT));
    assert_eq!(model.calls(), 5);
    fresh_shell();
}

/// (p) No model on this Mac: an enabled mail folder says so in one plain line (Failed, MODEL_MISSING_WORDS) instead of "syncing 0 %" for ever,
/// the word search over the crawled mail still answers, and once a model is handed over the folder reaches up to date. Mutants: the
/// folder keeps the contract's Syncing; handing a model over leaves the missing flag set.
#[test]
fn p_without_the_model_a_folder_says_so_and_word_search_still_answers() {
    use engine::bodies::crawl::MODEL_MISSING_WORDS;
    let _serial = one_at_a_time();
    fresh_shell();
    let dir = tempfile::tempdir().unwrap();
    let (core, account) = production(dir.path(), FakeSource::new(Duration::from_millis(2)));
    let store = core.store();
    let inbox = ReplySourceRef::MailFolder { account_id: account.clone(), folder: "INBOX".into() };
    store.reply_source_ensure(inbox.clone(), 1).unwrap();
    store.reply_set_source_enabled(inbox.clone(), true, 1).unwrap();
    struct Seen(Mutex<Vec<CoreEvent>>);
    impl EventSink for Seen { fn on_event(&self, e: CoreEvent) { self.0.lock().unwrap().push(e); } }
    let seen = Arc::new(Seen(Mutex::new(vec![])));
    core.subscribe(StreamKind::SyncStatus, seen.clone());
    let last = |seen: &Seen| seen.0.lock().unwrap().iter().rev().find_map(|e| match e { CoreEvent::ReplySourceProgress { source, progress, .. } if *source == inbox => Some(progress.clone()), _ => None });
    let bg = core.background();
    engine::bodies::crawl::set_background_embedder(None);   // what the shell does at launch when the weights are absent
    // the word index opens on the first body the indexer sees and the indexer cuts no chunk without a model, so the words are proven on a note
    bg.crawl_kick(&account);
    let ids = all_ids(&account);
    wait_until("every body is stored", || stored(&core, &ids) == ids.len());
    assert!(bg.settle_crawl(WAIT) && bg.settle_index(WAIT));
    let words = MODEL_MISSING_WORDS.to_string();
    wait_until("the folder says the model is missing", || last(&seen) == Some(ReplySyncProgress::Failed { reason_words: words.clone() }));
    let figures = bg.progress().unwrap().into_iter().find(|p| p.folder == "INBOX").unwrap();
    assert_eq!(figures.indexed, 0);
    assert_eq!(bg.folder_source(figures.clone(), true).progress, ReplySyncProgress::Failed { reason_words: words.clone() });
    assert_eq!(bg.folder_source(figures.clone(), false).progress, ReplySyncProgress::NotStarted, "a folder that is off says nothing about the model");
    assert!(MODEL_MISSING_WORDS.len() < 120 && !MODEL_MISSING_WORDS.contains("0 %"), "one plain line");
    let by_word = bg.search_words("invoice", 5);
    assert_eq!(by_word.first().map(|h| h.message_id.clone()), Some(id_of(&account, "Archive", 3)), "word search answers with no model: {by_word:?}");
    assert!(bg.search_meaning("bill", 3).unwrap().is_empty(), "no meaning hit without a model, and no error");
    // the model arrives (installed, handed over): the folder goes on to up to date
    engine::bodies::crawl::set_background_embedder(Some(Counting::new()));
    bg.set_embedder(Some(Counting::new()));
    assert!(!bg.meaning_model_missing(), "handing a model over clears the missing state");
    assert!(bg.settle_index(WAIT));
    wait_until("the folder is up to date", || last(&seen) == Some(ReplySyncProgress::UpToDate));
    fresh_shell();
}
