//! sync-full-r1: the low-priority indexer. Every body the core keeps (the crawl's, the prefetch's, the reader's) joins the word
//! index (search-text, `<data dir>/index/text`) and, once the shell gave the on-device model (`set_background_embedder`), the
//! meaning index (search-vector, `<data dir>/index/vector`), each body once: a message the meaning index holds is never embedded
//! again. One thread at the OS's background priority does it, EMBED_BATCH mails per step, a rest after each step (INDEX_DUTY_PCT),
//! no step while the process's footprint is at EMBED_INDEX_CEILING_MB (it waits EMBED_PAUSE_MS and reads it again), none while
//! the crawl's conditions hold it (battery, heat). The model is only called while there is work, so the shell's model unloads
//! EMBED_IDLE_UNLOAD_S after the last step.
//! Round 2 (contracts delta 21): each body is cut into `chunk` rows (pieces, SPEC A151: cleaned, sized in the model's tokens, search-vector
//! `pieces`; before: CHUNK_TOKENS whitespace words with CHUNK_OVERLAP); each chunk's vector is filed under
//! `chunk_vector_key(chunk.id)`, the chunk's OWN id, never reused, so a deleted message's vectors can never point at another message.
//! A chunk is marked `embedded_at_ms` only once the meaning index holding its vector is on disk, so "indexed" is exact across a crash.
//! A meaning hit is read back through its chunk row (date, sender, folder, subject, text); a hit whose chunk is gone (its message was
//! deleted) is skipped, and the next fold of the index drops it. An index folder without the chunk-key marker (vectors filed under
//! message rows before delta 21) is removed and rebuilt from the chunks.
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use contracts::constants::{EMBED_BATCH, EMBED_INDEX_CEILING_MB, EMBED_PAUSE_MS};
use contracts::{chunk_vector_key, Embedder, SearchHit};
use search_text::TextIndex;
use search_vector::pieces::{self, ByteBound, ModelTokenizer};
use search_vector::{Tokenizer, VectorIndex};
use store::Store;

use super::lock;

// Sizes this file needs that have no row in docs/CONSTANTS.md yet (proposed in docs/lanes/sync-full-r1.md); each is named here once.
/// The word index's writer memory (tantivy needs 15 MB per thread at least) and threads: the app's own index stays light.
pub const TEXT_INDEX_HEAP_BYTES: usize = 50_000_000;
pub const TEXT_INDEX_THREADS: usize = 1;
/// The share of one core the indexer works at: after a step of t, it rests t x (100 - INDEX_DUTY_PCT) / INDEX_DUTY_PCT.
pub const INDEX_DUTY_PCT: u32 = 25;
/// The longest rest after one step.
pub const INDEX_REST_MAX_MS: u64 = 2_000;
/// Characters of one body given to the indexes at most (about 10,000 words, 20 chunks).
pub const INDEX_BODY_MAX_CHARS: usize = 50_000;
/// The meaning index is written to disk at least this often while it works, and when the work runs out.
pub const VECTOR_COMMIT_S: u64 = 30;
/// Chunks the meaning index holds in memory before it folds them into its mapped base (about 15 MB of i8 vectors and graph).
pub const VECTOR_DELTA_MAX_CHUNKS: usize = 20_000;
/// Rows read per page while the indexer walks the stored bodies.
pub const INDEX_SCAN_ROWS: usize = 1_000;
/// The file that says a meaning index files its vectors under chunk ids (delta 21); a folder without it is rebuilt.
pub const CHUNK_KEYS_MARKER: &str = "chunk-keys-v1";
/// pieces (SPEC A151): the file that says the meaning index holds pieces cut by today's rule. A folder without it is removed, every
/// chunk is marked without its vector, and the backfill cuts every stored mail again (the `current` check sees the new text) and embeds
/// it once. "pieces-v1": pieces cut by the token rule (search-vector `pieces`); a folder without it held pieces of 512 whitespace words
/// the model read only the start of. "pieces-v2" (stale-fix, item c1436f49): `text_of` reads a text part of page code as its words (14 Nov),
/// so pieces cut before it are stale with no store write. ANY change of `text_of` or of the cut moves this marker (the pin test
/// `text_of_is_pinned_on_fixed_samples` in tests/engine_stale_pieces.rs says so when `text_of` changes).
pub const PIECE_RULE_MARKER: &str = "pieces-v2";

/// One chunk the meaning search found: the contract's `Chunk` row (its id, message, date, sender, folder, subject, text), the
/// message's account, and the cosine.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkHit { pub chunk: contracts::Chunk, pub account_id: String, pub score: f32 }

#[derive(Default)]
struct Work { ids: VecDeque<String>, queued: HashSet<String>, rows: VecDeque<i64>, queued_rows: HashSet<i64>, backfill: bool, stop: bool }

struct Inner {
    store: Arc<Store>,
    dir: PathBuf,
    log: PathBuf,
    work: Mutex<Work>,
    cv: Condvar,
    embedder: Mutex<Option<Arc<dyn Embedder>>>,
    held: AtomicBool,
    /// A job is taken and not finished. Set by `next_job` under the work lock, in the same breath as the queues are emptied, so `settle`
    /// never reads empty queues and "not busy" while a taken job runs (round 3: it once answered idle in that gap).
    busy: AtomicBool,
    /// Tests only: a pause between taking a job and running it (the gap `busy` must cover).
    pause_after_take: Mutex<Option<Duration>>,
    /// Chunks whose vectors are in the meaning index in memory, not yet on disk: marked embedded after the next commit.
    unmarked: Mutex<Vec<i64>>,
    /// Called after each step (the crawl's throttled progress report).
    after_step: Mutex<Option<Box<dyn Fn(bool) + Send + Sync>>>,
    text: Mutex<Option<Arc<TextIndex>>>,
    text_failed: AtomicBool,
    vector: Mutex<Option<Arc<VectorIndex>>>,
    /// model-late: what runs once when a model is handed over or the shell says there is none (the last search waiting for it).
    waiter: Mutex<Option<EmbedderWaiter>>,
    /// pieces: the model's tokenizer.json when the shell named it (`set_tokenizer_file`); else the one under the app's Models folder.
    tokenizer_file: Mutex<Option<PathBuf>>,
    /// pieces: the counter the pieces are sized with, built on the first step that needs it and dropped when the work runs out.
    counter: Mutex<Option<Arc<dyn Tokenizer>>>,
    /// pieces: pieces sent to the model that did not fit its input with their title (by the counter): 0 by construction.
    over_limit: AtomicU64,
    /// pieces, tests: a counter given by `use_counter` stays across idle times.
    pinned: AtomicBool,
}

