//! lane rank (SPEC A150, A152): on the real engine, the meaning part of a search scores every piece of a mail (the body's and each
//! attachment's), and marks RELATED (`MatchedIn::Related`) only the mails that are not word matches and whose closeness stands
//! clearly above the average closeness of that one search: lift = (closeness - mean) / max(deviation, 0.02) at or above 2.0, over
//! the mails in scope with a vector (the matches included), none under 20 such mails, at most 50 (the closest). The list: the
//! matches (A149's order), the related mails closest first, the other mails with a vector closest first, the mails with no vector
//! newest first.
//!
//! Each test runs over the rig's production core (`open_production(_, rig = true)`, accounts on 127.0.0.1 only) on a fresh data
//! folder with a fake embedder whose vectors the test sets: a text holding the token `cNN` is the vector [NN/100, sqrt(1 - (NN/100)^2),
//! 0, ...], and every query is [1, 0, ...], so the mail's closeness to any query is NN/100 (as the meaning index stores it, in
//! 8 bits: 0.60 reads about 0.60, see each test). An attachment's pieces are filed in the meaning index by the test itself, before
//! the core opens that index, with the vector the same embedder gives their text (the indexer of this commit embeds body pieces
//! only; a later indexer that embeds attachment pieces gives the same vector). Each test names its broken copy: the one-line change
//! that turns it red. No test prints a mail's text or a query.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::{AttachmentIn, NewMessage};

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work, never a budget (the indexer runs at the OS background priority).
const WAIT: Duration = Duration::from_secs(300);
/// The typed word: no mail holds it but the ones a test makes a match.
const QUERY: &str = "zebra";
const T: i64 = 1_700_000_000_000;
/// The engine's rule (search_cmd.rs, lane rank), restated for the checks.
const GATE: f64 = 2.0;
const FLOOR: f64 = 0.02;

/// The vector of a text: its first `cNN` token sets the closeness to every query (0.30 with none).
fn vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM;
    let c = text.split(|ch: char| !ch.is_alphanumeric())
        .find_map(|w| w.strip_prefix('c').filter(|d| d.len() == 2 && d.chars().all(|x| x.is_ascii_digit())).and_then(|d| d.parse::<u32>().ok()))
        .map_or(0.30f32, |n| n as f32 / 100.0);
    let mut v = vec![0f32; dim];
    v[0] = c;
    v[1] = (1.0 - c * c).sqrt();
    v
}

struct FakeModel;
impl Embedder for FakeModel {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| vector(t)).collect()) }
    fn embed_query(&self, _text: String) -> Result<Vec<f32>, CoreError> {
        let mut v = vec![0f32; search_vector::constants::EMBED_DIM];
        v[0] = 1.0;
        Ok(v)
    }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(vector(&path)) }
}

