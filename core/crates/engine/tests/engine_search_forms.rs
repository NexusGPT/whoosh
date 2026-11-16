//! search-forms (SPEC A141): the owner's search on the real engine uses the word index AND the meaning index and ranks EVERY stored
//! mail: the word matches first (a typed word also finds the words that start with it), then every other mail by closeness of
//! meaning down to no likeness, then the mails with no vector yet by date. Each test runs over the hidden test copy's production core
//! (`open_production(_, rig = true)`) on a fresh data folder, with the fake embedder of the engine tests (words of one meaning share
//! a direction), and reads the lists the core sends on the Search stream. On 399d0d59 every test fails at its first assertion: the
//! core answered `Command::Search` with not-built-yet.
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work, never a budget (the indexer runs at the OS background priority).
const WAIT: Duration = Duration::from_secs(120);
/// The list of a model run: how long a test waits to see that no further list came.
const QUIET: Duration = Duration::from_millis(600);
/// The ceiling on `run(Search)`: it must not wait for the model, which the gated embedder holds for as long as the test likes.
const RETURNS_AT_ONCE: Duration = Duration::from_millis(1000);
/// The old meaning cut-off (MEANING_MIN_COSINE): a mail below it must still be listed.
const OLD_CUT_OFF: f64 = 0.65;

/// Words of one meaning share a direction: "advisor" is near "consultant" with no letter in common at the start.
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

/// The test embedder. With a gate, `embed_query` waits until the gate opens (the model loading for seconds); documents never wait.
struct FakeModel { gate: Option<Arc<(Mutex<bool>, Condvar)>> }
impl Embedder for FakeModel {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> {
        if let Some(g) = &self.gate {
            let (open, cv) = &**g;
            let guard = open.lock().unwrap();
            let _ = cv.wait_timeout_while(guard, WAIT, |o| !*o).unwrap();
        }
        Ok(vector(&text))
    }
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

fn mail(account: &str, id: &str, folder: &str, subject: &str, body: Option<&str>, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.unwrap_or("").into(), received_at_ms: at, read: false, archived: folder == "Archive",
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: body.map(String::from), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

// The corpus. Two word matches for "consultant": E1 holds only "consultants" (subject and body), E2 holds "consultant" in its body
// only, in the Archive folder. M1 shares no word with "consultant" and is near it in meaning (an advisor); M2 is far (salmon, under
// the old cut-off); N has no body, so no vector; J holds "consultants" in the Junk folder (out of scope).
const E1: &str = "e1-consultants";
const E2: &str = "e2-consultant-archived";
const M1: &str = "m1-advisor";
const M2: &str = "m2-salmon";
const N: &str = "n-no-body";
const J: &str = "j-junk";
const WITH_BODY: [&str; 5] = [E1, E2, M1, M2, J];

/// The rig's production core over `dir`, the account and the corpus stored once (reopening the same folder finds them), the
/// model given when `model` is Some, every body handed to the indexer, and the indexer settled.
fn production(dir: &std::path::Path, model: Option<Arc<FakeModel>>) -> RustCore {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    if store.account_summaries().unwrap().is_empty() {
        let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
        store.insert_messages(vec![
            mail(&a, E1, "INBOX", "Consultants on site", Some("The consultants arrive Monday."), 1_700_000_001_000),
            mail(&a, E2, "Archive", "Weekly notes", Some("Our consultant team met today."), 1_700_000_000_500),
            mail(&a, M1, "INBOX", "Budget review", Some("An outside advisor will look at our budget."), 1_700_000_003_000),
            mail(&a, M2, "INBOX", "Lunch", Some("Miso glazed salmon and fish."), 1_700_000_002_000),
            mail(&a, N, "INBOX", "Parking notice", None, 1_700_000_004_000),
            mail(&a, J, "Junk", "Consultants cheap", Some("Cheap consultants for you."), 1_700_000_005_000),
        ]).unwrap();
    }
    let bg = core.background();
    if let Some(m) = model { bg.set_embedder(Some(m)); }
    bg.reindex(&WITH_BODY.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

fn ids(hits: &[SearchHit]) -> Vec<String> { hits.iter().map(|h| h.message_id.clone()).collect() }

/// Subscribes to the Search stream and runs the search; asserts the command is answered, and at once.
fn search(core: &RustCore, query: &str) -> Arc<Lists> {
    let lists = Arc::new(Lists(Mutex::new(vec![])));
    core.subscribe(StreamKind::Search, lists.clone());
    let t0 = Instant::now();
    let answer = core.run(Command::Search { query: query.into() });
    let took = t0.elapsed();
    assert!(answer.is_ok(), "the real engine answers Search (it answered {answer:?})");
    eprintln!("run(Search {query:?}) returned in {took:?}");
    assert!(took < RETURNS_AT_ONCE, "Search returns at once, it took {took:?}");
    lists
}

/// The two word matches first (in the word index's order), then the rest as given.
fn assert_words_then(list: &[SearchHit], rest: &[&str], what: &str) {
    let got = ids(list);
    let first: HashSet<String> = got.iter().take(2).cloned().collect();
    assert_eq!(first, HashSet::from([E1.to_string(), E2.to_string()]), "{what}: the word matches first: {got:?}");
    assert_eq!(got[2..].to_vec(), rest.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "{what}: then {rest:?}: {got:?}");
}

/// (1) `Command::Search` is answered by the real engine (it answered not-built-yet), and a list arrives on the Search stream for the
/// query string as sent.
#[test]
fn a_search_is_answered_and_a_list_arrives_on_the_search_stream() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), None);
    let lists = search(&core, "consultant");
    wait_until("a list on the Search stream", || lists.count() >= 1);
    let (query, hits) = lists.get(0);
    assert_eq!(query, "consultant", "the query string as sent");
    assert!(!hits.is_empty(), "never an empty list while mail exists");
}

/// (2) "consultant" finds E1, which holds only "consultants", as a word match: it is in the first part of the list, its subject
/// tinted over the whole word "Consultants".
#[test]
fn b_consultant_finds_a_mail_holding_only_consultants_as_a_word_match() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), None);
    let lists = search(&core, "consultant");
    wait_until("a list", || lists.count() >= 1);
    let (_, hits) = lists.get(0);
    let at = ids(&hits).iter().position(|id| id == E1).unwrap_or_else(|| panic!("E1 is listed: {:?}", ids(&hits)));
    assert!(at < 2, "E1 is a word match, ranked with the word matches: {:?}", ids(&hits));
    let e1 = &hits[at];
    assert_eq!(e1.matched_in, MatchedIn::SubjectWords { ranges: vec![TextRange { start: 0, end: 11 }] }, "the subject's \"Consultants\" is the match");
    assert_eq!(core.state("search/last/query".into()).as_deref(), Some("\"consultant\""));
}