/// model-late: run once with the model handed over (Some), or with None when the shell said the model is not on this Mac.
pub(crate) type EmbedderWaiter = Box<dyn FnOnce(Option<Arc<dyn Embedder>>) + Send>;

pub struct Indexer { inner: Arc<Inner>, thread: Mutex<Option<JoinHandle<()>>> }

impl Drop for Indexer {
    fn drop(&mut self) {
        lock(&self.inner.work).stop = true;
        self.inner.cv.notify_all();
        // the last owner can be the indexer's own thread (its progress hook upgraded a weak handle): it never joins itself
        if let Some(h) = lock(&self.thread).take() { if h.thread().id() != std::thread::current().id() { let _ = h.join(); } }
    }
}

/// The thread's priority: QOS_CLASS_BACKGROUND (macOS runs it on the efficiency cores and throttles its disk use).
#[cfg(target_os = "macos")]
fn background_priority() {
    extern "C" { fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32; }
    const QOS_CLASS_BACKGROUND: u32 = 0x09;
    // SAFETY: sets the calling thread's own QoS class; no pointer is passed.
    let _ = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND, 0) };
}
#[cfg(not(target_os = "macos"))]
fn background_priority() {}

/// The process's footprint in MB (phys_footprint, what Activity Monitor shows as Memory), None when the OS does not say.
#[cfg(target_os = "macos")]
pub fn footprint_mb() -> Option<u64> {
    extern "C" { fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32; fn getpid() -> i32; }
    const RUSAGE_INFO_V2: i32 = 2;
    // rusage_info_v2 is 21 u64 after a 16-byte uuid; ri_phys_footprint is the 10th u64 (index 9)
    let mut buf = [0u64; 64];
    // SAFETY: the buffer is larger than rusage_info_v2 and lives across the call.
    let r = unsafe { proc_pid_rusage(getpid(), RUSAGE_INFO_V2, buf.as_mut_ptr()) };
    (r == 0).then(|| buf[9] / (1024 * 1024))
}
#[cfg(not(target_os = "macos"))]
pub fn footprint_mb() -> Option<u64> { None }

fn now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }

/// The text a stored body is indexed by: the plain text, else the sanitised html as text, else the preview, else the subject; capped.
/// Public for the pin test (stale-fix): a change of its output moves PIECE_RULE_MARKER.
pub fn text_of(body_text: Option<&str>, body_html: Option<&str>, preview: &str, subject: &str) -> String {
    let t = match body_text.filter(|t| !t.trim().is_empty()) {
        // owner's picture 216: a text part of page code, kept before the parser read such a part as its words, is indexed by its words
        Some(t) if parse::text_is_page_code(t) => parse::page_code_words(&parse::page_code_as_page(t)),
        Some(t) => t.to_string(),
        None => match body_html.map(parse::safe_html_to_text).filter(|h| !h.trim().is_empty()) {
            Some(h) => h,
            None => if preview.trim().is_empty() { subject.to_string() } else { preview.to_string() },
        },
    };
    if t.chars().count() > INDEX_BODY_MAX_CHARS { t.chars().take(INDEX_BODY_MAX_CHARS).collect() } else { t }
}

enum Job { Backfill, Step(Vec<String>, Vec<i64>) }

/// pieces: the installed model's tokenizer.json: `<data dir>/..` is the app's Whoosh folder (Swift `ModelsFolder.whooshRoot`: the
/// support folder's Whoosh, whose Core folder holds the store), and the installer puts the model under
/// `Models/embeddinggemma-2/<revision>`. The first revision folder (by name) that holds one. A test's store in a temporary folder finds none.
fn installed_tokenizer(store_path: &std::path::Path) -> Option<PathBuf> {
    let models = store_path.parent()?.parent()?.join("Models").join("embeddinggemma-2");
    let mut revisions: Vec<PathBuf> = std::fs::read_dir(&models).ok()?.flatten().map(|e| e.path()).collect();
    revisions.sort();
    revisions.into_iter().find_map(|r| ModelTokenizer::file_in(&r))
}

impl Indexer {
    /// Starts the indexer's thread over `store`; its index folders sit beside the database. Nothing is opened before the first
    /// body arrives, so a core that keeps no body costs one sleeping thread.
    pub fn start(store: Arc<Store>, log: PathBuf) -> Indexer {
        let dir = store.path().with_file_name("index");
        let inner = Arc::new(Inner {
            store, dir, log, work: Mutex::new(Work::default()), cv: Condvar::new(), embedder: Mutex::new(None), held: AtomicBool::new(false),
            busy: AtomicBool::new(false), pause_after_take: Mutex::new(None), unmarked: Mutex::new(Vec::new()), after_step: Mutex::new(None), text: Mutex::new(None), text_failed: AtomicBool::new(false),
            vector: Mutex::new(None), waiter: Mutex::new(None), tokenizer_file: Mutex::new(None), counter: Mutex::new(None), over_limit: AtomicU64::new(0), pinned: AtomicBool::new(false),
        });
        let me = inner.clone();
        let thread = std::thread::Builder::new().name("whoosh-indexer".into()).spawn(move || me.run()).ok();
        Indexer { inner, thread: Mutex::new(thread) }
    }