/// Every `SearchResults` the core sent, in order.
struct Lists(Mutex<Vec<(String, Vec<SearchHit>)>>);
impl EventSink for Lists {
    fn on_event(&self, e: CoreEvent) { if let CoreEvent::SearchResults { query, hits } = e { self.0.lock().unwrap().push((query, hits)); } }
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

/// One mail to store. `body` None: no body, so no vector.
struct Mail { id: String, thread: String, subject: String, body: Option<String>, at: i64, file: Option<(String, String)> }

fn m(id: &str, subject: &str, body: Option<&str>, at: i64) -> Mail {
    Mail { id: id.into(), thread: format!("{id}:thread"), subject: subject.into(), body: body.map(String::from), at, file: None }
}

/// `n` mails of the flat field, closeness 0.60, older than every other mail of a test.
fn field(n: usize) -> Vec<Mail> {
    (0..n).map(|i| m(&format!("field-{i:03}"), "Weekly notes", Some("c60 the weekly notes"), T - 1_000_000 + i as i64 * 1_000)).collect()
}

fn row(account: &str, x: &Mail) -> NewMessage {
    NewMessage {
        message: Message {
            id: x.id.clone(), thread_id: x.thread.clone(), account_id: account.into(), sender_name: "Sender".into(), from_address: "sender@example.test".into(),
            to: vec![USER.into()], subject: x.subject.clone(), preview: String::new(), received_at_ms: x.at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: x.file.iter().map(|(n, _)| n.clone()).collect(), login_code: None, trackers_blocked: 0,
            list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: x.body.clone(), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// The rig's production core over `dir` with `mails` stored; each attachment's one piece filed in the meaning index before the
/// core opens it; then the model handed over, every body handed to the indexer, and the indexer settled.
fn production(dir: &std::path::Path, mails: &[Mail]) -> RustCore {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(mails.iter().map(|x| row(&a, x)).collect()).unwrap();
    let model: Arc<FakeModel> = Arc::new(FakeModel);
    let files: Vec<&Mail> = mails.iter().filter(|x| x.file.is_some()).collect();
    if !files.is_empty() {
        let rows = files.iter().map(|x| {
            let (name, _) = x.file.clone().unwrap();
            (x.id.clone(), vec![AttachmentIn { part: "2".into(), name, mime_type: "application/pdf".into(), size_bytes: 1_000, content_id: None, inline: false, encoding: "base64".into() }])
        }).collect();
        store.put_attachment_rows(rows, true).unwrap();
        let dir = store.path().with_file_name("index").join("vector");
        let index = search_vector::VectorIndex::open(&dir, model.clone()).expect("the meaning index opens");
        let mut filed = Vec::new();
        for x in &files {
            let (_, text) = x.file.clone().unwrap();
            let (ids, _) = store.replace_attachment_chunks(&store::attachment_row_id(&x.id, 0), vec![text.clone()]).unwrap();
            assert_eq!(ids.len(), 1, "one piece filed for the attachment of {}", x.id);
            index.index_vector(chunk_vector_key(ids[0]), &vector(&text)).expect("the piece's vector is filed");
            filed.extend(ids);
        }
        index.commit().expect("the meaning index is written");
        drop(index);
        // the core keeps an index that carries these two markers (indexer.rs `vector_index`); without the piece-rule one (SPEC A151) it
        // removes the folder and the test's attachment pieces, which have no stored file text to be cut again, lose their vectors
        std::fs::write(dir.join(engine::bodies::indexer::CHUNK_KEYS_MARKER), "vectors are filed under chunk ids (contracts chunk_vector_key)\n").unwrap();
        std::fs::write(dir.join(engine::bodies::indexer::PIECE_RULE_MARKER), "pieces are cut in the model's tokens (search-vector pieces)\n").unwrap();
        store.mark_chunks_embedded(filed, T).unwrap();
    }
    let bg = core.background();
    bg.set_embedder(Some(model));
    bg.reindex(&mails.iter().filter(|x| x.body.is_some()).map(|x| x.id.clone()).collect::<Vec<_>>());
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

/// Runs the search and answers its second list (the meaning order).
fn ranked(core: &RustCore) -> Vec<SearchHit> {
    let lists = Arc::new(Lists(Mutex::new(vec![])));
    core.subscribe(StreamKind::Search, lists.clone());
    assert!(core.run(Command::Search { query: QUERY.into() }).is_ok(), "the engine answers Search");
    wait_until("the ranked list", || lists.0.lock().unwrap().len() >= 2);
    assert_eq!(core.state("search/last/stage".into()).as_deref(), Some("\"ranked\""), "the meaning order is in");
    // a local first: the lock's guard must end before `lists` does (E0597 in the lead's compile of 2026-11-11)
    let second = lists.0.lock().unwrap()[1].1.clone();
    second
}

fn ids(hits: &[SearchHit]) -> Vec<String> { hits.iter().map(|h| h.message_id.clone()).collect() }
fn related(hits: &[SearchHit]) -> Vec<String> { ids(&hits.iter().filter(|h| h.matched_in == MatchedIn::Related).cloned().collect::<Vec<_>>()) }
fn hit<'a>(hits: &'a [SearchHit], id: &str) -> &'a SearchHit { hits.iter().find(|h| h.message_id == id).unwrap_or_else(|| panic!("{id} is listed")) }
fn at(hits: &[SearchHit], id: &str) -> usize { hits.iter().position(|h| h.message_id == id).unwrap_or_else(|| panic!("{id} is listed")) }
/// `search/last/hit/<n>/lift` of the hit holding `id`: Some(number) or None (null).
fn lift(core: &RustCore, hits: &[SearchHit], id: &str) -> Option<f64> {
    let text = core.state(format!("search/last/hit/{}/lift", at(hits, id))).expect("the lift is served");
    if text == "null" { None } else { Some(text.parse::<f64>().unwrap_or_else(|_| panic!("the lift of {id} is a number: {text}"))) }
}
fn related_count(core: &RustCore) -> String { core.state("search/last/related".into()).expect("the related count is served") }

/// One mail far above a flat field is related: 30 mails at 0.60, one at 0.75 (lift about 5.5); it is the only related mail, and
/// it comes right after the (absent) matches.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `related_of`, `.filter(|(c, lift)| **lift >= RELATED_LIFT_MIN && ...`
/// -> `.filter(|(c, lift)| **lift >= RELATED_LIFT_MIN * 4.0 && ...` (the gate at 8.0: no mail is related).
#[test]
fn a_one_mail_far_above_a_flat_field_is_related() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(30);
    mails.push(m("far", "Budget", Some("c75 the budget plan"), T));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    assert_eq!(related(&hits), vec!["far".to_string()], "the one mail far above the field is related, and only it");
    assert_eq!(at(&hits, "far"), 0, "it leads the list (no match)");
    assert!(hits.iter().filter(|h| h.message_id != "far").all(|h| h.matched_in == MatchedIn::MeaningOnly), "the field stays close in meaning");
}

/// A flat field gives no related mail: 30 mails at 0.60 and one at 0.62. The deviation is under the floor, so the 0.62 mail's
/// lift is about 1 (0.02 / 0.02), under the gate.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `lifts_of`, `let deviation = variance.sqrt().max(RELATED_DEVIATION_FLOOR);`
/// -> `let deviation = variance.sqrt();` (no floor: the 0.62 mail lifts about 5.5 and is marked).
#[test]
fn b_a_flat_field_gives_no_related_mail() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(30);
    mails.push(m("bump", "Budget", Some("c62 the budget plan"), T));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    assert!(related(&hits).is_empty(), "no related mail in a flat field: {:?}", related(&hits));
    assert_eq!(related_count(&core), "0");
    let l = lift(&core, &hits, "bump").expect("the mail has a vector");
    assert!(l < GATE, "the 0.62 mail's lift is under the gate: {l}");
}

/// Under RELATED_MIN_POPULATION (20) mails with a vector, none is related: 10 mails at 0.60 and one at 0.95, whose lift (about 3.2)
/// is over the gate, so only the population rule keeps it out.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `related_of`, `if meaning.len() < RELATED_MIN_POPULATION { return HashSet::new(); }`
/// -> `if meaning.len() < 1 { return HashSet::new(); }`.
#[test]
fn c_under_the_least_population_no_mail_is_related() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(10);
    mails.push(m("far", "Budget", Some("c95 the budget plan"), T));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    assert_eq!(hits.iter().filter(|h| h.score > -1.5).count(), 11, "11 mails with a vector, under 20");
    let l = lift(&core, &hits, "far").expect("the mail has a vector");
    assert!(l >= GATE, "its lift is over the gate ({l}): the population rule alone keeps it out");
    assert!(related(&hits).is_empty(), "no related mail under 20 mails with a vector: {:?}", related(&hits));
    assert_eq!(related_count(&core), "0");
}

/// RELATED_MAX (50) keeps the closest: 600 mails at 0.60, 50 at 0.95 (lift about 3.2) and 10 at 0.90 (lift about 2.7): all 60 pass
/// the gate; the 50 closest are related, the 10 others follow as close in meaning.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `related_of`, `.take(RELATED_MAX)` -> `.take(RELATED_MAX + 10)`.
#[test]
fn d_the_most_related_mails_are_the_closest() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(600);
    mails.extend((0..50).map(|i| m(&format!("top-{i:02}"), "Budget", Some("c95 the budget plan"), T + i as i64 * 1_000)));
    mails.extend((0..10).map(|i| m(&format!("next-{i:02}"), "Budget", Some("c90 the budget plan"), T + 100_000 + i as i64 * 1_000)));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    let got: HashSet<String> = related(&hits).into_iter().collect();
    let want: HashSet<String> = (0..50).map(|i| format!("top-{i:02}")).collect();
    assert_eq!(got, want, "the 50 closest are related, no more");
    assert_eq!(related_count(&core), "50");
    for i in 0..10 {
        let id = format!("next-{i:02}");
        assert_eq!(hit(&hits, &id).matched_in, MatchedIn::MeaningOnly, "{id} passes the gate and is not among the 50 closest");
        let l = lift(&core, &hits, &id).expect("the mail has a vector");
        assert!(l >= GATE, "{id}'s lift is over the gate ({l}): RELATED_MAX alone keeps it out");
        assert!(at(&hits, &id) >= 50, "{id} follows the related mails");
    }
}

