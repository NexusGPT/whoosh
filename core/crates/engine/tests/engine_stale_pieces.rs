//! stale-fix (item c1436f49, "words in no vector outside code pieces 0"): a mail's stored pieces never stay cut from a text it no longer
//! has. (a) A body rewritten while no search model is given loses the vector mark of its pieces, so the next model hand-over cuts it again
//! from today's text. (b) A meaning index cut before the rule marker moved (it holds the first marker, "pieces-v1", only) is cut again
//! once, and a second open cuts nothing. (c) `text_of` is pinned on fixed samples: a change of its output must move PIECE_RULE_MARKER.
//! (d) A mail queued while no model is given whose text did not change gets its vector mark back from the vector the index still holds,
//! with no second embedding.
//! The indexer runs on its own over a store in a temporary folder (no core, no account sync, no socket); the counter is the byte bound
//! (pinned); the embedder is a fake that hashes words. No mail text is printed: assertion messages carry counts, ids and the made-up
//! sample texts of this file only.
//! Broken copies (red, docs/lanes/stale-fix.md): (a) in indexer.rs `step`, the call to `unembed_body_chunks` removed; (b) the marker's
//! name put back to "pieces-v1"; (d) in indexer.rs `mark_held`, the chunks the index still holds are not handed back to be marked.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::*;
use engine::bodies::indexer::{text_of, Indexer, CHUNK_KEYS_MARKER, INDEX_BODY_MAX_CHARS, PIECE_RULE_MARKER};
use search_vector::pieces::{body_pieces, ByteBound};
use search_vector::VectorIndex;
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
/// A ceiling on the indexer's settling, never a budget: the indexer runs at the OS background priority, and on a loaded Mac it gets
/// little CPU by design (engine_sync_full_r1.rs WAIT is 120 s for the same reason; a 60 s ceiling ran out at load 90 here).
const SETTLE: Duration = Duration::from_secs(120);
/// The first rule marker, as an install before stale-fix wrote it.
const FIRST_MARKER: &str = "pieces-v1";

/// A vector per text: each word (lowercased, letters and digits) adds to one slot, so a query finds the piece holding its words.
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let mut v = vec![0.001f32; dim];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0;
    }
    v
}