    /// Bodies just kept: they join the indexes (once each; a message already in the meaning index is not embedded again).
    pub fn enqueue(&self, ids: &[String]) {
        if ids.is_empty() { return; }
        let mut w = lock(&self.inner.work);
        for id in ids { if w.queued.insert(id.clone()) { w.ids.push_back(id.clone()); } }
        drop(w);
        self.inner.cv.notify_all();
    }

    /// The meaning index's model (None: words only). A new model walks every stored body once for the ones it does not hold.
    pub fn set_embedder(&self, embedder: Option<Arc<dyn Embedder>>) {
        let old = lock(&self.inner.vector).take();
        if let Some(v) = old { self.inner.commit_and_mark(&v); }
        let some = embedder.clone();
        *lock(&self.inner.embedder) = embedder;
        if some.is_some() { lock(&self.inner.work).backfill = true; }
        self.inner.cv.notify_all();
        // model-late: the search that ran before the model came ranks again now (it starts its own thread and returns at once)
        if let Some(e) = some { if let Some(f) = self.take_waiter() { f(Some(e)); } }
    }

    /// model-late: `f` runs once, when a model is handed over (`set_embedder(Some)`, with it) or the shell says the model is not on
    /// this Mac (`said_missing`, with None). Only one waits: a newer call replaces the older one (only the last search ranks again).
    pub(crate) fn wait_for_embedder(&self, f: EmbedderWaiter) { *lock(&self.inner.waiter) = Some(f); }

    /// model-late: the waiter back, if no hand-over took it yet (whoever takes it runs it, so it runs once).
    pub(crate) fn take_waiter(&self) -> Option<EmbedderWaiter> { lock(&self.inner.waiter).take() }

    /// model-late: the shell said the model is not on this Mac: the waiting search keeps its first list, stage no_model.
    pub(crate) fn said_missing(&self) { if let Some(f) = self.take_waiter() { f(None); } }

    /// pieces: the model's tokenizer.json (the folder the shell's model loads from). None: the indexer looks under the app's Models
    /// folder, and without one counts with the byte bound (pieces that always fit, smaller than they need be).
    pub fn set_tokenizer_file(&self, path: Option<PathBuf>) {
        *lock(&self.inner.tokenizer_file) = path;
        *lock(&self.inner.counter) = None;
    }

    /// pieces: how many pieces this indexer sent to the model that did not fit its input with their title. The lead's check asserts 0.
    pub fn pieces_over_limit(&self) -> u64 { self.inner.over_limit.load(Ordering::SeqCst) }

    /// The engine read `index/pieces/<after rowid>/<limit>` (checks/ownercopy-pieces.py, item c1436f49): COUNTS ONLY, never a word of a mail.
    /// See `Inner::piece_counts`.
    pub fn piece_counts(&self, after_rowid: i64, limit: usize) -> String { self.inner.piece_counts(after_rowid, limit) }

    /// pieces, tests: the counter the next step sizes pieces with is `t` (None: found again as in the app).
    pub fn use_counter(&self, t: Option<Arc<dyn Tokenizer>>) {
        self.inner.pinned.store(t.is_some(), Ordering::SeqCst);
        *lock(&self.inner.counter) = t;
    }

    /// The crawl's conditions (battery, heat) hold the indexer too. A change of the hold writes ONE line to core.log with the count waiting
    /// (item b43515ba: a hold that kept the meaning index from growing said nothing; counts only, never mail text).
    pub fn set_held(&self, held: bool) {
        let was = self.inner.held.swap(held, Ordering::SeqCst);
        if was != held {
            let waiting = self.inner.waiting();
            self.inner.note(&if held { format!("held by battery or heat; {waiting} mails and rows wait") }
                            else { format!("no longer held by battery or heat; {waiting} mails and rows wait") });
        }
        self.inner.cv.notify_all();
    }

    /// Tests only: after taking each job, the indexer waits `d` before running it (None: no wait).
    pub fn pause_after_take(&self, d: Option<Duration>) { *lock(&self.inner.pause_after_take) = d; }

    /// `f` runs after each step of the indexer (on its thread; it must not wait), with `true` when the work just ran out.
    pub fn after_step(&self, f: Box<dyn Fn(bool) + Send + Sync>) { *lock(&self.inner.after_step) = Some(f); }

