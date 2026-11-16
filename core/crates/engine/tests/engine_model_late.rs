//! model-late (SPEC A141): a search typed before the meaning model is handed to the engine still ends ranked by meaning once the
//! model is handed over, and never says the model is missing while the shell is still looking for it. The shell says at launch that
//! it is looking (`expect_background_embedder`); a core never told so keeps today's answer (engine_search_forms.rs
//! `f_no_model_gives_the_word_matches_then_date_order`, unchanged). Each test runs over the hidden test copy's production core on a
//! fresh data folder with the fake embedder of the engine tests (words of one meaning share a direction). The shell's word is a
//! process static, so the tests here run one at a time.
//!
//! Red first, one broken line each (named on each test).
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work, never a budget (the indexer runs at the OS background priority).
const WAIT: Duration = Duration::from_secs(120);
/// How long a test waits to see that no further list came.
const QUIET: Duration = Duration::from_millis(600);
/// The ceiling on `run(Search)`: it never waits for the model.
const RETURNS_AT_ONCE: Duration = Duration::from_millis(1000);

const MEANINGS: [&[&str]; 3] = [&["consultant", "consultants", "advisor", "adviser", "expert"], &["invoice", "bill", "payment"], &["salmon", "fish"]];
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

struct FakeModel;
impl Embedder for FakeModel {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

/// Every `SearchResults` the core sent, in order.
struct Lists(Mutex<Vec<(String, Vec<SearchHit>)>>);
impl EventSink for Lists {
    fn on_event(&self, e: CoreEvent) { if let CoreEvent::SearchResults { query, hits } = e { self.0.lock().unwrap().push((query, hits)); } }
}
impl Lists {
    fn count(&self) -> usize { self.0.lock().unwrap().len() }
    fn get(&self, n: usize) -> (String, Vec<SearchHit>) { self.0.lock().unwrap()[n].clone() }
}

/// The shell's word on the model is one per process: one test at a time.
fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock().unwrap_or_else(|e| e.into_inner())
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

fn mail(account: &str, id: &str, subject: &str, body: Option<&str>, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.unwrap_or("").into(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: body.map(String::from), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

// E1 and E2 hold "consultant(s)": the word matches. M1 is near in meaning (an advisor), M2 far (salmon), N has no body (no vector).
const E1: &str = "e1-consultants";
const E2: &str = "e2-consultant";
const M1: &str = "m1-advisor";
const M2: &str = "m2-salmon";
const N: &str = "n-no-body";
const WITH_BODY: [&str; 4] = [E1, E2, M1, M2];

/// The rig's production core over `dir`, the corpus stored once, the model given to this core when `model` is true, every body
/// handed to the indexer and the indexer settled.
fn production(dir: &std::path::Path, model: bool) -> RustCore {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    if store.account_summaries().unwrap().is_empty() {
        let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![
            mail(&a, E1, "Consultants on site", Some("The consultants arrive Monday."), 1_700_000_001_000),
            mail(&a, E2, "Weekly notes", Some("Our consultant team met today."), 1_700_000_000_500),
            mail(&a, M1, "Budget review", Some("An outside advisor will look at our budget."), 1_700_000_003_000),
            mail(&a, M2, "Lunch", Some("Miso glazed salmon and fish."), 1_700_000_002_000),
            mail(&a, N, "Parking notice", None, 1_700_000_004_000),
        ]).unwrap();
    }
    let bg = core.background();
    if model { bg.set_embedder(Some(Arc::new(FakeModel))); }
    bg.reindex(&WITH_BODY.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

/// A data folder whose meaning index already holds every body's vectors (a launch after an earlier one), reopened with NO model:
/// the state of the owner's Mac in the first moments after launch.
fn relaunched_before_the_model(dir: &std::path::Path) -> RustCore {
    drop(production(dir, true));
    std::thread::sleep(Duration::from_millis(300));
    production(dir, false)
}

fn ids(hits: &[SearchHit]) -> Vec<String> { hits.iter().map(|h| h.message_id.clone()).collect() }

fn listen(core: &RustCore) -> Arc<Lists> {
    let lists = Arc::new(Lists(Mutex::new(vec![])));
    core.subscribe(StreamKind::Search, lists.clone());
    lists
}

/// Runs the search; asserts it is answered, and at once.
fn search(core: &RustCore, query: &str) {
    let t0 = Instant::now();
    let answer = core.run(Command::Search { query: query.into() });
    let took = t0.elapsed();
    assert!(answer.is_ok(), "the real engine answers Search (it answered {answer:?})");
    assert!(took < RETURNS_AT_ONCE, "Search returns at once, it took {took:?}");
}

fn stage(core: &RustCore) -> Option<String> { core.state("search/last/stage".into()) }
fn reason(core: &RustCore) -> Option<String> { core.state("search/last/reason".into()) }

/// The two word matches first, then the rest as given.
fn assert_words_then(list: &[SearchHit], rest: &[&str], what: &str) {
    let got = ids(list);
    let first: HashSet<String> = got.iter().take(2).cloned().collect();
    assert_eq!(first, HashSet::from([E1.to_string(), E2.to_string()]), "{what}: the word matches first: {got:?}");
    assert_eq!(got[2..].to_vec(), rest.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "{what}: then {rest:?}: {got:?}");
}

/// (1) The shell is looking for the model; a search runs before the hand-over: the word list at once, stage "words", no sentence
/// about a missing model. The model is handed over: the same search is ranked again, once: the second list puts M1 (near in meaning)
/// before M2, N (no vector) last; stage "ranked".
/// Red first: indexer.rs `set_embedder`, the line `if let Some(e) = some { if let Some(f) = self.take_waiter() { f(Some(e)); } }`
/// removed: nothing ranks again, "the ranked list after the hand-over" is never reached.
#[test]
fn a_search_before_the_model_is_handed_over_ends_ranked_when_it_comes() {
    let _serial = one_at_a_time();
    engine::bodies::crawl::expect_background_embedder();
    let dir = tempfile::tempdir().unwrap();
    let core = relaunched_before_the_model(dir.path());
    let lists = listen(&core);
    search(&core, "consultant");
    wait_until("the word list", || lists.count() >= 1);
    std::thread::sleep(QUIET);
    assert_eq!(lists.count(), 1, "nothing more while the model is not handed over");
    assert_words_then(&lists.get(0).1, &[N, M1, M2], "the first list (date order after the word matches)");
    assert_eq!(stage(&core).as_deref(), Some("\"words\""), "the model is coming: never no_model");
    assert_eq!(reason(&core).as_deref(), Some("null"), "no sentence about a missing model");

    core.background().set_embedder(Some(Arc::new(FakeModel)));
    wait_until("the ranked list after the hand-over", || lists.count() >= 2);
    let (query, hits) = lists.get(1);
    assert_eq!(query, "consultant", "the same search, as sent");
    assert_words_then(&hits, &[M1, M2, N], "the ranked list");
    assert_eq!(stage(&core).as_deref(), Some("\"ranked\""));
    std::thread::sleep(QUIET);
    assert_eq!(lists.count(), 2, "ranked again once");
}

/// (2) The shell is looking; a search runs; then the shell says the model is not on this Mac (a None hand-over): the word list stays
/// the answer (one list), stage "no_model", with the Sources pane's sentence.
/// Red first: crawl.rs `set_background_embedder`, the line `if embedder.is_none() { b.indexer.said_missing(); }` removed: the stage
/// stays "words".
#[test]
fn a_search_before_the_shell_says_no_model_ends_no_model_with_the_sentence() {
    let _serial = one_at_a_time();
    engine::bodies::crawl::expect_background_embedder();
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), false);
    let lists = listen(&core);
    search(&core, "consultant");
    wait_until("the word list", || lists.count() >= 1);
    assert_eq!(stage(&core).as_deref(), Some("\"words\""), "the model is coming: never no_model before the shell says so");

    engine::bodies::crawl::set_background_embedder(None);
    wait_until("stage no_model", || stage(&core).as_deref() == Some("\"no_model\""));
    let words = reason(&core).unwrap_or_default();
    assert!(words.contains("The meaning model is not on this Mac"), "the no-model words above the rows: {words}");
    std::thread::sleep(QUIET);
    assert_eq!(lists.count(), 1, "one list with no model");
    assert_words_then(&lists.get(0).1, &[N, M1, M2], "no model");
}

/// (3) Bounded: two searches before the hand-over, then a newer one with filters only (sent once, nothing to rank): when the model
/// comes, no older search is ranked again, so no list follows the filtered one and the stage stays "filtered".
/// Red first: search_cmd.rs `fn current(&self, generation: u64) -> bool { self.generation.load(Ordering::SeqCst) == generation }`
/// made `{ true }`: the older search ranks again and its list arrives after the filtered one.
#[test]
fn only_the_last_search_ranks_again_and_only_while_its_list_is_current() {
    let _serial = one_at_a_time();
    engine::bodies::crawl::expect_background_embedder();
    let dir = tempfile::tempdir().unwrap();
    let core = relaunched_before_the_model(dir.path());
    let lists = listen(&core);
    search(&core, "salmon");
    wait_until("the first word list", || lists.count() >= 1);
    search(&core, "consultant");
    wait_until("the second word list", || lists.count() >= 2);
    search(&core, "subject:lunch");
    wait_until("the filtered list", || lists.count() >= 3);
    assert_eq!(stage(&core).as_deref(), Some("\"filtered\""));

    core.background().set_embedder(Some(Arc::new(FakeModel)));
    std::thread::sleep(QUIET * 3);
    let queries: Vec<String> = (0..lists.count()).map(|n| lists.get(n).0).collect();
    assert_eq!(queries, vec!["salmon", "consultant", "subject:lunch"], "no older search is ranked again: {queries:?}");
    assert_eq!(stage(&core).as_deref(), Some("\"filtered\""), "the last list stays the filtered one");
}
