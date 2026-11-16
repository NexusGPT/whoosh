//! pieces (SPEC A151, A152), the indexer's side: a mail cut by the OLD rule (512 whitespace words, its vectors on disk under a
//! meaning index without the piece marker) is cut again ONCE by the token rule after the update, its old vectors removed; a file of a
//! mail whose text is on this Mac is cut by the same rule and embedded under "<subject> · <file name>"; no piece sent to the model is
//! over its input. The indexer runs on its own over a store in a temporary folder (no core, no account sync, no socket); the counter is
//! injected (`FiveTok`, 5 tokens a word); the embedder is a fake that records titles and piece counts only. No mail text is printed:
//! assertion messages carry counts and ids only.
use std::ops::Range;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::*;
use engine::bodies::indexer::{Indexer, CHUNK_KEYS_MARKER, PIECE_RULE_MARKER};
use search_vector::pieces::{attachment_pieces, body_pieces, clean_for_meaning, fits};
use search_vector::{chunk_text, Tokenizer, VectorIndex, WhitespaceTokenizer};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
/// A ceiling on the indexer's settling, never a budget.
const SETTLE: Duration = Duration::from_secs(30);
/// A chunk id no stored chunk has.
const ORPHAN_CHUNK: i64 = 9_999_999;

/// 5 tokens per whitespace word, 2 special tokens per input.
struct FiveTok;
impl Tokenizer for FiveTok {
    fn spans(&self, text: &str) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut at: Option<usize> = None;
        for (i, c) in text.char_indices() {
            match (c.is_whitespace(), at) {
                (true, Some(s)) => { for _ in 0..5 { out.push(s..i); } at = None; }
                (false, None) => at = Some(i),
                _ => {}
            }
        }
        if let Some(s) = at { for _ in 0..5 { out.push(s..text.len()); } }
        out
    }
    fn specials(&self) -> usize { 2 }
}

/// Records each call's title and its pieces (kept in memory for the assertions, never printed).
#[derive(Default)]
struct Recorder { calls: Mutex<Vec<(Option<String>, Vec<String>)>> }
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.split_whitespace() { v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0; }
    v
}
impl Embedder for Recorder {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        let v = self.embed(texts.clone());
        self.calls.lock().unwrap().push((title, texts));
        v
    }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: &str, subject: &str, body: &str) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: subject.into(), preview: String::new(), received_at_ms: 1_700_000_000_000, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// A long body with links and a quoted reply (unique words, letters only).
fn long_body() -> String {
    let word = |i: usize| -> String { format!("note{}", i.to_string().chars().map(|c| (b'a' + c.to_digit(10).unwrap() as u8) as char).collect::<String>()) };
    let mut b = String::new();
    for i in 0..900 {
        b.push_str(&word(i));
        b.push(if i % 40 == 39 { '\n' } else { ' ' });
        if i % 100 == 50 { b.push_str("https://www.tracker.example/c?id=8f3a9c2e7b1d4f60a9c2e7b1d4f60a9c "); }
    }
    b.push_str("\nOn Mon, 10 Nov 2026 at 09:00, Ann <ann@example.test> wrote:\n> the earlier mail\n");
    b
}

fn open_store(dir: &std::path::Path) -> (Arc<Store>, String) {
    let store = Arc::new(Store::open(&dir.join("whoosh.sqlite")).expect("the store opens"));
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (store, account)
}

