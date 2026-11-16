//! search-forms (SPEC A141): the owner's search on the real engine. `Command::Search { query }` returns at once; the ranking runs on
//! a thread of its own and the hits arrive on the Search stream as `CoreEvent::SearchResults { query, hits }`, the record the test
//! engine sends (FixtureSearch.swift), for the query string exactly as the app sent it.
//!
//! The list is EVERY stored mail in scope, each once, in this order:
//! (a) the word matches (match-inside, SPEC A149): the mails that hold every typed word, a typed word of
//!     `search_text::limits::SUBSTRING_MIN_CHARS` letters or more matching those letters also inside a longer word ("consultant"
//!     finds "consultants" and brightconsultants.example) in the subject, the sender's name and address and the text; first the typed
//!     word itself, then a word that starts with it, then a word that holds it, in each the subject or sender before the text
//!     (the score says that rank), then newest first, then id; `matched_in` saying where with the letters to show (`SubjectWords`,
//!     `SenderWords`, an attachment's name or text, `TextWords` with ranges in the snippet, the passage around the first
//!     occurrence). No word match is ever `MeaningOnly` or `Related`: the app starts its "Close in meaning" group at the first
//!     `MeaningOnly` hit;
//! (r) lane rank (SPEC A150, A152): the RELATED mails, `MatchedIn::Related`, closest first: a mail that is not a word match whose
//!     closeness stands clearly above this search's average. A mail's closeness is the best cosine of ANY of its pieces (the body's
//!     and every attachment's, `chunk.part` 'body' or 'attachment:<id>') to the query's vector (`Embedder::embed_query`, the QUERY
//!     form). Over the N mails in scope that have a vector (the word matches included: they are part of the distribution), with
//!     mean m and deviation s (the population's: the root of the mean squared distance to m), each mail's
//!         lift = (closeness - m) / max(s, RELATED_DEVIATION_FLOOR);
//!     a mail that is not a word match is related when lift >= RELATED_LIFT_MIN AND its closeness >= RELATED_MIN_COSINE, at most
//!     RELATED_MAX of them (the closest), and none when N < RELATED_MIN_POPULATION. The lift is the per-search cut-off; the closeness
//!     floor stands above what this model gives unrelated text (0.56 to 0.69), so a search typed with letters no mail is about marks
//!     no mail (the lift alone marked 6 to 19 of the owner's 310 mails for each of 12 made-up queries, 14 Nov 2026);
//! (b) every other mail with a vector, by its closeness, highest first, down to the least close, `MeaningOnly`; no score cut-off;
//! (c) the mails with no vector yet, by date, newest first, `MeaningOnly` with the score NO_VECTOR_SCORE, under every cosine.
//! A meaning row ((r) and (b)) shows the passage of its best piece; when that piece is an attachment's, `attachment_name` is the
//! file's name; `message_id` is always the mail that holds the piece (the screen folds a conversation into its first hit, so a
//! conversation is ranked by the best piece of any of its mails).
//! So the exact group is every hit before the first `Related` or `MeaningOnly` one; `exact_matches` gives it alone, for whatever
//! reads search hits as MEMBERS (a smart folder's rule match and its count, a saved search's count), which never read the ranked tail.
//! At most SEARCH_RANKED_MAX rows are sent.
//!
//! Two sends for one query: the first as soon as the word hits exist ((a), then every other mail by date, newest first), the second
//! when the query's vector is ready ((a), (r), (b), (c)); the second replaces the first. The related mark is made in the second
//! send only, from the cosines that send reads anyway (no other pass over the store). With no model, or a model that fails, the first
//! send is the answer. A newer search makes an older one stop before its next send.
//! model-late: when the shell said it is looking for the model (`crawl::expect_background_embedder`) and has not handed it over yet,
//! the first send says "words" (not "no_model"), and the last search ranks again, once, when the model is handed over
//! (`Indexer::set_embedder(Some)`), if no newer search started; if the shell then says the model is not on this Mac, its stage
//! becomes "no_model". A core never told so treats "no model handed" as "no model on this Mac", as before.
//!
//! Scope: every account, archived mail included; the mail in a Junk or Trash folder (the role the server named at its last LIST,
//! else the usual names, `reads::folders_with_role`) and in the local Spam and Trash folders (the actions crate's) is left out.
//!
//! State (the test engine's paths, for the checks): `search/last/query`, `search/last/count`, `search/last/stage` ("words": the
//! first send, the vector still to come; "ranked": the meaning order is in; "no_model"; "model_failed"; "failed"; "filtered"),
//! `search/last/reason` (the words to show above the rows for no_model, model_failed and failed; null otherwise), and
//! `search/last/hit/<n>/<message_id|matched_in|attachment_name|subject_ranges|score>`; lane rank: `search/last/related` (how many
//! hits of the last list are `Related`) and `search/last/hit/<n>/lift` (the hit's lift, two decimals; null when the mail has no
//! vector or the list is the first send; served for word matches too).
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use contracts::constants::SEARCH_SNIPPET_CHARS;
use contracts::{chunk_vector_key, ActionFailed, CommandOutcome, CoreError, CoreEvent, Embedder, MatchedIn, SearchHit, StreamKind};
use rusqlite::params;
use store::Store;
use sync_imap::FolderRole;

use crate::bodies::indexer::Indexer;
use crate::bodies::Bodies;
use crate::imap_source::Roles;
use crate::reads::json_text;
use crate::sync::{emit, Sinks};

/// The most rows one search sends.
/// docs/CONSTANTS.md SEARCH_RANKED_MAX.
const SEARCH_RANKED_MAX: usize = contracts::constants::SEARCH_RANKED_MAX as usize;
/// The score of a mail with no vector (c): under any cosine (-1 at the least), so the tail stays below the meaning rows by score too.
const NO_VECTOR_SCORE: f64 = -contracts::constants::NO_VECTOR_SCORE_BELOW;