/// A mail that stands out of the search and is under the closeness floor is not related: 30 mails at 0.60, "under" at 0.68 and "over" at
/// 0.72. Both lift over the gate (about 2.8 and 4.3); RELATED_MIN_COSINE (0.70) keeps "under" out. This is the search typed with letters
/// no mail is about: its closest mails stand out of a field that is all far from the query.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `related_of`, `&& f64::from(c.cos) >= RELATED_MIN_COSINE` removed
/// ("under" is marked: the count reads 2).
#[test]
fn d2_a_mail_that_stands_out_under_the_closeness_floor_is_not_related() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(30);
    mails.push(m("under", "Budget", Some("c68 the budget plan"), T));
    mails.push(m("over", "Budget", Some("c72 the budget plan"), T + 1_000));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    let l = lift(&core, &hits, "under").expect("the mail has a vector");
    assert!(l >= GATE, "the 0.68 mail's lift is over the gate ({l}): the closeness floor alone keeps it out");
    assert_eq!(related(&hits), vec!["over".to_string()], "only the mail at or above the floor is related");
    assert_eq!(related_count(&core), "1");
    assert_eq!(hit(&hits, "under").matched_in, MatchedIn::MeaningOnly, "the mail under the floor stays close in meaning");
}

/// A word match is never marked related, though its closeness is the highest; its lift is served; a mail with no word that stands
/// as high is related. 30 mails at 0.60; "match" holds the word at 0.95; "close" at 0.80.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `related_of`, `&& !matches.contains(c.id.as_str())` -> `&& true`
/// (the match is counted related: the count reads 2).
#[test]
fn e_a_match_is_never_marked_related() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(30);
    mails.push(m("match", "Budget", Some("c95 a zebra on the plan"), T));
    mails.push(m("close", "Budget", Some("c80 the budget plan"), T + 1_000));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    assert!(matches!(hit(&hits, "match").matched_in, MatchedIn::TextWords { .. }), "the word match shows its text: {:?}", hit(&hits, "match").matched_in);
    assert_eq!(at(&hits, "match"), 0, "the match leads");
    assert_eq!(related(&hits), vec!["close".to_string()], "only the mail with no word is related");
    assert_eq!(related_count(&core), "1");
    let l = lift(&core, &hits, "match").expect("the lift is served for a match");
    assert!(l >= GATE, "the match stands above the gate ({l}) and is still a match");
}