/// Counts the pieces it embedded (kept in memory for the assertions, never printed).
#[derive(Default)]
struct Recorder { pieces: Mutex<Vec<String>> }
impl Recorder { fn embedded(&self) -> Vec<String> { self.pieces.lock().unwrap().clone() } }
impl Embedder for Recorder {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        let v = self.embed(texts.clone());
        self.pieces.lock().unwrap().extend(texts);
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

fn open_store(dir: &std::path::Path) -> (Arc<Store>, String) {
    let store = Arc::new(Store::open(&dir.join("whoosh.sqlite")).expect("the store opens"));
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (store, account)
}

fn start(store: &Arc<Store>, dir: &std::path::Path) -> Indexer {
    let ix = Indexer::start(store.clone(), dir.join("core.log"));
    ix.use_counter(Some(Arc::new(ByteBound)));
    ix
}

/// The indexer settles within SETTLE; the time it took and the machine's load are printed (counts and times only).
fn settled(ix: &Indexer, what: &str) {
    let t0 = std::time::Instant::now();
    let ok = ix.settle(SETTLE);
    eprintln!("engine_stale_pieces: {what}: {ok} after {:?}, load {:.1}", t0.elapsed(), load_1min());
    assert!(ok, "{what}");
}

/// The Mac's 1-minute load average (getloadavg); 0 when the OS does not say.
fn load_1min() -> f64 {
    extern "C" { fn getloadavg(loadavg: *mut f64, nelem: i32) -> i32; }
    let mut l = [0f64; 3];
    // SAFETY: the buffer holds the 3 values asked for and lives across the call.
    if unsafe { getloadavg(l.as_mut_ptr(), 3) } >= 1 { l[0] } else { 0.0 }
}

/// One mail's body chunks: (id, text, has its vector).
fn chunks(store: &Store, id: &str) -> Vec<(i64, String, bool)> { store.background_reads().unwrap().body_chunks(id).unwrap() }

/// Today's pieces of a body (the cut the indexer makes, empty pieces left out).
fn pieces_of(body: &str, subject: &str) -> Vec<String> {
    body_pieces(&ByteBound, &text_of(Some(body), None, "", subject), subject).texts.into_iter().filter(|t| !t.trim().is_empty()).collect()
}

/// (a) A body rewritten while no model is given is cut again from its new text when the model comes back: the meaning search finds the
/// new words and not the old ones, every piece has its vector, and the engine's own count for that row (`index/pieces/<after>/<limit>`)
/// reads 0 words in stale pieces. Broken copy: the call to `unembed_body_chunks` in `step` removed (the rewrite is taken off the queue with
/// no mark lost, and the backfill never queues a mail whose pieces all carry vectors).
#[test]
fn a_body_changed_with_no_model_is_cut_again_when_the_model_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = open_store(dir.path());
    let id = "m-rewritten".to_string();
    let (old_body, new_body, subject) = ("A note about fish for dinner.", "zebra quartz violin", "Dinner plans");
    store.insert_messages(vec![mail(&account, &id, subject, old_body)]).unwrap();
    let model = Arc::new(Recorder::default());
    let ix = start(&store, dir.path());
    ix.set_embedder(Some(model.clone()));
    settled(&ix, "the indexer settled with the model");
    let first = chunks(&store, &id);
    assert_eq!(first.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), pieces_of(old_body, subject), "the first cut is of the first text");
    assert!(first.iter().all(|c| c.2), "the first pieces carry their vectors");

    // the model is taken away; the body is rewritten (a repair, a download) and handed to the indexer
    ix.set_embedder(None);
    store.set_body(&id, new_body.into(), None, 0, new_body.into()).unwrap();
    ix.enqueue(std::slice::from_ref(&id));
    settled(&ix, "the indexer settled without the model");
    let while_no_model = chunks(&store, &id);

    // the model is given again
    ix.set_embedder(Some(model.clone()));
    settled(&ix, "the indexer settled with the model again");
    let now = chunks(&store, &id);
    assert_eq!(now.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), pieces_of(new_body, subject), "the pieces are cut from the new text");
    assert!(now.iter().all(|c| c.2), "every new piece carries its vector");
    let found = ix.search_meaning(new_body, 5).unwrap();
    assert_eq!(found.first().map(|h| (h.chunk.message_id.clone(), h.chunk.text.clone())), Some((id.clone(), new_body.to_string())), "the meaning search finds the new words");
    let old_words = ix.search_meaning("fish dinner", 5).unwrap();
    assert!(old_words.iter().all(|h| !h.chunk.text.contains("fish")), "no piece holds the old words: {:?}", old_words.iter().map(|h| h.chunk.id).collect::<Vec<_>>());
    let rowid = store.background_reads().unwrap().rowids_of(std::slice::from_ref(&id)).unwrap()[0];
    let counts: serde_json::Value = serde_json::from_str(&ix.piece_counts(rowid - 1, 1)).unwrap();
    assert_eq!((counts["mails"].as_u64(), counts["last"].as_i64()), (Some(1), Some(rowid)), "index/pieces reads that row: {counts}");
    assert_eq!((counts["words_in_stale_pieces"].as_u64(), counts["words_in_no_vector"].as_u64(), counts["mails_with_words_in_no_piece"].as_array().map(|a| a.len())),
               (Some(0), Some(0), Some(0)), "index/pieces: {counts}");
    // the mechanism: while no model was given, the queued mail kept its old pieces (nothing cut) and they lost their vector mark
    assert_eq!(while_no_model.iter().map(|c| c.0).collect::<Vec<_>>(), first.iter().map(|c| c.0).collect::<Vec<_>>(), "nothing is cut while no model is given");
    assert!(while_no_model.iter().all(|c| !c.2), "the pieces of a mail queued while no model is given lose their vector mark");
}

/// (b) A meaning index cut before the marker moved (its folder holds the chunk-key marker and the first piece marker only) is cut again at
/// the next open: the stored pieces cut from an older `text_of` are today's cut, each embedded once, the new marker written, the first one
/// gone with its folder; a second open (a relaunch) cuts and embeds nothing. Broken copy: PIECE_RULE_MARKER put back to "pieces-v1".
#[test]
fn an_index_cut_before_the_marker_moved_is_cut_again_once() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = open_store(dir.path());
    let id = "m-before".to_string();
    let (body, subject) = ("The quarterly figures are ready for review on Monday.", "Figures");
    store.insert_messages(vec![mail(&account, &id, subject, body)]).unwrap();
    // the install before: a piece cut from an older text_of, its vector on disk, marked, in a folder holding the first marker
    let (old_ids, _) = store.replace_body_chunks(&id, vec!["an older cut of this mail".to_string()]).unwrap();
    let folder = dir.path().join("index").join("vector");
    {
        let old = VectorIndex::open(&folder, Arc::new(Recorder::default())).unwrap();
        for c in &old_ids { old.index_vector(chunk_vector_key(*c), &vector("an older cut of this mail")).unwrap(); }
        old.commit().unwrap();
    }
    std::fs::write(folder.join(CHUNK_KEYS_MARKER), "vectors are filed under chunk ids (contracts chunk_vector_key)\n").unwrap();
    std::fs::write(folder.join(FIRST_MARKER), "pieces are cut in the model's tokens (search-vector pieces)\n").unwrap();
    store.mark_chunks_embedded(old_ids.clone(), 1).unwrap();

    let model = Arc::new(Recorder::default());
    let ix = start(&store, dir.path());
    ix.set_embedder(Some(model.clone()));
    settled(&ix, "the indexer settled at the first open");
    let expected = pieces_of(body, subject);
    let now = chunks(&store, &id);
    assert_eq!(now.iter().map(|c| c.1.clone()).collect::<Vec<_>>(), expected, "the pieces are today's cut");
    assert!(now.iter().all(|c| c.2 && !old_ids.contains(&c.0)), "new pieces, each with its vector");
    assert_eq!(model.embedded(), expected, "each piece embedded once");
    assert!(folder.join(PIECE_RULE_MARKER).exists(), "the new marker is written");
    assert!(!folder.join(FIRST_MARKER).exists(), "the first marker went with its folder");
    drop(ix);

    // a second open: nothing to cut, nothing to embed
    let again = Arc::new(Recorder::default());
    let ix = start(&store, dir.path());
    ix.set_embedder(Some(again.clone()));
    settled(&ix, "the indexer settled at the second open");
    assert_eq!(chunks(&store, &id), now, "the second open cuts nothing");
    assert_eq!(again.embedded().len(), 0, "the second open embeds nothing");
}