/// SPEC A150: a mail that is not a word match is RELATED when its lift (deviations above the search's mean closeness) is at least
/// this. DigUp's gate (Searcher.swift:116 `zGate: Double = 2.0`); the lead sets it by a measure on the owner's mail. Ledger row owed
/// (docs/CONSTANTS.md RELATED_LIFT_MIN).
const RELATED_LIFT_MIN: f64 = contracts::constants::RELATED_LIFT_MIN;
/// The least deviation a lift divides by, so a flat field (every closeness alike) lifts no mail. DigUp's floor (Searcher.swift:338
/// `max(..., 0.02)`); depends on the model's cosine scale. Ledger row owed (docs/CONSTANTS.md RELATED_DEVIATION_FLOOR).
const RELATED_DEVIATION_FLOOR: f64 = contracts::constants::RELATED_DEVIATION_FLOOR;
/// Under this many mails with a vector in scope no mail is marked related (a mean and a deviation over a handful of mails say
/// nothing). DigUp's small-group line (Searcher.swift:342 "< 20 vectors"). Ledger row owed (docs/CONSTANTS.md RELATED_MIN_POPULATION).
const RELATED_MIN_POPULATION: usize = contracts::constants::RELATED_MIN_POPULATION as usize;
/// The most related mails one answer marks: the closest. Depends on the list's first screen, not the model. Ledger row owed
/// (docs/CONSTANTS.md RELATED_MAX).
const RELATED_MAX: usize = contracts::constants::RELATED_MAX as usize;
/// A related mail's closeness is at least this, beside its lift: the lift is relative to the search itself, so some mail stands out of
/// every search, also one typed with letters no mail is about (the owner's mail, 14 Nov 2026: 12 made-up queries each marked 6 to 19
/// mails by the lift alone; their closest mail reads 0.632 to 0.692). Depends on the model (docs/CONSTANTS.md RELATED_MIN_COSINE).
const RELATED_MIN_COSINE: f64 = contracts::constants::RELATED_MIN_COSINE;

/// What a search reads and where it sends.
pub(crate) struct Parts { pub store: Arc<Store>, pub indexer: Arc<Indexer>, pub roles: Roles, pub sinks: Sinks, pub utc_offset_s: i32 }

#[derive(Clone)]
struct Last { query: String, hits: Vec<SearchHit>, stage: &'static str, reason: Option<String>, lifts: Vec<Option<f64>> }

/// The words shown when the model failed on the query or the meaning index did not answer: the list stays words, then date.
const MODEL_FAILED_WORDS: &str = "Search by meaning could not run this time. The list shows the word matches, then the other mail by date.";

/// The words of `search/last/reason` for a stage set without a failure of its own: the model's absence says the Sources pane's words.
fn reason_of(stage: &str) -> Option<String> {
    match stage {
        "no_model" => Some(crate::bodies::crawl::MODEL_MISSING_WORDS.to_string()),
        "model_failed" => Some(MODEL_FAILED_WORDS.to_string()),
        _ => None,
    }
}

/// The searches of one core: the newest one's number, one ranking at a time, the last list sent.
#[derive(Default)]
pub(crate) struct SearchBook { generation: AtomicU64, running: Mutex<()>, last: Mutex<Option<Last>>, never_answers: std::sync::atomic::AtomicBool }

/// One stored mail in scope: its id and date.
struct Mail { id: String, at: i64 }

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

/// The most characters of the model's own words one log line keeps (query-embed: an MLX error can be long).
const MODEL_WORDS_MAX_CHARS: usize = contracts::constants::QUERY_MODEL_WORDS_MAX_CHARS as usize;

/// The model's error as the log keeps it: its own words on one line, every place the query appears replaced by `<query>` (a query
/// never reaches a log), at most MODEL_WORDS_MAX_CHARS characters.
fn model_words(e: &CoreError, query: &str) -> String {
    let mut words = e.to_string().replace(['\n', '\r'], " ");
    let q = query.trim();
    if !q.is_empty() { words = words.replace(q, "<query>"); }
    if words.chars().count() > MODEL_WORDS_MAX_CHARS { words = words.chars().take(MODEL_WORDS_MAX_CHARS).collect::<String>() + "…"; }
    if words.trim().is_empty() { "the model gave no words".to_string() } else { words }
}

fn note(store: &Store, line: &str) {
    eprintln!("search: {line}");
    crate::log::log_line(&crate::log::path_beside(store.path()), &format!("search: {line}"));
}

impl SearchBook {
    /// `Command::Search`: refuses empty words, else starts the ranking on its own thread and returns at once.
    pub(crate) fn start(self: &Arc<Self>, parts: Parts, query: String) -> Result<CommandOutcome, CoreError> {
        if query.trim().is_empty() { return Err(CoreError::Invalid { field: "query".into(), reason_words: "type something to search for".into() }); }
        // the rig mutant `search-never-answers`: Ok, and nothing is ever sent
        if self.never_answers.load(Ordering::SeqCst) { return Ok(CommandOutcome { undo_entry_id: None, created_id: None }); }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let me = self.clone();
        let parts = Arc::new(parts);
        std::thread::Builder::new().name("whoosh-search".into()).spawn(move || me.rank(generation, &parts, &query))
            .map_err(|e| crate::rust_core::refused(&format!("The search could not start: {e}")))?;
        Ok(CommandOutcome { undo_entry_id: None, created_id: None })
    }

    /// The rig mutant `search-never-answers` (set only by `open_production` with `rig` true).
    pub(crate) fn set_never_answers(&self, on: bool) { self.never_answers.store(on, Ordering::SeqCst); }