/// A word match never takes one of the RELATED_MAX places: 600 mails at 0.60, a word match at 0.95 (the closest of all), and 50 mails at
/// 0.90 with no word: all 50 are related. With the match left in the pool the 50 places go to the match and 49 of them, and one mail
/// that deserves the mark is left out (the old test, `e`, saw only the final list, which drops the match from the related rows anyway).
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `related_of`, `&& !matches.contains(c.id.as_str())` -> `&& true`.
#[test]
fn e2_a_word_match_does_not_use_up_a_place_among_the_related_mails() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(600);
    mails.push(m("match", "Budget", Some("c95 a zebra on the plan"), T));
    mails.extend((0..50).map(|i| m(&format!("top-{i:02}"), "Budget", Some("c90 the budget plan"), T + 1_000 + i as i64 * 1_000)));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    assert!(matches!(hit(&hits, "match").matched_in, MatchedIn::TextWords { .. }), "the word match stays a match: {:?}", hit(&hits, "match").matched_in);
    let got: HashSet<String> = related(&hits).into_iter().collect();
    let want: HashSet<String> = (0..50).map(|i| format!("top-{i:02}")).collect();
    assert_eq!(got, want, "all 50 mails without the word are related: the match took none of their places");
    assert_eq!(related_count(&core), "50");
}