    /// Waits until the indexer has nothing left to do and the word index committed (true), or `within` passed (false).
    pub fn settle(&self, within: Duration) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < within {
            let idle = { let w = lock(&self.inner.work); w.ids.is_empty() && w.rows.is_empty() && !w.backfill } && !self.inner.busy.load(Ordering::SeqCst)
                && lock(&self.inner.unmarked).is_empty();
            let words = lock(&self.inner.text).as_ref().is_none_or(|t| t.pending() == 0);
            if idle && words { return true; }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// The word search over every stored body (empty until the word index is open).
    pub fn search_words(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let text = lock(&self.inner.text).clone();
        text.and_then(|t| t.search(query, limit).ok()).unwrap_or_default()
    }

    /// search-forms (SPEC A141): the owner's search on the word index, typed words also finding the words that start with them
    /// (`TextIndex::search_ranked`), up to `limit` hits, strongest first. Opens the word index when this launch has not yet (a
    /// relaunch with nothing new to index never opened it), so it runs off the caller's thread. Err: the index cannot answer.
    pub fn search_words_ranked(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
        let text = self.inner.text_index().ok_or_else(|| "the word index is not open".to_string())?;
        text.search_ranked(query, limit).map_err(|e| e.to_string())
    }

    /// indexer-race, tests: opens the word index now with no work queued (what a search does on a launch with nothing new to index);
    /// true once it is open. A reopened index starts its exact comparison with the table at once.
    pub fn open_words(&self) -> bool { self.inner.text_index().is_some() }

    /// search-forms: the model the meaning index embeds with (None: the shell gave none, or took it back).
    pub fn embedder(&self) -> Option<Arc<dyn Embedder>> { lock(&self.inner.embedder).clone() }

    /// search-forms: the exact cosine of a query vector to each chunk's vector (keys: `chunk_vector_key` of the chunk ids), None for
    /// a chunk with no vector. Opens the meaning index when this launch has not yet. Ok(None): no model, so no meaning index.
    pub fn chunk_cosines(&self, query: &[f32], keys: &[u64]) -> Result<Option<Vec<Option<f32>>>, String> {
        let Some(vector) = self.inner.vector_index()? else { return Ok(None) };
        Ok(Some(vector.cosines(query, keys)))
    }

    /// The meaning search: the `k` closest chunks, each with its message's date, sender, folder, subject and the chunk's text.
    pub fn search_meaning(&self, query: &str, k: usize) -> Result<Vec<ChunkHit>, String> {
        let Some(vector) = lock(&self.inner.vector).clone() else { return Ok(Vec::new()) };
        // over-ask: a vector whose chunk is gone (its message was deleted) is skipped until the next fold drops it
        let hits = vector.search(query, k.saturating_mul(2).max(k)).map_err(|e| e.to_string())?;
        let reads = self.inner.store.background_reads().map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for h in hits {
            let Ok(id) = i64::try_from(h.message_key) else { continue };
            let Some((chunk, account_id)) = reads.chunk(id).map_err(|e| e.to_string())? else { continue };
            out.push(ChunkHit { chunk, account_id, score: h.score });
            if out.len() == k { break; }
        }
        Ok(out)
    }

    /// fix-blockforget (docs/lanes/real-accounts.md finding 2): an account was removed from this Mac. `message_ids` and `chunk_ids` are what
    /// the store held of it, read before the removal (`Store::account_index_rows`). Its bodies leave the indexer's queue; each chunk's vector
    /// leaves the meaning index when that index is open (in memory at once; the next commit or fold writes it); the word index is asked to
    /// compare itself with the table, which drops the documents of the rows that are gone on its own thread (the removal's delete asks the
    /// same). A meaning index not opened this launch keeps vectors whose chunks are gone: a search skips them (`search_meaning`, the search's
    /// candidates come from the store) and the next fold drops them. Returns how many vectors were removed. No store write, no wait.
    pub fn forget_account(&self, message_ids: &[String], chunk_ids: &[i64]) -> usize {
        {
            let gone: HashSet<&String> = message_ids.iter().collect();
            let mut w = lock(&self.inner.work);
            w.ids.retain(|id| !gone.contains(id));
            w.queued.retain(|id| !gone.contains(id));
        }
        let chunks: HashSet<i64> = chunk_ids.iter().copied().collect();
        lock(&self.inner.unmarked).retain(|id| !chunks.contains(id));
        let mut removed = 0;
        let vector = lock(&self.inner.vector).clone();
        if let Some(vector) = vector {
            for id in chunk_ids {
                let key = chunk_vector_key(*id);
                if vector.contains_message(key) && vector.remove_message(key).is_ok() { removed += 1; }
            }
        }
        let text = lock(&self.inner.text).clone();
        if let Some(t) = text { t.request_repair(); }
        removed
    }
}

impl Inner {
    fn note(&self, line: &str) {
        eprintln!("indexer: {line}");
        crate::log::log_line(&self.log, &format!("indexer: {line}"));
    }

    fn stopped(&self) -> bool { lock(&self.work).stop }

    /// The mails and chunk rows queued for the indexer now (a count for the log lines).
    fn waiting(&self) -> usize { let w = lock(&self.work); w.ids.len() + w.rows.len() }

    /// Sleeps `d`, or less when the indexer is stopped.
    fn rest(&self, d: Duration) {
        let w = lock(&self.work);
        if w.stop { return; }
        let _ = self.cv.wait_timeout_while(w, d, |w| !w.stop);
    }

    fn next_job(&self) -> Option<Job> {
        let mut w = lock(&self.work);
        loop {
            if w.stop { return None; }
            let ready = w.backfill || !w.ids.is_empty() || !w.rows.is_empty();
            if ready && !self.held.load(Ordering::SeqCst) { break; }
            // no timer while idle: every change that can start a job (work, a model, a release, a stop) notifies
            w = self.cv.wait(w).unwrap_or_else(|e| e.into_inner());
        }
        // taken = busy, under the same lock that empties the queues
        self.busy.store(true, Ordering::SeqCst);
        if w.backfill { w.backfill = false; return Some(Job::Backfill); }
        let n = EMBED_BATCH as usize;
        let ids: Vec<String> = (0..n).filter_map(|_| w.ids.pop_front()).collect();
        for id in &ids { w.queued.remove(id); }
        let rows: Vec<i64> = (0..n.saturating_sub(ids.len())).filter_map(|_| w.rows.pop_front()).collect();
        for r in &rows { w.queued_rows.remove(r); }
        Some(Job::Step(ids, rows))
    }

    fn run(self: Arc<Self>) {
        background_priority();
        let mut last_commit = Instant::now();
        while let Some(job) = self.next_job() {
            let pause = *lock(&self.pause_after_take);
            if let Some(d) = pause { std::thread::sleep(d); }
            // already set by `next_job`; kept so a step reads busy whatever took the job
            self.busy.store(true, Ordering::SeqCst);
            let t0 = Instant::now();
            let done = match job { Job::Backfill => self.backfill(), Job::Step(ids, rows) => self.step(ids, rows) };
            if let Err(e) = done { self.note(&e); }
            self.keep_vectors(&mut last_commit);
            let drained = { let w = lock(&self.work); w.ids.is_empty() && w.rows.is_empty() && !w.backfill } && lock(&self.unmarked).is_empty();
            if let Some(f) = lock(&self.after_step).as_ref() { f(drained); }
            // pieces: the model's tokenizer is held only while there is work (tens of MB); built again on the next step
            if drained && !self.pinned.load(Ordering::SeqCst) { *lock(&self.counter) = None; }
            self.busy.store(false, Ordering::SeqCst);
            let worked = t0.elapsed();
            let rest = worked * (100 - INDEX_DUTY_PCT) / INDEX_DUTY_PCT;
            self.rest(rest.min(Duration::from_millis(INDEX_REST_MAX_MS)));
        }
        if let Some(v) = lock(&self.vector).take() { self.commit_and_mark(&v); }
    }