    fn current(&self, generation: u64) -> bool { self.generation.load(Ordering::SeqCst) == generation }

    /// Sends one list (unless a newer search started) and keeps it for the state paths, with each hit's lift (None: no vector read).
    #[allow(clippy::too_many_arguments)]
    fn send(&self, generation: u64, parts: &Parts, query: &str, hits: Vec<SearchHit>, lifts: Vec<Option<f64>>, stage: &'static str) {
        if !self.current(generation) { return; }
        *lock(&self.last) = Some(Last { query: query.to_string(), hits: hits.clone(), stage, reason: reason_of(stage), lifts });
        emit(&parts.sinks, StreamKind::Search, CoreEvent::SearchResults { query: query.to_string(), hits });
    }

    fn set_stage(&self, generation: u64, stage: &'static str) {
        if !self.current(generation) { return; }
        if let Some(l) = lock(&self.last).as_mut() { l.stage = stage; l.reason = reason_of(stage); }
    }

    fn rank(self: &Arc<Self>, generation: u64, parts: &Arc<Parts>, query: &str) {
        // part 2: the exact filters keep the mails that pass them all; the words left rank the kept mails
        let (filters, rest) = parse_query(query, parts.utc_offset_s);
        let words = rest.trim();
        // one ranking at a time, held over the reads and the sends only: the query's embedding runs outside it, so a newer search's
        // word list never waits for an older search's model
        let (mails, exact, embedder, coming) = {
            if !self.current(generation) { return; }
            let _one = lock(&self.running);
            if !self.current(generation) { return; }
            let mails = match in_scope(parts, &filters) {
                Ok(m) => m,
                Err(e) => {
                    note(&parts.store, &format!("the stored mail could not be read: {e}"));
                    if self.current(generation) {
                        let words = format!("Search could not read the stored mail: {e}");
                        *lock(&self.last) = Some(Last { query: query.to_string(), hits: vec![], stage: "failed", reason: Some(words.clone()), lifts: vec![] });
                        let failure = ActionFailed { account_name: String::new(), reason_words: words, retryable: true, code: None };
                        emit(&parts.sinks, StreamKind::Search, CoreEvent::ActionFailed { failure });
                    }
                    return;
                }
            };
            if words.is_empty() {
                // filters and no words: every kept mail is an exact match, by date, sent once
                let list = filtered_group(parts, &mails);
                note(&parts.store, &format!("{} mails pass the filters", list.len()));
                let lifts = vec![None; list.len()];
                return self.send(generation, parts, query, list, lifts, "filtered");
            }
            let exact = exact_group(parts, words, &mails);
            let embedder = parts.indexer.embedder();
            let first = assemble(parts, &exact, &[], &HashSet::new(), &mails);
            note(&parts.store, &format!("{} word hits of {} mails, first list {} rows", exact.len(), mails.len(), first.len()));
            // model-late: no model yet, and the shell said it is looking: the word list now, the meaning order when the model comes
            let coming = embedder.is_none() && crate::bodies::crawl::model_coming();
            // the first send reads no vector: no lift, no related mail
            let lifts = vec![None; first.len()];
            self.send(generation, parts, query, first, lifts, if embedder.is_some() || coming { "words" } else { "no_model" });
            (mails, exact, embedder, coming)
        };
        match embedder {
            Some(embedder) => self.meaning(generation, parts, query, words, &mails, &exact, embedder),
            None if coming => self.wait_for_model(generation, parts, query, words, mails, exact),
            None => {}
        }
    }

    /// model-late: the last search waits for the model's hand-over (one waiter per core: a newer search's replaces it), then ranks
    /// again, once, on a thread of its own; or the shell says the model is not on this Mac and the stage becomes no_model.
    fn wait_for_model(self: &Arc<Self>, generation: u64, parts: &Arc<Parts>, query: &str, words: &str, mails: Vec<Mail>, exact: Vec<SearchHit>) {
        let (me, p, q, w) = (self.clone(), parts.clone(), query.to_string(), words.to_string());
        parts.indexer.wait_for_embedder(Box::new(move |given| me.model_came(generation, &p, &q, &w, mails, exact, given)));
        // a hand-over (or the shell's "missing") between the first send and the line above found no waiter: look again; whoever takes
        // the waiter runs it, so it runs once
        let now = parts.indexer.embedder();
        if now.is_some() || crate::bodies::crawl::model_said_missing() {
            if let Some(f) = parts.indexer.take_waiter() { f(now); }
        }
    }

    /// model-late: the model was handed over (Some) or said missing (None) after the first list of `generation` was sent.
    #[allow(clippy::too_many_arguments)]
    fn model_came(self: &Arc<Self>, generation: u64, parts: &Arc<Parts>, query: &str, words: &str, mails: Vec<Mail>, exact: Vec<SearchHit>, given: Option<Arc<dyn Embedder>>) {
        // only while its first list is still the last one sent (a newer search makes this one stop)
        if !self.current(generation) { return; }
        let Some(embedder) = given else { return self.set_stage(generation, "no_model") };
        note(&parts.store, "the model came after the first list: ranking the last search by meaning");
        // the hand-over's caller (the shell's queue) never waits for the query's vector
        let (me, p, q, w) = (self.clone(), parts.clone(), query.to_string(), words.to_string());
        let spawned = std::thread::Builder::new().name("whoosh-search".into()).spawn(move || me.meaning(generation, &p, &q, &w, &mails, &exact, embedder));
        if let Err(e) = spawned {
            note(&parts.store, &format!("the second list could not start: {e}"));
            self.set_stage(generation, "model_failed");
        }
    }

