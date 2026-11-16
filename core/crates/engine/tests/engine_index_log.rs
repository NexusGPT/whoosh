//! Item b43515ba: the meaning index says in core.log why it is not growing and what each pass did, counts only (never mail text).
//! (1) a hold by battery or heat writes one line when it starts and one when it ends, each with the count waiting, and a repeated
//! hold of the same value writes nothing more; (2) a finished embedding pass writes "N chunks embedded in this pass; M mails and rows
//! wait". The indexer runs on its own over a store in a temporary folder (no core, no sync, no socket); the embedder is a fake.
//! The memory wait's lines (`wait_for_room`) need the process's footprint over EMBED_INDEX_CEILING_MB and are not staged here.
use std::sync::Arc;
use std::time::Duration;

use contracts::*;
use engine::bodies::indexer::Indexer;
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
/// A ceiling on the indexer's settling, never a budget.
const SETTLE: Duration = Duration::from_secs(30);

struct Fake;
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.split_whitespace() { v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0; }
    v
}
impl Embedder for Fake {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: &str) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: "Pass note".into(), preview: String::new(), received_at_ms: 1_700_000_000_000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some("alpha beta gamma".into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

fn log_lines(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path).unwrap_or_default().lines().map(str::to_string).collect()
}

/// (1) Broken copy (red): in indexer.rs `set_held`, the two `self.inner.note(...)` lines removed.
#[test]
fn a_hold_by_battery_or_heat_is_said_once_when_it_starts_and_once_when_it_ends() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("whoosh.sqlite")).expect("the store opens"));
    let log = dir.path().join("core.log");
    let ix = Indexer::start(store.clone(), log.clone());
    ix.set_held(true);
    ix.set_held(true);                                   // the same hold again: no second line
    ix.set_held(false);
    let lines = log_lines(&log);
    let held = lines.iter().filter(|l| l.contains("indexer: held by battery or heat; ") && l.ends_with(" mails and rows wait")).count();
    let freed = lines.iter().filter(|l| l.contains("indexer: no longer held by battery or heat; ") && l.ends_with(" mails and rows wait")).count();
    assert_eq!((held, freed), (1, 1), "one line when the hold starts, one when it ends; {} lines in the log", lines.len());
}

/// (2) Broken copy (red): in indexer.rs `commit_and_mark`, the `self.note(&format!("{} chunks embedded in this pass; ...` line removed.
#[test]
fn a_finished_embedding_pass_says_how_many_chunks_it_embedded() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("whoosh.sqlite")).expect("the store opens"));
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![mail(&account, "m-pass")]).unwrap();
    let (ids, _) = store.replace_body_chunks("m-pass", vec!["alpha beta gamma".to_string()]).unwrap();
    let log = dir.path().join("core.log");
    let ix = Indexer::start(store.clone(), log.clone());
    ix.set_embedder(Some(Arc::new(Fake)));
    assert!(ix.settle(SETTLE), "the indexer settled");
    drop(ix);                                            // the last pass commits and marks on the way out at the latest
    let lines = log_lines(&log);
    let passes: Vec<&String> = lines.iter().filter(|l| l.contains(" chunks embedded in this pass; ") && l.ends_with(" mails and rows wait")).collect();
    assert!(!passes.is_empty(), "a pass line is written for {} chunk(s); {} lines in the log", ids.len(), lines.len());
    assert!(lines.iter().all(|l| !l.contains("alpha")), "no mail text in the log");
}