    /// Writes the meaning index to disk, THEN marks the chunks whose vectors it now holds (a crash in between re-embeds them, never
    /// leaves a chunk marked whose vector was lost).
    fn commit_and_mark(&self, vector: &VectorIndex) {
        if let Err(e) = vector.commit() { self.note(&format!("the meaning index was not kept: {e}")); return; }
        let ids = std::mem::take(&mut *lock(&self.unmarked));
        if ids.is_empty() { return; }
        if let Err(e) = self.store.mark_chunks_embedded(ids.clone(), now_ms()) {
            self.note(&format!("the chunks were not marked embedded: {e}"));
            lock(&self.unmarked).extend(ids);
            return;
        }
        // one line per finished embedding pass, counts only (the lead's read of the installed log, 2026-11-12)
        let waiting = self.waiting();
        self.note(&format!("{} chunks embedded in this pass; {} mails and rows wait", ids.len(), waiting));
    }

    fn text_index(&self) -> Option<Arc<TextIndex>> {
        if crate::rig::on(self.store.path(), crate::rig::RigMutant::IndexNeverWrites) { return None; }
        let mut slot = lock(&self.text);
        if slot.is_none() {
            match TextIndex::open_with(&self.store, &self.dir.join("text"), TEXT_INDEX_HEAP_BYTES, TEXT_INDEX_THREADS, true) {
                Ok(t) => *slot = Some(Arc::new(t)),
                Err(e) => if !self.text_failed.swap(true, Ordering::SeqCst) { self.note(&format!("the word index did not open: {e}")); },
            }
        }
        slot.clone()
    }

    /// The meaning index, opened with the shell's model (None while no model was given). A folder without the chunk-key marker holds
    /// vectors filed under message rows (before delta 21): it is removed. A new folder holds no vector, so every chunk is marked
    /// not embedded and the backfill embeds them again.
    fn vector_index(&self) -> Result<Option<Arc<VectorIndex>>, String> {
        let Some(embedder) = lock(&self.embedder).clone() else { return Ok(None) };
        let mut slot = lock(&self.vector);
        if slot.is_none() {
            let dir = self.dir.join("vector");
            let marker = dir.join(CHUNK_KEYS_MARKER);
            let rule = dir.join(PIECE_RULE_MARKER);
            if dir.exists() && !marker.exists() {
                std::fs::remove_dir_all(&dir).map_err(|e| format!("the meaning index filed under message rows was not removed: {e}"))?;
                self.note("the meaning index filed under message rows was removed; it is rebuilt from the chunks");
            } else if dir.exists() && !rule.exists() {
                // pieces: cut once again by the token rule; the old vectors go with the folder
                std::fs::remove_dir_all(&dir).map_err(|e| format!("the meaning index cut by the older rule was not removed: {e}"))?;
                self.note("the meaning index held pieces cut by the older rule; every stored mail is cut again and embedded once");
            }
            if !marker.exists() || !rule.exists() {
                let n = self.store.unembed_all_chunks().map_err(|e| e.to_string())?;
                if n > 0 { self.note(&format!("{n} chunks have no vector any more; they are embedded again")); }
            }
            let v = VectorIndex::open(&dir, embedder).map_err(|e| format!("the meaning index did not open: {e}"))?;
            std::fs::write(&marker, "vectors are filed under chunk ids (contracts chunk_vector_key)\n").map_err(|e| format!("the meaning index's marker was not written: {e}"))?;
            std::fs::write(&rule, "pieces are cut in the model's tokens (search-vector pieces)\n").map_err(|e| format!("the meaning index's piece marker was not written: {e}"))?;
            *slot = Some(Arc::new(v));
        }
        Ok(slot.clone())
    }

    /// Every stored body not fully in the meaning index (no chunk yet, or a chunk without its vector) is queued, once per model.
    fn backfill(&self) -> Result<(), String> {
        if self.vector_index()?.is_none() { return Ok(()); }
        let reads = self.store.background_reads().map_err(|e| format!("the store did not open a read: {e}"))?;
        let mut after = 0i64;
        loop {
            if self.stopped() { return Ok(()); }
            let page = reads.bodies_to_index_after(after, INDEX_SCAN_ROWS).map_err(|e| e.to_string())?;
            let Some(last) = page.last() else { break };
            after = *last;
            let mut w = lock(&self.work);
            for rowid in page { if w.queued_rows.insert(rowid) { w.rows.push_back(rowid); } }
        }
        Ok(())
    }