    /// (b) and the second send: the query's vector, the meaning order, the ranked list.
    #[allow(clippy::too_many_arguments)]
    fn meaning(&self, generation: u64, parts: &Parts, query: &str, words: &str, mails: &[Mail], exact: &[SearchHit], embedder: Arc<dyn Embedder>) {
        if !self.current(generation) { return; }
        // (b) the query's vector: the model may take seconds to load the first time; the first list is on screen meanwhile
        let vector = match embedder.embed_query(words.to_string()) {
            Ok(v) => v,
            Err(e) => {
                // query-embed: the model's own words go to the log (never the query: `model_words` takes it out); the stage and the
                // words above the rows stay as they were
                note(&parts.store, &format!("the model did not embed the query: {}; the first list stays", model_words(&e, words)));
                return self.set_stage(generation, "model_failed");
            }
        };
        if !self.current(generation) { return; }
        let _one = lock(&self.running);
        if !self.current(generation) { return; }
        let at: HashMap<&str, i64> = mails.iter().map(|m| (m.id.as_str(), m.at)).collect();
        let meaning = match closeness(parts, &vector, &at) {
            Ok(Some(m)) => m,
            Ok(None) => return self.set_stage(generation, "no_model"),
            Err(e) => { note(&parts.store, &format!("the meaning index did not answer: {e}")); return self.set_stage(generation, "model_failed"); }
        };
        // (r) the related mark, from the cosines just read: no other pass over the store
        let lifts = lifts_of(&meaning);
        let related = related_of(&meaning, &lifts, exact);
        let ranked = assemble(parts, exact, &meaning, &related, mails);
        let lift_of: HashMap<&str, f64> = meaning.iter().zip(&lifts).map(|(c, l)| (c.id.as_str(), *l)).collect();
        let hit_lifts: Vec<Option<f64>> = ranked.iter().map(|h| lift_of.get(h.message_id.as_str()).copied()).collect();
        note(&parts.store, &format!("{} mails ranked by meaning, {} related, second list {} rows", meaning.len(), related.len(), ranked.len()));
        self.send(generation, parts, query, ranked, hit_lifts, "ranked");
    }