/// (1) A mail cut by the old rule, with its vectors in a meaning index that has the chunk-key marker but not the piece marker, is cut
/// again by the token rule once the model is handed over: its chunks are the new pieces (no link, no quote, each fits with its title),
/// each marked embedded, the old chunks and their vectors gone, the folder carries both markers, and no piece was over the input.
/// Broken copy (red): in indexer.rs `vector_index`, `} else if dir.exists() && !rule.exists() {` -> `} else if false {` (the folder cut by the old
/// rule is kept with its old vectors). The former copy, `if !marker.exists() || !rule.exists() {` -> `if !marker.exists() {`, is equivalent: the
/// folder without the rule marker is removed just before, so `!marker.exists()` already holds (c1436f49 m9, green at ee09e51f5).
#[test]
fn a_mail_cut_by_the_old_rule_is_cut_again_once_by_the_token_rule() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = open_store(dir.path());
    let id = "m-old".to_string();
    let body = long_body();
    store.insert_messages(vec![mail(&account, &id, "Project notes", &body)]).unwrap();
    // the old rule: 512 whitespace words, 64 shared, the raw text; vectors on disk, chunks marked
    let old_texts: Vec<String> = chunk_text(&body, &WhitespaceTokenizer, 512, 64).unwrap().into_iter().map(|c| c.text).collect();
    let (old_ids, _) = store.replace_body_chunks(&id, old_texts).unwrap();
    let model = Arc::new(Recorder::default());
    let folder = dir.path().join("index").join("vector");
    {
        let old = VectorIndex::open(&folder, model.clone()).unwrap();
        for c in &old_ids { old.index_vector(chunk_vector_key(*c), &vector("old")).unwrap(); }
        // a vector of the old folder whose chunk is no longer in the store: only removing the folder takes it away (the stale-chunk pass
        // removes the vectors of the chunks it replaces, never one that has no chunk)
        old.index_vector(chunk_vector_key(ORPHAN_CHUNK), &vector("orphan")).unwrap();
        old.commit().unwrap();
    }
    std::fs::write(folder.join(CHUNK_KEYS_MARKER), "old layout with chunk keys\n").unwrap();
    store.mark_chunks_embedded(old_ids.clone(), 1).unwrap();
    assert!(!folder.join(PIECE_RULE_MARKER).exists());

    let ix = Indexer::start(store.clone(), dir.path().join("core.log"));
    ix.use_counter(Some(Arc::new(FiveTok)));
    ix.set_embedder(Some(model.clone()));
    assert!(ix.settle(SETTLE), "the indexer settled");

    let expected = body_pieces(&FiveTok, &body, "Project notes");
    let now = store.background_reads().unwrap().body_chunks(&id).unwrap();
    assert_eq!(now.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), expected.texts, "the stored chunks are the token rule's pieces");
    assert!(now.len() > 1);
    assert!(now.iter().all(|c| c.2), "every new chunk is marked embedded");
    assert!(now.iter().all(|c| !old_ids.contains(&c.0)), "the old chunks are gone");
    assert!(now.iter().all(|c| !c.1.contains("://") && !c.1.contains("earlier mail")), "links and the quote left the pieces");
    assert!(now.iter().all(|c| fits(&FiveTok, expected.title.as_deref(), &c.1)), "every piece fits with its title");
    assert!(folder.join(PIECE_RULE_MARKER).exists() && folder.join(CHUNK_KEYS_MARKER).exists(), "the rebuilt folder carries both markers");
    assert_eq!(ix.pieces_over_limit(), 0);
    let calls = model.calls.lock().unwrap().clone();
    let sent: usize = calls.iter().filter(|c| c.0.as_deref() == Some("Project notes")).map(|c| c.1.len()).sum();
    assert_eq!(sent, now.len(), "each new piece embedded once under the subject: {} calls", calls.len());
    // ownercopy-pieces (item c1436f49): the engine read the private-copy check sums (`index/pieces/<after>/<limit>`) says the same of this
    // store: one mail, its words all in a piece with a vector, every stored piece counted, none over the model's input
    let counts: serde_json::Value = serde_json::from_str(&ix.piece_counts(0, 100)).unwrap();
    assert_eq!((counts["mails"].as_u64(), counts["words_in_no_vector"].as_u64(), counts["words_in_no_piece"].as_u64(), counts["pieces"].as_u64(), counts["pieces_cut"].as_u64()),
               (Some(1), Some(0), Some(0), Some(now.len() as u64), Some(0)), "index/pieces: {counts}");
    assert!(counts["words"].as_u64().unwrap_or(0) > 0 && counts["last"].is_i64(), "index/pieces: {counts}");
    assert_eq!((counts["words_lost_by_the_cut"].as_u64(), counts["mails_with_words_in_no_piece"].as_array().map(|a| a.len())), (Some(0), Some(0)), "index/pieces causes: {counts}");
    drop(ix);
    let reread = VectorIndex::open(&folder, model.clone()).unwrap();
    assert!(now.iter().all(|c| reread.contains_message(chunk_vector_key(c.0))), "the new vectors are on disk");
    assert!(old_ids.iter().all(|c| !reread.contains_message(chunk_vector_key(*c))), "the old vectors are gone");
    assert!(!reread.contains_message(chunk_vector_key(ORPHAN_CHUNK)), "the folder cut by the old rule was removed whole, an orphan vector with it");
    // a word of the cleaned text that is in no stored piece: none
    let clean = clean_for_meaning(&body);
    let held: std::collections::HashSet<&str> = now.iter().flat_map(|c| c.1.split_whitespace()).collect();
    assert_eq!(clean.split_whitespace().filter(|w| !held.contains(w)).count(), 0, "words of the mail in no vector");
}