    /// ownercopy-pieces (item c1436f49, SPEC A151): for up to `limit` stored bodies after row `after_rowid`, the counts the finish line reads, as one
    /// JSON object, with the engine's own text (`text_of`), cleaner (`pieces::clean_for_meaning`) and counter (the one the steps size pieces with):
    /// `mails` read under the cap (INDEX_BODY_MAX_CHARS) and `over_cap` left out; `words` of the cleaned texts; `words_in_no_piece`, a word in no
    /// stored body piece of its mail; `words_in_no_vector`, a word in no piece that has its vector; `pieces`; `pieces_cut`, stored pieces that do not
    /// fit the model's input with their title (the model would cut them); `attachment_pieces_with_vector`; `last`, the last row read (null: none
    /// left); `counter`, "model tokenizer" when a tokenizer.json is known, else "byte bound". Never a subject, an address or a word.
    fn piece_counts(&self, after_rowid: i64, limit: usize) -> String {
        let reads = match self.store.background_reads() { Ok(r) => r, Err(e) => return piece_counts_error(&format!("the store did not open a read: {e}")) };
        let rows = match reads.stored_after(after_rowid, limit) { Ok(r) => r, Err(e) => return piece_counts_error(&e.to_string()) };
        let last = rows.last().map(|r| r.0);
        let rowids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let bodies = match reads.index_rows(&rowids) { Ok(b) => b, Err(e) => return piece_counts_error(&e.to_string()) };
        let handed = || super::crawl::embedding_model_folder().and_then(|f| ModelTokenizer::file_in(std::path::Path::new(&f)));
        let known = lock(&self.tokenizer_file).clone().or_else(handed).or_else(|| installed_tokenizer(self.store.path())).is_some();
        let tok = self.counter();
        let (mut mails, mut over_cap, mut words, mut no_piece, mut no_vector, mut pieces_n, mut cut, mut files) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
        // round 20 (main111: 4 words in no stored piece of 72809): why each word is in no piece, and which mails (row ids and counts only)
        let (mut lost_by_cut, mut in_codes, mut in_stale) = (0u64, 0u64, 0u64);
        let mut listed: Vec<String> = Vec::new();
        for b in bodies {
            let full = text_of(b.body_text.as_deref(), b.body_html.as_deref(), &b.preview, &b.subject);
            if full.chars().count() >= INDEX_BODY_MAX_CHARS { over_cap += 1; continue; }
            mails += 1;
            let have = reads.body_chunks(&b.message_id).unwrap_or_default();
            let now = pieces::body_pieces(tok.as_ref(), &full, &b.subject);
            let title = now.title.clone();
            pieces_n += have.len() as u64;
            cut += have.iter().filter(|(_, t, _)| !pieces::fits(tok.as_ref(), title.as_deref(), t)).count() as u64;
            // a word is held when every byte of it lies in the stored pieces (pieces::words_outside): a word the cutter split inside (a run with no
            // space) is held by the pieces that hold its parts; the count by whole words before round 20 called it lost
            let clean = pieces::clean_for_meaning(&full);
            // round 23: the stored pieces are today's cut of this text (current) or not (stale); a current mail's pieces are read as byte ranges of the
            // cut (pieces::piece_ranges, pieces::cut_ranges), never found again by their text, which a mail that repeats a passage holds more than once
            let kept: Vec<String> = now.texts.into_iter().filter(|t| !t.trim().is_empty()).collect();
            let current = kept.len() == have.len() && kept.iter().zip(&have).all(|(k, h)| *k == h.1);
            let (n, outside, outside_v) = if current {
                let ranges = pieces::piece_ranges(tok.as_ref(), title.as_deref(), &clean);
                // a mail of numbers keeps the subject as its one piece: then no range of the text is stored
                let ranges = if ranges.len() == have.len() { ranges } else { Vec::new() };
                let embedded: Vec<std::ops::Range<usize>> = ranges.iter().zip(&have).filter(|(_, h)| h.2).map(|(r, _)| r.clone()).collect();
                let (n, o) = pieces::words_outside_ranges(&clean, &ranges);
                (n, o, pieces::words_outside_ranges(&clean, &embedded).1)
            } else {
                let stored: Vec<&str> = have.iter().map(|c| c.1.as_str()).collect();
                let embedded: Vec<&str> = have.iter().filter(|c| c.2).map(|c| c.1.as_str()).collect();
                let (n, o) = pieces::words_outside(&clean, &stored);
                (n, o, pieces::words_outside(&clean, &embedded).1)
            };
            words += n as u64;
            no_piece += outside.len() as u64;
            no_vector += outside_v.len() as u64;
            if !outside.is_empty() {
                // the cause: the stored pieces are not today's cut of this text (stale), the cut itself leaves a word out (a cutter defect), or a piece of
                // the cut was left out as mostly codes (pieces_of's rule); a mail's words are counted under ONE cause: a "cut" mail's words outside the cut
                // are lost by the cut, the rest of its words in no piece are in code pieces (main112: row 32, 116 = 50 + 66)
                let whole = pieces::cut_ranges(tok.as_ref(), title.as_deref(), &clean, pieces::PIECE_TEXT_TOKENS, pieces::PIECE_OVERLAP_TOKENS);
                let (_, by_cut) = pieces::words_outside_ranges(&clean, &whole);
                let why = if !current { in_stale += outside.len() as u64; "stale" }
                    else if !by_cut.is_empty() { lost_by_cut += by_cut.len() as u64; in_codes += (outside.len() - by_cut.len().min(outside.len())) as u64; "cut" }
                    else { in_codes += outside.len() as u64; "code-piece" };
                if listed.len() < 40 { listed.push(format!("[{},{},\"{}\",{},{},{}]", b.rowid, outside.len(), why, clean.len(), have.len(), by_cut.len())); }
            }
            for f in self.store.attachments(&b.message_id).unwrap_or_default() {
                files += self.store.attachment_chunks(&f.id).unwrap_or_default().iter().filter(|c| c.2).count() as u64;
            }
        }
        // written by hand: the shipped engine does not link serde_json (a test-only library here), and every value is a number or a fixed word
        format!(
            "{{\"mails\":{mails},\"over_cap\":{over_cap},\"words\":{words},\"words_in_no_piece\":{no_piece},\"words_in_no_vector\":{no_vector},\"pieces\":{pieces_n},\"pieces_cut\":{cut},\"attachment_pieces_with_vector\":{files},\"words_lost_by_the_cut\":{lost_by_cut},\"words_in_code_pieces\":{in_codes},\"words_in_stale_pieces\":{in_stale},\"mails_with_words_in_no_piece\":[{}],\"last\":{},\"counter\":\"{}\"}}",
            listed.join(","),
            last.map(|v| v.to_string()).unwrap_or_else(|| "null".to_string()),
            if known { "model tokenizer" } else { "byte bound" }
        )
    }