    /// `search/last/*`, the last list sent.
    pub(crate) fn state(&self, path: &str) -> Option<String> {
        let rest = path.strip_prefix("search/last/")?;
        let last = lock(&self.last).clone()?;
        let parts: Vec<&str> = rest.split('/').collect();
        match parts.as_slice() {
            ["query"] => Some(json_text(&last.query)),
            ["count"] => Some(last.hits.len().to_string()),
            ["stage"] => Some(json_text(last.stage)),
            ["reason"] => Some(last.reason.as_deref().map(json_text).unwrap_or_else(|| "null".into())),
            ["related"] => Some(last.hits.iter().filter(|h| h.matched_in == MatchedIn::Related).count().to_string()),
            ["hit", n, field] => {
                let n = n.parse::<usize>().ok()?;
                let h = last.hits.get(n)?;
                match *field {
                    "lift" => Some(match last.lifts.get(n).copied().flatten() { Some(l) => format!("{l:.2}"), None => "null".into() }),
                    "message_id" => Some(json_text(&h.message_id)),
                    "matched_in" => Some(json_text(matched_word(&h.matched_in))),
                    "attachment_name" => Some(h.attachment_name.as_deref().map(json_text).unwrap_or_else(|| "null".into())),
                    "score" => Some(format!("{}", h.score)),
                    "subject_ranges" => match &h.matched_in {
                        MatchedIn::SubjectWords { ranges } => Some(format!("[{}]", ranges.iter().map(|r| format!("[{},{}]", r.start, r.end)).collect::<Vec<_>>().join(","))),
                        _ => Some("null".into()),
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// (a) alone: the word hits in scope, each once, strongest first (ties: newest, then id). The word index failing is logged and gives none.
fn exact_group(parts: &Parts, words: &str, mails: &[Mail]) -> Vec<SearchHit> {
    let at: HashMap<&str, i64> = mails.iter().map(|m| (m.id.as_str(), m.at)).collect();
    let mut exact = match parts.indexer.search_words_ranked(words, SEARCH_RANKED_MAX) {
        Ok(h) => h,
        Err(e) => { note(&parts.store, &format!("the word index did not answer: {e}")); Vec::new() }
    };
    let mut seen = HashSet::new();
    exact.retain(|h| at.contains_key(h.message_id.as_str()) && seen.insert(h.message_id.clone()));
    exact.sort_by(|a, b| b.score.total_cmp(&a.score)
        .then_with(|| at.get(b.message_id.as_str()).cmp(&at.get(a.message_id.as_str())))
        .then_with(|| a.message_id.cmp(&b.message_id)));
    exact
}

/// The exact-match group of a search, never the ranked tail: what a reader of search hits as MEMBERS uses (a smart folder's rule
/// match and its count, a saved search's count). Same scope and order as the first part of the owner's list. Runs on the caller's
/// thread (it reads the store and the word index): call it off the main thread. No caller in the engine yet: smart folders and saved
/// searches answer empty today (`RustCore::smart_folders`).
#[allow(dead_code)]
pub(crate) fn exact_matches(parts: &Parts, query: &str) -> Result<Vec<SearchHit>, String> {
    let (filters, rest) = parse_query(query, parts.utc_offset_s);
    let words = rest.trim();
    if words.is_empty() && filters.is_empty() { return Ok(Vec::new()); }
    let mails = in_scope(parts, &filters)?;
    if words.is_empty() { return Ok(filtered_group(parts, &mails)); }
    Ok(exact_group(parts, words, &mails))
}

/// Filters and no words: every kept mail (already in date order, newest first) as an exact match, `SubjectWords` with no ranges,
/// score 0, the preview as its snippet; at most SEARCH_RANKED_MAX.
fn filtered_group(parts: &Parts, mails: &[Mail]) -> Vec<SearchHit> {
    let conn = search_text::source::open_reader(parts.store.path()).ok();
    mails.iter().take(SEARCH_RANKED_MAX).map(|m| {
        let preview: String = conn.as_ref().and_then(|c| c.query_row("SELECT preview FROM message WHERE id = ?1", params![m.id], |r| r.get::<_, String>(0)).ok()).unwrap_or_default();
        SearchHit { message_id: m.id.clone(), score: 0.0, snippet: preview.chars().take(SEARCH_SNIPPET_CHARS as usize).collect(), matched_in: MatchedIn::SubjectWords { ranges: Vec::new() }, attachment_name: None, attachment_note: None }
    }).collect()
}

// ---- part 2: the exact filters ----

/// The filters of one query (every one must pass; a key given twice must pass twice). Values are folded (case, accents).
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct Filters {
    pub from: Vec<String>,
    /// SPEC A153: `from:@<domain>`, the domain in lower case without its @; the sender's address must be at it (see `sender_is_at`)
    pub from_domain: Vec<String>,
    pub to: Vec<String>,
    pub subject: Vec<String>,
    pub attachment: Vec<String>,
    pub label: Vec<String>,
    /// received strictly before this time (the start of the named day, local time)
    pub before_ms: Option<i64>,
    /// received at or after this time (the start of the named day, local time: the day itself is kept)
    pub after_ms: Option<i64>,
    pub has_attachment: bool,
}

impl Filters {
    pub(crate) fn is_empty(&self) -> bool { *self == Filters::default() }
}

/// Days from 1970-01-01 to the date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY-MM-DD` as the epoch ms of that day's local midnight (`utc_offset_s` east of UTC); None for any other text.
fn day_start_ms(value: &str, utc_offset_s: i32) -> Option<i64> {
    let parts: Vec<&str> = value.split('-').collect();
    let [y, m, d] = parts.as_slice() else { return None };
    if y.len() != 4 || m.len() != 2 || d.len() != 2 { return None; }
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) { return None; }
    Some(days_from_civil(y, m, d) * 86_400_000 - i64::from(utc_offset_s) * 1000)
}

/// Splits the typed text into its filters and the words left. Grammar (case-blind keys), each filter a `key:value` that starts the text
/// or follows a character that is not a letter or digit:
/// `from:` `to:` `subject:` `attachment:` `label:` take a value: `"…"` (any characters but a quote; an unclosed quote runs to the end)
/// or a run of characters up to the next space or quote; `before:` and `after:` take `YYYY-MM-DD`; `has:` takes `attachment` or
/// `attachments`. `before:` keeps mail received strictly before the named day's local midnight, `after:` mail received at or after it
/// (the day itself is kept), so `after:2026-07-01 before:2026-08-01` is July. A filter whose value is empty or not understood stays in the text as words.
pub(crate) fn parse_query(raw: &str, utc_offset_s: i32) -> (Filters, String) {
    const KEYS: [&str; 8] = ["from", "to", "subject", "attachment", "label", "before", "after", "has"];
    let c: Vec<char> = raw.chars().collect();
    let mut f = Filters::default();
    let mut rest = String::new();
    let mut i = 0;
    while i < c.len() {
        let starts = i == 0 || !c[i - 1].is_alphanumeric();
        let mut j = i;
        while j < c.len() && c[j].is_ascii_alphabetic() { j += 1; }
        let key: String = c[i..j].iter().collect::<String>().to_ascii_lowercase();
        if starts && j > i && j < c.len() && c[j] == ':' && KEYS.contains(&key.as_str()) {
            let (value, next) = if j + 1 < c.len() && c[j + 1] == '"' {
                let mut k = j + 2;
                while k < c.len() && c[k] != '"' { k += 1; }
                (c[(j + 2).min(c.len())..k].iter().collect::<String>(), (k + 1).min(c.len()))
            } else {
                let mut k = j + 1;
                while k < c.len() && !c[k].is_whitespace() && c[k] != '"' { k += 1; }
                (c[j + 1..k].iter().collect::<String>(), k)
            };
            let value = value.trim().to_string();
            let folded = crate::rust_core::people::fold(&value);
            let taken = !value.is_empty() && match key.as_str() {
                // SPEC A153: a value that starts with "@" is a sender domain, not a person; "@" alone is no domain and stays words
                "from" if value.starts_with('@') => {
                    let d = value.trim_matches(|c: char| c == '@' || c.is_whitespace()).to_lowercase();
                    if d.is_empty() { false } else { f.from_domain.push(d); true }
                }
                "from" => { f.from.push(folded); true }
                "to" => { f.to.push(folded); true }
                "subject" => { f.subject.push(folded); true }
                "attachment" => { f.attachment.push(folded); true }
                "label" => { f.label.push(folded); true }
                "before" => match day_start_ms(&value, utc_offset_s) { Some(t) => { f.before_ms = Some(f.before_ms.map_or(t, |b| b.min(t))); true } None => false },
                "after" => match day_start_ms(&value, utc_offset_s) { Some(t) => { f.after_ms = Some(f.after_ms.map_or(t, |a| a.max(t))); true } None => false },
                "has" => { let ok = folded == "attachment" || folded == "attachments"; if ok { f.has_attachment = true; } ok }
                _ => false,
            };
            if taken { rest.push(' '); i = next; continue; }
        }
        rest.push(c[i]);
        i += 1;
    }
    (f, rest)
}

/// SPEC A153: whether the sender's address is at `domain` (lower case): the text after its last "@" is the domain itself or a
/// sub-domain of it (`x@mail.lorvan-hotels.example` is at `lorvan-hotels.example`; `x@notlorvan-hotels.example` is not). The
/// test engine's rule (SearchEngine.senderIsAt).
pub(crate) fn sender_is_at(domain: &str, address: &str) -> bool {
    let a = address.trim().to_lowercase();
    let Some((_, host)) = a.rsplit_once('@') else { return false };
    !domain.is_empty() && (host == domain || host.strip_suffix(domain).is_some_and(|head| head.ends_with('.')))
}

/// The test engine's words for `matched_in` (FixtureSearchBook.matchedWord).
fn matched_word(m: &MatchedIn) -> &'static str {
    match m { MatchedIn::SubjectWords { .. } => "subject", MatchedIn::AttachmentName => "attachment_name", MatchedIn::AttachmentContent => "attachment_content", MatchedIn::MeaningOnly => "meaning", MatchedIn::SenderWords { .. } => "sender", MatchedIn::TextWords { .. } => "text", MatchedIn::Related => "related" }
}

/// Every stored mail of every account, archived included, junk and trash left out (module comment), that passes every filter.
fn in_scope(parts: &Parts, filters: &Filters) -> Result<Vec<Mail>, String> {
    let accounts: Vec<String> = parts.store.account_summaries().map_err(|e| e.to_string())?.into_iter().map(|a| a.id).collect();
    let mut out_of_scope: HashSet<(String, String)> = HashSet::new();
    for role in [FolderRole::Junk, FolderRole::Trash] {
        out_of_scope.extend(crate::reads::folders_with_role(&parts.store, &parts.roles, &accounts, role));
    }
    let local = [actions::constants::SPAM_FOLDER, actions::constants::TRASH_FOLDER];
    let conn = search_text::source::open_reader(parts.store.path()).map_err(|e| e.to_string())?;
    use crate::rust_core::people::fold;
    // what the filters read beyond the message row, each read only when a filter needs it: id -> folded values
    let many = |sql: &str| -> Result<HashMap<String, Vec<String>>, String> {
        let mut st = conn.prepare(sql).map_err(|e| e.to_string())?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows { let (id, v) = row.map_err(|e| e.to_string())?; out.entry(id).or_default().push(fold(&v)); }
        Ok(out)
    };
    let none: HashMap<String, Vec<String>> = HashMap::new();
    let recipients = if filters.to.is_empty() { none.clone() } else {
        many("SELECT m.id, j.value FROM message m, json_each(m.to_json) j UNION ALL SELECT m.id, j.value FROM message m, json_each(m.cc_json) j")?
    };
    let files = if filters.attachment.is_empty() && !filters.has_attachment { none.clone() } else { many("SELECT message_id, file_name FROM attachment")? };
    let labels = if filters.label.is_empty() { none.clone() } else {
        many("SELECT a.message_id, l.name FROM label_assignment a JOIN label l ON l.id = a.label_id")?
    };
    let empty: Vec<String> = Vec::new();
    let of = |map: &HashMap<String, Vec<String>>, id: &str| -> Vec<String> { map.get(id).cloned().unwrap_or_else(|| empty.clone()) };
    let mut st = conn.prepare("SELECT id, account_id, folder, received_at_ms, sender_name, from_address, subject FROM message").map_err(|e| e.to_string())?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?,
        r.get::<_, String>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?))).map_err(|e| e.to_string())?;
    let mut mails = Vec::new();
    for row in rows {
        let (id, account, folder, at, sender_name, from_address, subject) = row.map_err(|e| e.to_string())?;
        if local.contains(&folder.as_str()) || out_of_scope.contains(&(account, folder)) { continue; }
        if filters.before_ms.is_some_and(|b| at >= b) || filters.after_ms.is_some_and(|a| at < a) { continue; }
        if !filters.from.is_empty() {
            let (name, address) = (fold(&sender_name), fold(&from_address));
            if !filters.from.iter().all(|v| name.contains(v.as_str()) || address.contains(v.as_str())) { continue; }
        }
        if !filters.from_domain.iter().all(|d| sender_is_at(d, &from_address)) { continue; }
        if !filters.subject.is_empty() {
            let subject = fold(&subject);
            if !filters.subject.iter().all(|v| subject.contains(v.as_str())) { continue; }
        }
        if !filters.to.is_empty() {
            let r = of(&recipients, &id);
            if !filters.to.iter().all(|v| r.iter().any(|x| x.contains(v.as_str()))) { continue; }
        }
        if !filters.attachment.is_empty() || filters.has_attachment {
            let names = of(&files, &id);
            if filters.has_attachment && names.is_empty() { continue; }
            if !filters.attachment.iter().all(|v| names.iter().any(|x| x.contains(v.as_str()))) { continue; }
        }
        if !filters.label.is_empty() {
            let held = of(&labels, &id);
            if !filters.label.iter().all(|v| held.iter().any(|x| x == v)) { continue; }
        }
        mails.push(Mail { id, at });
    }
    // date order, newest first, then id: the order (c) and the first send's tail use
    mails.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.id.cmp(&b.id)));
    Ok(mails)
}