/// The order of one answer: the matches in A149's order (the subject before the text), the related mails closest first, the other
/// mails with a vector closest first (the field's equal ones newest first), then the mails with no vector newest first.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `assemble`, `for want_related in [true, false] {` -> `for want_related in [false, true] {`.
#[test]
fn f_the_order_is_matches_related_others_then_no_vector() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(30);
    mails.push(m("m-text", "Budget", Some("c50 a zebra on the plan"), T + 9_000));
    mails.push(m("m-subject", "Zebra crossing", Some("c55 the crossing plan"), T + 1_000));
    mails.push(m("r-80", "Budget", Some("c80 the budget plan"), T + 8_000));
    mails.push(m("r-85", "Budget", Some("c85 the budget plan"), T + 2_000));
    mails.push(m("o-50", "Budget", Some("c50 the budget plan"), T + 7_000));
    mails.push(m("o-40", "Budget", Some("c40 the budget plan"), T + 6_000));
    mails.push(m("v-old", "Parking", None, T + 3_000));
    mails.push(m("v-new", "Parking", None, T + 4_000));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    let mut want: Vec<String> = ["m-subject", "m-text", "r-85", "r-80"].iter().map(|s| s.to_string()).collect();
    want.extend((0..30).rev().map(|i| format!("field-{i:03}")));
    want.extend(["o-50", "o-40", "v-new", "v-old"].iter().map(|s| s.to_string()));
    assert_eq!(ids(&hits), want, "matches, related, the others by closeness, then no vector by date");
    assert_eq!(related(&hits), vec!["r-85".to_string(), "r-80".to_string()], "the related mails, closest first");
}

/// A mail whose best piece is an attachment's: the hit is the mail, carries the file's name, and shows that piece's passage. 30
/// mails at 0.60; "plan" has a body at 0.55 and a file whose piece is at 0.90, so it is related by its attachment.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `closeness`, `"SELECT id, message_id, part FROM chunk"` ->
/// `"SELECT id, message_id, part FROM chunk WHERE part = 'body'"` (attachment pieces never scored: "plan" sits at 0.55, not related).
#[test]
fn g_a_mail_whose_best_piece_is_an_attachment_carries_the_file_and_its_passage() {
    let dir = tempfile::tempdir().unwrap();
    let passage = "c90 the quarterly plan for the new office";
    let mut mails = field(30);
    let mut plan = m("plan", "Budget", Some("c55 see the file"), T);
    plan.file = Some(("office-plan.pdf".into(), passage.into()));
    mails.push(plan);
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    let h = hit(&hits, "plan");
    assert_eq!(h.matched_in, MatchedIn::Related, "related by its attachment's piece");
    assert_eq!(h.attachment_name.as_deref(), Some("office-plan.pdf"), "the hit carries the file's name");
    let want: String = passage.chars().take(contracts::constants::SEARCH_SNIPPET_CHARS as usize).collect();
    assert!(h.snippet == want, "the snippet is the attachment piece's passage, not the body's");
    assert!(h.score > 0.85, "the closeness is the attachment piece's: {}", h.score);
    assert_eq!(core.state(format!("search/last/hit/{}/attachment_name", at(&hits, "plan"))).as_deref(), Some("\"office-plan.pdf\""));
}