    /// One step: these bodies into the word index, then into the meaning index (the ones it does not hold yet).
    fn step(&self, ids: Vec<String>, rows: Vec<i64>) -> Result<(), String> {
        let reads = self.store.background_reads().map_err(|e| format!("the store did not open a read: {e}"))?;
        let mut rowids = reads.rowids_of(&ids).map_err(|e| e.to_string())?;
        rowids.extend(rows);
        let bodies = reads.index_rows(&rowids).map_err(|e| e.to_string())?;
        if bodies.is_empty() { return Ok(()); }
        if let Some(t) = self.text_index() { t.reindex(bodies.iter().map(|b| b.message_id.clone()).collect()); }
        let Some(vector) = self.vector_index()? else {
            // stale-fix (item c1436f49): these mails cannot be cut now (no model, so no meaning index); the chunks they hold lose their
            // vector mark, so the next model hand-over's backfill queues them and this step cuts them again from today's text (`current`
            // decides). The next line's return (the model taken back between the two reads) is kept as it was: checks/proofs/g6/0c272b3c.json
            // breaks that line by its text.
            let queued: Vec<String> = bodies.iter().map(|b| b.message_id.clone()).collect();
            self.store.unembed_body_chunks(&queued).map_err(|e| e.to_string())?;
            return Ok(());
        };
        let Some(embedder) = lock(&self.embedder).clone() else { return Ok(()) };
        self.wait_for_room();
        let tok = self.counter();
        for b in bodies {
            if self.stopped() { return Ok(()); }
            let have = reads.body_chunks(&b.message_id).map_err(|e| e.to_string())?;
            let text = text_of(b.body_text.as_deref(), b.body_html.as_deref(), &b.preview, &b.subject);
            // pieces: cleaned for the meaning model and cut in its tokens; each piece with the subject (cut) fits the model's input
            let cut = pieces::body_pieces(tok.as_ref(), &text, &b.subject);
            let texts: Vec<String> = cut.texts.into_iter().filter(|t| !t.trim().is_empty()).collect();
            // round 3: chunks cut from an older text (a body rewritten after it was indexed, e.g. body-r2's repair) are cut again, so
            // "indexed" never stands for a text the index does not hold
            let current = have.len() == texts.len() && have.iter().zip(&texts).all(|((_, t, _), n)| t == n);
            // once: of a body cut from its current text, only the chunks without their vector (neither marked on disk nor held in memory
            // since the last commit) are embedded; none left = nothing to do
            let todo: Vec<(i64, String)> = if !current {
                let (ids, old) = self.store.replace_body_chunks(&b.message_id, texts.clone()).map_err(|e| e.to_string())?;
                for o in old { let _ = vector.remove_message(chunk_vector_key(o)); }
                ids.into_iter().zip(texts).collect()
            } else {
                self.mark_held(&have, &vector);
                have.into_iter().filter(|(id, _, marked)| !*marked && !vector.contains_message(chunk_vector_key(*id))).map(|(id, t, _)| (id, t)).collect()
            };
            if !todo.is_empty() { self.embed_pieces(&b.message_id, cut.title, todo, tok.as_ref(), embedder.as_ref(), &vector); }
            // pieces (SPEC A152): the files of the mail whose text is on this Mac, under "<subject> · <file name>"
            self.file_pieces(&b.message_id, &b.subject, tok.as_ref(), embedder.as_ref(), &vector);
        }
        Ok(())
    }

    /// stale-fix: of a body cut from its current text, a chunk without its mark whose vector the index holds (it lost its mark while no
    /// model was given, `step`; a chunk id is never reused, so the vector is of this text) is marked again at the next commit, never
    /// embedded twice. A chunk filed since the last commit is already waiting for its mark.
    fn mark_held(&self, have: &[(i64, String, bool)], vector: &VectorIndex) {
        let mut unmarked = lock(&self.unmarked);
        for (id, _, marked) in have {
            if !*marked && vector.contains_message(chunk_vector_key(*id)) && !unmarked.contains(id) { unmarked.push(*id); }
        }
    }

    /// pieces: the counter the pieces are sized with: the model's own tokenizer when its tokenizer.json is known (the shell's word, or
    /// `<whoosh folder>/Models/embeddinggemma-2/<revision>/tokenizer.json` beside the core's data folder), else the byte bound. Built
    /// on the first step after an idle time, dropped when the work runs out (`run`).
    fn counter(&self) -> Arc<dyn Tokenizer> {
        let mut slot = lock(&self.counter);
        if let Some(t) = slot.as_ref() { return t.clone(); }
        // the shell's word first (`set_tokenizer_file`, then the folder handed with the model), then the installer's folder
        let handed = || super::crawl::embedding_model_folder().and_then(|f| ModelTokenizer::file_in(std::path::Path::new(&f)));
        let file = lock(&self.tokenizer_file).clone().or_else(handed).or_else(|| installed_tokenizer(self.store.path()));
        let t: Arc<dyn Tokenizer> = match file.map(|f| ModelTokenizer::from_file(&f)) {
            Some(Ok(m)) => Arc::new(m) as Arc<dyn Tokenizer>,
            Some(Err(e)) => { self.note(&format!("{e}; the pieces are sized by the byte bound")); Arc::new(ByteBound) as Arc<dyn Tokenizer> }
            None => Arc::new(ByteBound) as Arc<dyn Tokenizer>,
        };
        *slot = Some(t.clone());
        t
    }

    /// pieces: embeds `todo` (chunk id, piece) of one mail or file under `title` and files each vector under its chunk's id. A piece
    /// that does not fit the model's input with its title (by the counter) is counted (`pieces_over_limit`) and named in one note.
    fn embed_pieces(&self, owner: &str, title: Option<String>, todo: Vec<(i64, String)>, tok: &dyn Tokenizer, embedder: &dyn Embedder, vector: &VectorIndex) {
        let over = todo.iter().filter(|(_, t)| !pieces::fits(tok, title.as_deref(), t)).count() as u64;
        if over > 0 {
            self.over_limit.fetch_add(over, Ordering::SeqCst);
            self.note(&format!("{owner}: {over} pieces are over the model's input of {} tokens", pieces::MODEL_INPUT_TOKENS));
        }
        let vectors = match embedder.embed_titled(title, todo.iter().map(|(_, t)| t.clone()).collect()) {
            Ok(v) if v.len() == todo.len() => v,
            Ok(v) => { self.note(&format!("{owner}: the model returned {} vectors for {} chunks", v.len(), todo.len())); return; }
            Err(e) => { self.note(&format!("{owner} was not embedded: {e}")); return; }
        };
        for ((id, _), v) in todo.iter().zip(vectors) {
            match vector.index_vector(chunk_vector_key(*id), &v) {
                Ok(()) => lock(&self.unmarked).push(*id),
                Err(e) => self.note(&format!("chunk {id} of {owner} was not filed: {e}")),
            }
        }
    }