/// One in-scope mail with a vector: its closeness (the best cosine of any of its pieces), that piece's chunk id, and the attachment
/// id when the piece is an attachment's (`chunk.part` 'attachment:<id>'), None for the body's.
struct Close { id: String, cos: f32, chunk: i64, attachment: Option<String> }

/// (r) and (b): each in-scope mail with a vector, its closeness and the piece that holds it, highest first (ties: newest, then id).
/// Reads every piece of every part (lane rank, SPEC A152: the body's and each attachment's). Ok(None): no model now (the meaning
/// index cannot open).
fn closeness(parts: &Parts, vector: &[f32], at: &HashMap<&str, i64>) -> Result<Option<Vec<Close>>, String> {
    let conn = search_text::source::open_reader(parts.store.path()).map_err(|e| e.to_string())?;
    let mut st = conn.prepare("SELECT id, message_id, part FROM chunk").map_err(|e| e.to_string())?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))).map_err(|e| e.to_string())?;
    // (chunk id, mail, attachment id): the attachment id is kept only for an attachment's piece
    let mut chunks: Vec<(i64, String, Option<String>)> = Vec::new();
    for row in rows {
        let (id, message, part) = row.map_err(|e| e.to_string())?;
        if at.contains_key(message.as_str()) { chunks.push((id, message, part.strip_prefix("attachment:").map(str::to_string))); }
    }
    let keys: Vec<u64> = chunks.iter().map(|(id, _, _)| chunk_vector_key(*id)).collect();
    let Some(cosines) = parts.indexer.chunk_cosines(vector, &keys)? else { return Ok(None) };
    let mut best: HashMap<String, (f32, i64, Option<String>)> = HashMap::new();
    for ((chunk, message, attachment), cos) in chunks.into_iter().zip(cosines) {
        let Some(cos) = cos.filter(|c| c.is_finite()) else { continue };
        match best.entry(message) {
            Entry::Occupied(mut o) => { if cos > o.get().0 { o.insert((cos, chunk, attachment)); } }
            Entry::Vacant(v) => { v.insert((cos, chunk, attachment)); }
        }
    }
    let mut out: Vec<Close> = best.into_iter().map(|(id, (cos, chunk, attachment))| Close { id, cos, chunk, attachment }).collect();
    out.sort_by(|a, b| b.cos.total_cmp(&a.cos)
        .then_with(|| at.get(b.id.as_str()).cmp(&at.get(a.id.as_str())))
        .then_with(|| a.id.cmp(&b.id)));
    Ok(Some(out))
}