/// (3) With the model: after the word matches, every other mail by closeness of meaning (M1, the advisor, before M2, the salmon),
/// then the mail with no vector (N) last; each mail once; the Junk mail out of scope.
#[test]
fn c_a_mail_with_no_shared_word_follows_the_word_matches_by_closeness_of_meaning() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), Some(Arc::new(FakeModel { gate: None })));
    let lists = search(&core, "consultant");
    wait_until("the ranked list", || lists.count() >= 2);
    let (query, hits) = lists.get(1);
    assert_eq!(query, "consultant");
    assert_words_then(&hits, &[M1, M2, N], "the ranked list");
    let score = |id: &str| hits.iter().find(|h| h.message_id == id).unwrap().score;
    assert!(score(M1) > score(M2), "M1 is closer than M2: {} vs {}", score(M1), score(M2));
    let matched = |id: &str| hits.iter().find(|h| h.message_id == id).unwrap().matched_in.clone();
    assert_eq!(matched(M1), MatchedIn::MeaningOnly, "the meaning group says meaning");
    assert_eq!(matched(N), MatchedIn::MeaningOnly, "the mail with no vector sits under the divider");
    // SPEC A149 changes this expectation: "Every match SHOWS why: the letters in bold in the subject or the sender, or one line of
    // the text around them" (was `SubjectWords { ranges: vec![] }`, the contract then having no text value)
    assert_eq!(matched(E2), MatchedIn::TextWords { ranges: vec![TextRange { start: 4, end: 14 }] }, "a body word match is never sent as meaning");
    let divider = hits.iter().position(|h| h.matched_in == MatchedIn::MeaningOnly).expect("a meaning group");
    assert_eq!(divider, 2, "the first meaning hit follows the two word matches");
    assert!(hits[divider..].iter().all(|h| h.matched_in == MatchedIn::MeaningOnly), "nothing exact after the divider");
    assert!(score(N) < score(M2), "the mail with no vector has the lowest score: {} vs {}", score(N), score(M2));
    assert_eq!(core.state("search/last/stage".into()).as_deref(), Some("\"ranked\""));
    assert_eq!(core.state("search/last/reason".into()).as_deref(), Some("null"));
}