/// (1b) `index/pieces` counts a word whose piece has no vector: the same mail with its stored pieces (today's cut) but only the first one
/// embedded says `words_in_no_vector` > 0 while `words_in_no_piece` stays 0. (1) reads a mail whose every piece has its vector, so the figure
/// was 0 either way (c1436f49 m10: `no_vector += outside_v.len()` -> `no_vector += 0` stayed green).
/// Broken copy: in indexer.rs `piece_counts`, `no_vector += outside_v.len() as u64;` -> `no_vector += 0;`.
#[test]
fn index_pieces_counts_the_words_whose_piece_has_no_vector() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = open_store(dir.path());
    let id = "m-half".to_string();
    let body = long_body();
    store.insert_messages(vec![mail(&account, &id, "Project notes", &body)]).unwrap();
    let cut = body_pieces(&FiveTok, &body, "Project notes");
    assert!(cut.texts.len() > 2, "a long mail is several pieces: {}", cut.texts.len());
    let (ids, _) = store.replace_body_chunks(&id, cut.texts.clone()).unwrap();
    store.mark_chunks_embedded(vec![ids[0]], 1).unwrap();
    // no meaning model is handed over: the indexer leaves the chunks as they are, so the counts read exactly this state
    let ix = Indexer::start(store.clone(), dir.path().join("core.log"));
    ix.use_counter(Some(Arc::new(FiveTok)));
    assert!(ix.settle(SETTLE), "the indexer settled");
    let counts: serde_json::Value = serde_json::from_str(&ix.piece_counts(0, 100)).unwrap();
    assert_eq!((counts["mails"].as_u64(), counts["words_in_no_piece"].as_u64(), counts["pieces"].as_u64()), (Some(1), Some(0), Some(cut.texts.len() as u64)), "index/pieces: {counts}");
    assert!(counts["words_in_no_vector"].as_u64().unwrap_or(0) > 0, "the words of the pieces with no vector are counted: {counts}");
}

/// (2) A file of a mail whose text is on this Mac is cut by the same rule and embedded under "<subject> · <file name>"; its chunks
/// are marked embedded. The body keeps the subject alone as its title.
/// Broken copy (red): in indexer.rs `step`, delete `self.file_pieces(&b.message_id, &b.subject, tok.as_ref(), embedder.as_ref(), &vector);`.
#[test]
fn a_files_pieces_carry_the_subject_and_the_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = open_store(dir.path());
    let id = "m-file".to_string();
    store.insert_messages(vec![mail(&account, &id, "Quote for May", "Please find the quote attached.")]).unwrap();
    let att = format!("{id}:att:001");
    store.put_message_attachments(&id, vec![AttachmentFile {
        id: att.clone(), message_id: id.clone(), file_name: "devis.pdf".into(), mime_type: "application/pdf".into(), size_bytes: 1_000, local_path: None, inline: false, content_id: None,
    }]).unwrap();
    let text = "Quarterly brochure quote\nHeavy matte stock, 170 g, two colours\nUnit price 0.84 EUR for 12,000 copies";
    store.put_attachment_content(AttachmentContent {
        attachment_id: att.clone(), text: Some(text.into()), ocr_text: None, labels: vec![], transcript: None, embedded: false, vector_key: None, updated_at_ms: 1,
    }).unwrap();

    let model = Arc::new(Recorder::default());
    let ix = Indexer::start(store.clone(), dir.path().join("core.log"));
    ix.use_counter(Some(Arc::new(FiveTok)));
    ix.set_embedder(Some(model.clone()));
    assert!(ix.settle(SETTLE), "the indexer settled");

    let expected = attachment_pieces(&FiveTok, text, "Quote for May", "devis.pdf");
    assert_eq!(expected.title.as_deref(), Some("Quote for May · devis.pdf"));
    let calls = model.calls.lock().unwrap().clone();
    let file_calls: Vec<_> = calls.iter().filter(|c| c.0.as_deref() == Some("Quote for May · devis.pdf")).collect();
    assert_eq!(file_calls.len(), 1, "the file is embedded once under its title ({} calls)", calls.len());
    assert_eq!(file_calls[0].1, expected.texts);
    assert!(calls.iter().any(|c| c.0.as_deref() == Some("Quote for May")), "the body keeps the subject as its title");
    let chunks = store.attachment_chunks(&att).unwrap();
    assert_eq!(chunks.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), expected.texts);
    assert!(chunks.iter().all(|c| c.2), "the file's chunks are marked embedded");
    assert_eq!(ix.pieces_over_limit(), 0);
}