/// (c) `text_of` on fixed samples. Its output is what the pieces are cut from: a change here leaves every stored piece cut before it stale
/// with no store write, so it must move PIECE_RULE_MARKER (indexer.rs) in the same change, and this test's expected strings with it.
#[test]
fn text_of_is_pinned_on_fixed_samples() {
    const MOVE: &str = "text_of changed: move PIECE_RULE_MARKER";
    let plain = "Hello Ann,\nthe report is attached.\n";
    assert_eq!(text_of(Some(plain), Some("<p>ignored</p>"), "preview", "subject"), plain, "{MOVE} (plain text)");
    assert_eq!(text_of(Some("Price:&nbsp;10&nbsp;EUR &amp; more"), None, "", "s"), "Price:&nbsp;10&nbsp;EUR &amp; more", "{MOVE} (plain text with entities)");
    assert_eq!(text_of(None, Some("<p>Hello <b>Ann</b></p><p>See you&nbsp;soon &amp; well.</p>"), "preview", "subject"), "Hello Ann\nSee you soon & well.\n", "{MOVE} (html only, entities)");
    let page = "<html><body><p>Your ticket 42 is open.</p><p>Reply above this line.</p></body></html>";
    assert_eq!(text_of(Some(page), None, "", "s"), "Your ticket 42 is open.\nReply above this line.\n", "{MOVE} (a text that is page code)");
    assert_eq!(text_of(Some("  \n"), None, "the preview", "the subject"), "the preview", "{MOVE} (no body: the preview)");
    assert_eq!(text_of(None, None, " ", "the subject"), "the subject", "{MOVE} (no body, no preview: the subject)");
    let long = "word ".repeat(INDEX_BODY_MAX_CHARS / 5 + 10);
    assert_eq!(text_of(Some(&long), None, "", "s").chars().count(), INDEX_BODY_MAX_CHARS, "{MOVE} (the cap)");
}

/// (d) A mail queued while no model is given whose text did not change (a file of it landed, a repair that wrote the same text): its
/// pieces lose their mark by (a)'s rule, and when the model comes back the vectors the index still holds mark them again, with no piece
/// cut or embedded a second time. Broken copy: in indexer.rs `mark_held`, the chunks the index holds are not handed back to be marked.
#[test]
fn an_unchanged_mail_queued_with_no_model_gets_its_mark_back_without_a_second_embedding() {
    let dir = tempfile::tempdir().unwrap();
    let (store, account) = open_store(dir.path());
    let id = "m-same".to_string();
    let (body, subject) = ("The lease renewal papers are signed and filed.", "Lease");
    store.insert_messages(vec![mail(&account, &id, subject, body)]).unwrap();
    let model = Arc::new(Recorder::default());
    let ix = start(&store, dir.path());
    ix.set_embedder(Some(model.clone()));
    settled(&ix, "the indexer settled with the model");
    let first = chunks(&store, &id);
    assert!(!first.is_empty() && first.iter().all(|c| c.2), "the first pieces carry their vectors");
    let embedded_once = model.embedded().len();

    ix.set_embedder(None);
    ix.enqueue(std::slice::from_ref(&id));
    settled(&ix, "the indexer settled without the model");
    ix.set_embedder(Some(model.clone()));
    settled(&ix, "the indexer settled with the model again");
    let now = chunks(&store, &id);
    assert_eq!(now.iter().map(|c| (c.0, c.1.clone())).collect::<Vec<_>>(), first.iter().map(|c| (c.0, c.1.clone())).collect::<Vec<_>>(), "the same pieces (not cut again)");
    assert!(now.iter().all(|c| c.2), "every piece carries its vector mark again");
    assert_eq!(model.embedded().len(), embedded_once, "no piece embedded a second time");
}