/// (4) No mail is dropped by a score: every stored mail in scope is listed once, M2 included though its cosine is under the old
/// cut-off; the Junk mail is the only one left out.
#[test]
fn d_no_mail_is_dropped_by_a_score() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), Some(Arc::new(FakeModel { gate: None })));
    let lists = search(&core, "consultant");
    wait_until("the ranked list", || lists.count() >= 2);
    let (_, hits) = lists.get(1);
    let got = ids(&hits);
    let unique: HashSet<String> = got.iter().cloned().collect();
    assert_eq!(unique.len(), got.len(), "each mail once: {got:?}");
    let in_scope: HashSet<String> = [E1, E2, M1, M2, N].iter().map(|s| s.to_string()).collect();
    assert_eq!(unique, in_scope, "every mail in scope, Junk out: {got:?}");
    let m2 = hits.iter().find(|h| h.message_id == M2).unwrap();
    assert!(m2.score < OLD_CUT_OFF, "M2 is under the old cut-off ({}), and listed", m2.score);
}

/// (5) A restart (a new engine over the same store and index folders) gives the same list.
#[test]
fn e_a_restart_gives_the_same_list() {
    let dir = tempfile::tempdir().unwrap();
    let first = {
        let core = production(dir.path(), Some(Arc::new(FakeModel { gate: None })));
        let lists = search(&core, "consultant");
        wait_until("the ranked list", || lists.count() >= 2);
        let list = ids(&lists.get(1).1);
        drop(core);
        list
    };
    std::thread::sleep(Duration::from_millis(300));
    let core = production(dir.path(), Some(Arc::new(FakeModel { gate: None })));
    let lists = search(&core, "consultant");
    wait_until("the ranked list after the restart", || lists.count() >= 2);
    assert_eq!(ids(&lists.get(1).1), first, "the same list after the restart");
}

/// (6) No model on this Mac: the word matches, then every other mail by date (newest first: N, M1, M2), sent once.
#[test]
fn f_no_model_gives_the_word_matches_then_date_order() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), None);
    let lists = search(&core, "consultant");
    wait_until("a list", || lists.count() >= 1);
    std::thread::sleep(QUIET);
    assert_eq!(lists.count(), 1, "one list with no model");
    assert_words_then(&lists.get(0).1, &[N, M1, M2], "no model");
    assert_eq!(core.state("search/last/stage".into()).as_deref(), Some("\"no_model\""));
    let reason = core.state("search/last/reason".into()).unwrap_or_default();
    assert!(reason.contains("The meaning model is not on this Mac"), "the no-model words above the rows: {reason}");
}

/// (7) The model takes its time on the query: the word list is sent first (words, then date order) while the query's vector is not
/// ready, and the ranked list replaces it once the vector comes; the command returned before either.
#[test]
fn g_the_list_is_sent_twice_when_the_vector_comes_later() {
    let dir = tempfile::tempdir().unwrap();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let core = production(dir.path(), Some(Arc::new(FakeModel { gate: Some(gate.clone()) })));
    let lists = search(&core, "consultant");
    wait_until("the word list", || lists.count() >= 1);
    std::thread::sleep(QUIET);
    assert_eq!(lists.count(), 1, "nothing more while the query's vector is not ready");
    assert_words_then(&lists.get(0).1, &[N, M1, M2], "the first list");
    assert_eq!(core.state("search/last/stage".into()).as_deref(), Some("\"words\""));
    assert_eq!(core.state("search/last/reason".into()).as_deref(), Some("null"));
    { let (open, cv) = &*gate; *open.lock().unwrap() = true; cv.notify_all(); }
    wait_until("the ranked list", || lists.count() >= 2);
    let (query, second) = lists.get(1);
    assert_eq!(query, "consultant", "the same query string");
    assert_words_then(&second, &[M1, M2, N], "the second list");
    let a: HashMap<String, usize> = ids(&lists.get(0).1).into_iter().enumerate().map(|(i, id)| (id, i)).collect();
    assert_eq!(a.len(), second.len(), "the second list holds the same mails");
}
