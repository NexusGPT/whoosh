//! query-embed (SPEC A141): when the model fails on the query, the engine's log keeps the model's own words
//! (`search: the model did not embed the query: <words>; the first list stays`), never the query itself, and the search still ends
//! `model_failed` with the first list on screen. On 2d7b1a72 the error was dropped (`Err(_)`), so a hidden run could not say why.
//!
//! Red-first broken copies (each one line in core/crates/engine/src/search_cmd.rs):
//! - `the_log_keeps_the_models_own_words`: the base's arm `Err(_) => { note(&parts.store, "the model did not embed the query; the first
//!   list stays"); ... }` (the words dropped): the log has no line with "could not load: config.json names no model type".
//! - `the_log_never_holds_the_query`: `model_words` without `words = words.replace(q, "<query>")`: the line holds "freelance".
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work, never a budget.
const WAIT: Duration = Duration::from_secs(120);

fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0;
    }
    v
}

/// Documents embed; the query fails with the words `query_error` makes of the query.
struct FailingQueryModel { query_error: fn(&str) -> String }
impl Embedder for FailingQueryModel {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Err(CoreError::Unexpected { reason_words: (self.query_error)(&text) }) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

struct Lists(Mutex<usize>);
impl EventSink for Lists {
    fn on_event(&self, e: CoreEvent) { if let CoreEvent::SearchResults { .. } = e { *self.0.lock().unwrap() += 1; } }
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

fn mail(account: &str, id: &str, subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.into(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// The rig's production core over `dir` with two mails, the failing model handed over, the indexer settled.
fn production(dir: &std::path::Path, model: Arc<FailingQueryModel>) -> RustCore {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![
        mail(&a, "m-budget", "Budget review", "An outside advisor will look at our budget.", 1_700_000_003_000),
        mail(&a, "m-lunch", "Lunch", "Miso glazed salmon.", 1_700_000_002_000),
    ]).unwrap();
    let bg = core.background();
    bg.set_embedder(Some(model));
    bg.reindex(&["m-budget".to_string(), "m-lunch".to_string()]);
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

/// Searches `query`, waits until the search ended `model_failed` and a quiet time passed; returns the engine's log beside the store
/// and how many lists the search sent.
fn search_until_model_failed(core: &RustCore, query: &str) -> (String, usize) {
    let lists = Arc::new(Lists(Mutex::new(0)));
    core.subscribe(StreamKind::Search, lists.clone());
    assert!(core.run(Command::Search { query: query.into() }).is_ok(), "the engine answers Search");
    wait_until("the first list", || *lists.0.lock().unwrap() >= 1);
    wait_until("the stage model_failed", || core.state("search/last/stage".into()).as_deref() == Some("\"model_failed\""));
    let log = core.store().path().parent().unwrap().join("core.log");
    let mut text = String::new();
    wait_until("the search's model line in the log", || {
        text = std::fs::read_to_string(&log).unwrap_or_default();
        text.contains("search: the model did not embed the query")
    });
    std::thread::sleep(Duration::from_millis(600));
    let sent = *lists.0.lock().unwrap();
    (text, sent)
}

fn model_line(log: &str) -> String {
    log.lines().find(|l| l.contains("search: the model did not embed the query")).unwrap_or_default().to_string()
}

#[test]
fn the_log_keeps_the_models_own_words() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), Arc::new(FailingQueryModel {
        query_error: |_| "the search model could not load: config.json names no model type".to_string(),
    }));
    let (log, sent) = search_until_model_failed(&core, "advisor");
    let line = model_line(&log);
    assert!(line.contains("search: the model did not embed the query: the search model could not load: config.json names no model type"),
        "the log keeps the model's words: {line}");
    assert_eq!(sent, 1, "the first list stays: no second list after a failed model");
    let reason = core.state("search/last/reason".into()).unwrap_or_default();
    assert!(reason.contains("Search by meaning could not run this time"), "the words above the rows are unchanged: {reason}");
}

#[test]
fn the_log_never_holds_the_query() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), Arc::new(FailingQueryModel { query_error: |text| format!("the tokenizer refused: {text}") }));
    let (log, _) = search_until_model_failed(&core, "freelance expert");
    let line = model_line(&log);
    assert!(line.contains("search: the model did not embed the query: the tokenizer refused: <query>"), "the model's words stay, the query is replaced: {line}");
    assert!(!log.contains("freelance"), "no query in the log: {line}");
}