    /// pieces (SPEC A152): each file of the mail with a text on this Mac (its read text, picture words and speech) is cut by the same
    /// rule and embedded under "<subject> · <file name>"; pieces cut before by another rule (the download's byte bound, the old word
    /// windows) are cut again, their vectors removed.
    fn file_pieces(&self, message_id: &str, subject: &str, tok: &dyn Tokenizer, embedder: &dyn Embedder, vector: &VectorIndex) {
        let files = match self.store.attachments(message_id) { Ok(f) => f, Err(e) => { self.note(&format!("the files of {message_id} were not read: {e}")); return; } };
        for f in files {
            if self.stopped() { return; }
            let Ok(Some(content)) = self.store.attachment_content(&f.id) else { continue };
            let text = [content.text, content.ocr_text, content.transcript].into_iter().flatten().filter(|t| !t.trim().is_empty()).collect::<Vec<_>>().join("\n");
            if text.is_empty() { continue; }
            // the ledger's cap on one file's text (ATTACHMENT_TEXT_MAX_CHARS), as the download keeps it
            let text: String = match attachment_index::constants::ATTACHMENT_TEXT_MAX_CHARS { Some(m) if text.chars().count() > m => text.chars().take(m).collect(), _ => text };
            let cut = pieces::attachment_pieces(tok, &text, subject, &f.file_name);
            let texts: Vec<String> = cut.texts.into_iter().filter(|t| !t.trim().is_empty()).collect();
            let Ok(have) = self.store.attachment_chunks(&f.id) else { continue };
            let current = have.len() == texts.len() && have.iter().zip(&texts).all(|((_, t, _), n)| t == n);
            let todo: Vec<(i64, String)> = if !current {
                let Ok((ids, old)) = self.store.replace_attachment_chunks(&f.id, texts.clone()) else { self.note(&format!("the pieces of file {} were not filed", f.id)); continue };
                for o in old { let _ = vector.remove_message(chunk_vector_key(o)); }
                ids.into_iter().zip(texts).collect()
            } else {
                have.into_iter().filter(|(id, _, marked)| !*marked && !vector.contains_message(chunk_vector_key(*id))).map(|(id, t, _)| (id, t)).collect()
            };
            if !todo.is_empty() { self.embed_pieces(&format!("{message_id} file {}", f.id), cut.title, todo, tok, embedder, vector); }
        }
    }

    /// No step starts while the footprint is at EMBED_INDEX_CEILING_MB: it waits EMBED_PAUSE_MS and reads it again.
    /// A wait writes one line when it starts and one when it ends (item b43515ba), counts only.
    fn wait_for_room(&self) {
        let mut waited = false;
        while let Some(mb) = footprint_mb().filter(|mb| *mb >= EMBED_INDEX_CEILING_MB) {
            if self.stopped() { return; }
            if !waited {
                waited = true;
                // round 25 (main112 crawl form: "indexed 0 of 100" for 300 s with the model loaded and the footprint at 1822 to 1897 MB): the vectors the
                // steps before this wait filed are written and their chunks marked now, so the work done counts while the memory holds the next step back
                if let Some(v) = lock(&self.vector).clone() { self.commit_and_mark(&v); }
                self.note(&format!("waiting for memory: footprint {mb} MB, ceiling {EMBED_INDEX_CEILING_MB} MB; {} mails and rows wait", self.waiting()));
            }
            self.rest(Duration::from_millis(EMBED_PAUSE_MS));
        }
        if waited { self.note(&format!("memory allows a step again: footprint {} MB", footprint_mb().unwrap_or(0))); }
    }

    /// Writes the meaning index when the work ran out or VECTOR_COMMIT_S passed; folds its memory into the mapped base past
    /// VECTOR_DELTA_MAX_CHUNKS.
    fn keep_vectors(&self, last_commit: &mut Instant) {
        let Some(vector) = lock(&self.vector).clone() else { return };
        if vector.delta_chunks() > VECTOR_DELTA_MAX_CHUNKS {
            let Ok(reads) = self.store.background_reads() else { return };
            // what stays: the chunks marked embedded and the ones filed since the last commit; a vector whose chunk is gone is dropped
            let mut live: Vec<u64> = lock(&self.unmarked).iter().map(|id| chunk_vector_key(*id)).collect();
            let mut after = 0i64;
            while let Ok(page) = reads.embedded_chunks_after(after, INDEX_SCAN_ROWS) {
                let Some(last) = page.last() else { break };
                after = *last;
                live.extend(page.into_iter().map(chunk_vector_key));
            }
            match vector.compact(live) {
                // the fold wrote a new base: the vectors filed since the last commit are on disk now
                Ok(()) => { let ids = std::mem::take(&mut *lock(&self.unmarked)); if !ids.is_empty() { let _ = self.store.mark_chunks_embedded(ids, now_ms()); } }
                Err(e) => self.note(&format!("the meaning index was not folded: {e}")),
            }
            *last_commit = Instant::now();
            return;
        }
        let idle = { let w = lock(&self.work); w.ids.is_empty() && w.rows.is_empty() };
        if idle || last_commit.elapsed() >= Duration::from_secs(VECTOR_COMMIT_S) {
            self.commit_and_mark(&vector);
            *last_commit = Instant::now();
        }
    }
}


/// The JSON object `{"error": "<words>"}` of `Indexer::piece_counts`, written by hand (see there); the words are the store's own, never mail text.
fn piece_counts_error(words: &str) -> String {
    let mut out = String::with_capacity(words.len() + 12);
    for c in words.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    format!("{{\"error\":\"{out}\"}}")
}