/// A conversation is ranked by the best piece of any of its mails: the screen folds a thread into its first hit, so that hit must
/// be the thread's best mail. Thread "t-plan": "older" (body 0.62, a file at 0.93) and "newer" (body 0.70); "other", a thread of
/// its own, at 0.80. The list holds older (0.93), other (0.80), newer (0.70): the thread's first hit is older, before other.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `closeness`, `"SELECT id, message_id, part FROM chunk"` ->
/// `"SELECT id, message_id, part FROM chunk WHERE part = 'body'"` (the thread's first hit would be newer, after other).
#[test]
fn h_the_best_mail_of_a_thread_comes_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut older = m("older", "Office plan", Some("c62 the office plan"), T);
    older.thread = "t-plan".into();
    older.file = Some(("floor-plan.pdf".into(), "c93 the floor plan of the new office".into()));
    let mut newer = m("newer", "Re: Office plan", Some("c70 thanks for the plan"), T + 60_000);
    newer.thread = "t-plan".into();
    let other = m("other", "Budget", Some("c80 the budget plan"), T + 30_000);
    let core = production(dir.path(), &[older, newer, other]);
    let hits = ranked(&core);
    assert_eq!(ids(&hits), vec!["older".to_string(), "other".to_string(), "newer".to_string()], "by the best piece of each mail");
    let thread_of = |id: &str| core.message(id.to_string()).map(|x| x.thread_id).unwrap_or_default();
    let first = hits.iter().find(|h| thread_of(&h.message_id) == "t-plan").expect("the thread is listed");
    assert_eq!(first.message_id, "older", "the thread's first hit is its best mail");
    assert_eq!(first.attachment_name.as_deref(), Some("floor-plan.pdf"), "opened at the file that matched");
    assert!(at(&hits, "older") < at(&hits, "other"), "the thread ranks by its best piece, above the other thread");
}

/// The two state values: `search/last/related` is the number of related hits; `search/last/hit/<n>/lift` is each hit's lift with
/// two decimals, (closeness - mean) / max(deviation, 0.02) over the mails with a vector, computed here again from the hits' scores
/// (no match, so every score of a mail with a vector is its closeness), and null for the mail with no vector.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `lifts_of`, `meaning.iter().map(|c| (f64::from(c.cos) - mean) / deviation).collect()`
/// -> `meaning.iter().map(|c| (f64::from(c.cos) - mean) / deviation.powi(2)).collect()`.
#[test]
fn i_the_related_count_and_each_lift_are_served() {
    let dir = tempfile::tempdir().unwrap();
    let mut mails = field(30);
    mails.push(m("far", "Budget", Some("c75 the budget plan"), T));
    mails.push(m("bare", "Parking", None, T + 1_000));
    let core = production(dir.path(), &mails);
    let hits = ranked(&core);
    assert_eq!(related_count(&core), related(&hits).len().to_string(), "the count is the related hits'");
    assert_eq!(related_count(&core), "1");
    let with_vector: Vec<f64> = hits.iter().filter(|h| h.message_id != "bare").map(|h| h.score).collect();
    assert_eq!(with_vector.len(), 31, "31 mails with a vector");
    let n = with_vector.len() as f64;
    let mean = with_vector.iter().sum::<f64>() / n;
    let deviation = (with_vector.iter().map(|c| (c - mean).powi(2)).sum::<f64>() / n).sqrt().max(FLOOR);
    for (i, h) in hits.iter().enumerate() {
        let text = core.state(format!("search/last/hit/{i}/lift")).expect("the lift is served");
        if h.message_id == "bare" {
            assert_eq!(text, "null", "no vector, no lift");
            continue;
        }
        let decimals = text.split_once('.').map(|(_, d)| d.len());
        assert_eq!(decimals, Some(2), "two decimals: {text}");
        let got: f64 = text.parse().unwrap();
        let want = (h.score - mean) / deviation;
        assert!((got - want).abs() <= 0.006, "hit {i} ({}): lift {got}, the rule gives {want:.4}", h.message_id);
    }
    assert!(lift(&core, &hits, "far").unwrap() >= GATE, "the related mail's lift is over the gate");
    assert_eq!(core.state(format!("search/last/hit/{}/matched_in", at(&hits, "far"))).as_deref(), Some("\"related\""));
}