/// (r): the lift of each mail of `meaning` (same order): (closeness - mean) / max(deviation, RELATED_DEVIATION_FLOOR), the mean and
/// the deviation (population: divided by N) taken over every mail of `meaning`, the word matches included.
fn lifts_of(meaning: &[Close]) -> Vec<f64> {
    if meaning.is_empty() { return Vec::new(); }
    let n = meaning.len() as f64;
    let mean = meaning.iter().map(|c| f64::from(c.cos)).sum::<f64>() / n;
    let variance = meaning.iter().map(|c| (f64::from(c.cos) - mean).powi(2)).sum::<f64>() / n;
    let deviation = variance.sqrt().max(RELATED_DEVIATION_FLOOR);
    meaning.iter().map(|c| (f64::from(c.cos) - mean) / deviation).collect()
}

/// (r): the related mails: no word match, lift at least RELATED_LIFT_MIN, closeness at least RELATED_MIN_COSINE, the RELATED_MAX closest
/// (`meaning` is closest first); none when fewer than RELATED_MIN_POPULATION mails have a vector.
fn related_of<'a>(meaning: &'a [Close], lifts: &[f64], exact: &[SearchHit]) -> HashSet<&'a str> {
    if meaning.len() < RELATED_MIN_POPULATION { return HashSet::new(); }
    let matches: HashSet<&str> = exact.iter().map(|h| h.message_id.as_str()).collect();
    meaning.iter().zip(lifts)
        .filter(|(c, lift)| **lift >= RELATED_LIFT_MIN && f64::from(c.cos) >= RELATED_MIN_COSINE && !matches.contains(c.id.as_str()))
        .take(RELATED_MAX)
        .map(|(c, _)| c.id.as_str())
        .collect()
}

/// The list: (a) the word hits, then (r) the related mails, then (b) the other mails by closeness, then (c) every other mail by
/// date; each mail once; at most SEARCH_RANKED_MAX rows. With no meaning order, (c) is every mail that is not a word hit.
fn assemble(parts: &Parts, exact: &[SearchHit], meaning: &[Close], related: &HashSet<&str>, mails: &[Mail]) -> Vec<SearchHit> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out: Vec<SearchHit> = Vec::new();
    for h in exact {
        if out.len() == SEARCH_RANKED_MAX { return out; }
        if seen.insert(h.message_id.as_str()) { out.push(h.clone()); }
    }
    let conn = search_text::source::open_reader(parts.store.path()).ok();
    let chunk_text = |id: i64| -> String {
        conn.as_ref().and_then(|c| c.prepare_cached("SELECT text FROM chunk WHERE id = ?1").and_then(|mut st| st.query_row(params![id], |r| r.get::<_, String>(0))).ok()).unwrap_or_default()
    };
    let preview = |id: &str| -> String {
        conn.as_ref().and_then(|c| c.prepare_cached("SELECT preview FROM message WHERE id = ?1").and_then(|mut st| st.query_row(params![id], |r| r.get::<_, String>(0))).ok()).unwrap_or_default()
    };
    // lane rank: the file's name when the best piece is an attachment's (one keyed read per row sent, never a pass)
    let file_name = |attachment: &str| -> Option<String> {
        conn.as_ref().and_then(|c| c.prepare_cached("SELECT file_name FROM attachment WHERE id = ?1").and_then(|mut st| st.query_row(params![attachment], |r| r.get::<_, String>(0))).ok())
    };
    let cut = |s: String| -> String { s.chars().take(SEARCH_SNIPPET_CHARS as usize).collect() };
    // (r) the related mails, then (b) the others; both closest first (`meaning`'s order)
    for want_related in [true, false] {
        for c in meaning.iter().filter(|c| related.contains(c.id.as_str()) == want_related) {
            if out.len() == SEARCH_RANKED_MAX { return out; }
            if seen.insert(c.id.as_str()) {
                let matched_in = if want_related { MatchedIn::Related } else { MatchedIn::MeaningOnly };
                let attachment_name = c.attachment.as_deref().and_then(&file_name);
                out.push(SearchHit { message_id: c.id.clone(), score: f64::from(c.cos), snippet: cut(chunk_text(c.chunk)), matched_in, attachment_name, attachment_note: None });
            }
        }
    }
    for m in mails {
        if out.len() == SEARCH_RANKED_MAX { return out; }
        if seen.insert(m.id.as_str()) {
            out.push(SearchHit { message_id: m.id.clone(), score: NO_VECTOR_SCORE, snippet: cut(preview(&m.id)), matched_in: MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None });
        }
    }
    out
}

// ---- the hidden check's mail pair (rig only, `WHOOSH_CORE_CHECK_SEARCH=1`) ----

/// The hidden check's pair: a mail that holds only "consultants" (subject and body), and a mail with no word in common with
/// "consultant" whose meaning is near it (an outside advisor). The check searches "consultant" (after a relaunch too): the first
/// is a word hit with the subject tinted, the second is in the list after it, by meaning.
pub(crate) const CHECK_PAIR: [&str; 2] = ["search-check:consultants", "search-check:advisor"];
/// How long the seed waits for the check to add its account through the sheet.
const CHECK_SEED_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Stores the pair under the first account once one exists (the check adds it through the sheet), once per data folder, and hands
/// both to the indexer. Runs on a thread of its own; the owner's copy never reads the variable.
pub(crate) fn check_seed(store: Arc<Store>, bodies: Arc<Bodies>) {
    let spawned = std::thread::Builder::new().name("whoosh-check-search".into()).spawn(move || {
        let t0 = std::time::Instant::now();
        let account = loop {
            if let Some(a) = store.account_summaries().ok().and_then(|v| v.into_iter().next()) { break a.id; }
            if t0.elapsed() > CHECK_SEED_WAIT { return note(&store, "check seed: no account was added; the pair was not stored"); }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        let ids: Vec<String> = CHECK_PAIR.iter().map(|s| s.to_string()).collect();
        if store.message(&ids[0]).ok().flatten().is_none() {
            let now = crate::rust_core::now_ms();
            let rows = vec![
                pair_row(&account, CHECK_PAIR[0], "Visit from the consultants", "The consultants arrive on Monday to review the plan.", now),
                pair_row(&account, CHECK_PAIR[1], "An outside advisor", "An outside advisor will look at our budget next week.", now - 60_000),
            ];
            if let Err(e) = store.insert_messages(rows) { return note(&store, &format!("check seed: the pair was not stored: {e}")); }
        }
        bodies.reindex(&ids);
        note(&store, "check seed: the pair is stored and handed to the indexer");
    });
    if let Err(e) = spawned { eprintln!("search: the check seed did not start: {e}"); }
}

fn pair_row(account: &str, id: &str, subject: &str, body: &str, at: i64) -> store::NewMessage {
    store::NewMessage {
        message: contracts::Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Search check".into(),
            from_address: format!("search-check@{}", crate::rust_core::CHECK_DOMAIN), to: vec![], subject: subject.into(), preview: body.into(),
            received_at_ms: at, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}


#[cfg(test)]
mod domain_filter_tests {
    //! SPEC A153: the parse and the match of `from:@<domain>` (what `in_scope` and `exact_matches` both read through `parse_query`).
    use super::*;

    /// Broken copy: the `"from" if value.starts_with('@')` arm is removed (the value lands in `from`, `from_domain` stays empty).
    #[test]
    fn the_at_form_is_a_domain_filter_and_the_plain_form_a_person() {
        let (f, rest) = parse_query("from:@Lorvan-Hotels.Example invoice from:zoe", 0);
        assert_eq!(f.from_domain, vec!["lorvan-hotels.example".to_string()]);
        assert_eq!(f.from, vec!["zoe".to_string()]);
        assert_eq!(rest.trim(), "invoice");
        let (f, _) = parse_query("FROM:\"@mail.lorvan-hotels.example\" from:@a.test", 0);
        assert_eq!(f.from_domain, vec!["mail.lorvan-hotels.example".to_string(), "a.test".to_string()], "quoted, any key case, a key given twice");
    }

    /// Broken copy: an empty domain is pushed (`from:@` then filters on "").
    #[test]
    fn an_at_with_no_domain_stays_words() {
        let (f, rest) = parse_query("from:@ hello", 0);
        assert!(f.is_empty(), "no filter: {f:?}");
        assert_eq!(rest, "from:@ hello");
    }

    /// Broken copy: `sender_is_at` is `host.ends_with(domain)` (the look-alike passes), or compares case-sensitively, or reads the text
    /// before the last "@" instead of after it.
    #[test]
    fn the_sender_is_at_the_domain_or_a_sub_domain_of_it_and_never_a_look_alike() {
        assert!(sender_is_at("lorvan-hotels.example", "a@lorvan-hotels.example"));
        assert!(sender_is_at("lorvan-hotels.example", "A@LORVAN-HOTELS.EXAMPLE"));
        assert!(sender_is_at("lorvan-hotels.example", " a@lorvan-hotels.example "));
        assert!(sender_is_at("lorvan-hotels.example", "a@mail.lorvan-hotels.example"), "a sub-domain, the test engine's rule");
        assert!(!sender_is_at("lorvan-hotels.example", "a@notlorvan-hotels.example"), "a look-alike");
        assert!(!sender_is_at("lorvan-hotels.example", "lorvan-hotels.example@other.test"), "the domain in the local part");
        assert!(!sender_is_at("lorvan-hotels.example", "lorvan-hotels.example"), "no address");
        assert!(!sender_is_at("", "a@b.test"), "no domain");
    }
}
