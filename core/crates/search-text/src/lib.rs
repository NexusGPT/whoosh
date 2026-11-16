//! Whoosh full-text index: tantivy (MIT) over every message's subject, people, body and extracted
//! attachment text, kept current by the store's change feed.
//!
//! - `TextIndex::open(store, folder)` loads the index in `folder`, or builds it from the message table when
//!   the folder is missing or unreadable (blocking; call it off the main thread).
//! - The store's table feed (`Store::subscribe_tables`) wakes one worker thread. It indexes new messages and
//!   attachments, drops deleted ones, and commits; `search` sees the commit at once.
//! - `search(text, limit)`: bare words must all appear; a quoted phrase is an exact phrase.
//!
//! Third-party code: tantivy (MIT) and rusqlite (MIT, bundled SQLite public domain). Nothing is copied.

mod engine;
mod error;
pub mod limits;
mod query;
mod schema;
#[cfg(test)]
mod wake_tests;
pub mod source;

pub use error::Error;
pub use query::HitFacts;

use engine::{Engine, Watermarks};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use store::{Store, TableChange};
use tantivy::{Index, ReloadPolicy};

pub use contracts::SearchHit;

const WAKE_INSERT: u8 = 1;
const WAKE_UPDATE: u8 = 2;
const WAKE_DELETE: u8 = 4;
const WAKE_REPAIR: u8 = 8;
/// sync-full-r1: messages named by `reindex` (a body kept by an update the change feed cannot name).
const WAKE_IDS: u8 = 16;
const WAKE_STOP: u8 = 128;

struct Wake {
    flags: Mutex<u8>,
    cv: Condvar,
}

impl Wake {
    fn raise(&self, flag: u8) {
        *self.flags.lock().unwrap_or_else(|e| e.into_inner()) |= flag;
        self.cv.notify_all();
    }
    /// Flags raised since the last take, without waiting.
    fn take_now(&self) -> u8 {
        std::mem::take(&mut *self.flags.lock().unwrap_or_else(|e| e.into_inner()))
    }
    fn take(&self) -> u8 {
        let mut g = self.flags.lock().unwrap_or_else(|e| e.into_inner());
        while *g == 0 {
            g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner());
        }
        std::mem::take(&mut *g)
    }
}

/// Shared between the handle and the worker.
struct Shared {
    index: Index,
    fields: schema::Fields,
    reader: tantivy::IndexReader,
    snippets: Mutex<rusqlite::Connection>,
    last_error: Mutex<Option<String>>,
    commits: AtomicU64,
    /// sync-full-r1: the ids `reindex` was given that the worker has not indexed yet.
    pending: Mutex<Vec<String>>,
    /// sync-full-r1: a step is running.
    busy: std::sync::atomic::AtomicBool,
}

pub struct TextIndex {
    shared: Arc<Shared>,
    wake: Arc<Wake>,
    worker: Mutex<Option<JoinHandle<()>>>,
    store: Arc<Store>,
    subscription: u64,
    folder: PathBuf,
    rebuilt: bool,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Load the index in `folder` when it is whole and current; otherwise None.
fn open_existing(folder: &Path, fields_schema: &tantivy::schema::Schema) -> Option<(Index, Watermarks)> {
    let index = Index::open_in_dir(folder).ok()?;
    if index.schema() != *fields_schema {
        return None;
    }
    schema::register_analyzer(&index);
    let payload = index.load_metas().ok()?.payload?;
    let marks = Watermarks::from_payload(&payload)?;
    Some((index, marks))
}

impl TextIndex {
    pub fn open(store: &Arc<Store>, folder: &Path) -> Result<TextIndex, Error> {
        TextIndex::open_with(store, folder, limits::WRITER_HEAP_BYTES, limits::WRITER_THREADS, false)
    }

    /// sync-full-r1: `open` with the writer's memory and threads of the caller's choosing (the real core's background indexer runs
    /// one thread on a small heap, so the word index never weighs on the app). `background`: the worker thread runs at the OS's
    /// background priority (macOS QOS_CLASS_BACKGROUND), so its commits never take CPU from the app's own threads.
    pub fn open_with(store: &Arc<Store>, folder: &Path, heap_bytes: usize, threads: usize, background: bool) -> Result<TextIndex, Error> {
        let (schema, fields) = schema::build();
        let (index, marks, rebuilt) = match open_existing(folder, &schema) {
            Some((index, marks)) => (index, marks, false),
            None => {
                if folder.exists() {
                    std::fs::remove_dir_all(folder)?;
                }
                std::fs::create_dir_all(folder)?;
                let index = Index::create_in_dir(folder, schema.clone())?;
                schema::register_analyzer(&index);
                (index, Watermarks::default(), true)
            }
        };
        let reader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into()?;
        let mut engine = Engine::new_with(&index, fields, reader.clone(), source::open_reader(store.path())?, marks, threads, heap_bytes)?;
        if rebuilt {
            engine.rebuild()?;
            let documents = engine.documents() as i64;
            let folder_text = folder.to_string_lossy().to_string();
            store.write(move |ctx| {
                ctx.tx.execute(
                    "INSERT INTO index_manifest (index_kind, folder, documents, built_at_ms) VALUES ('text', ?1, ?2, ?3) \
                     ON CONFLICT(index_kind) DO UPDATE SET folder = excluded.folder, documents = excluded.documents, built_at_ms = excluded.built_at_ms",
                    rusqlite::params![folder_text, documents, now_ms()],
                )?;
                Ok(())
            })?;
        }
        let commits_at_open = engine.commits;
        let shared = Arc::new(Shared {
            index,
            fields,
            reader,
            snippets: Mutex::new(source::open_reader(store.path())?),
            last_error: Mutex::new(None),
            commits: AtomicU64::new(commits_at_open),
            pending: Mutex::new(Vec::new()),
            busy: std::sync::atomic::AtomicBool::new(false),
        });
        let wake = Arc::new(Wake { flags: Mutex::new(0), cv: Condvar::new() });
        // Subscribe before the first catch-up so a write that lands in between is not missed.
        let sink_wake = wake.clone();
        let subscription = store.subscribe_tables(Arc::new(move |c: &TableChange| {
            if c.table != "message" && c.table != "attachment" {
                return;
            }
            let mut flags = 0;
            if c.inserts > 0 {
                flags |= WAKE_INSERT;
            }
            if c.updates > 0 {
                flags |= WAKE_UPDATE;
            }
            if c.deletes > 0 {
                flags |= WAKE_DELETE;
            }
            sink_wake.raise(flags);
        }));
        wake.raise(if rebuilt { WAKE_INSERT } else { WAKE_INSERT | WAKE_REPAIR });
        let worker = {
            let (shared, wake) = (shared.clone(), wake.clone());
            std::thread::Builder::new().name("whoosh-text-index".into()).spawn(move || { if background { background_priority(); } work(engine, shared, wake) }).map_err(|e| Error::Io(e.to_string()))?
        };
        Ok(TextIndex { shared, wake, worker: Mutex::new(Some(worker)), store: store.clone(), subscription, folder: folder.to_path_buf(), rebuilt })
    }

    /// Hits for `text`, best first. A quoted phrase matches only that exact run of words.
    pub fn search(&self, text: &str, limit: usize) -> Result<Vec<SearchHit>, Error> {
        let body_of = |id: &str| {
            let conn = self.shared.snippets.lock().unwrap_or_else(|e| e.into_inner());
            source::body_of(&conn, id).ok().flatten()
        };
        query::run(&self.shared.index, &self.shared.fields, &self.shared.reader, text, limit, &body_of)
    }

    /// match-inside (SPEC A149): the owner's search. A bare typed word of `limits::SUBSTRING_MIN_CHARS` letters or more matches
    /// every mail holding those letters, also inside a longer word ("consultant" finds "consultants" and brightconsultants.example);
    /// a shorter word matches whole words only; every typed word must be held; a quoted phrase is exact. Best first: the word
    /// itself, a word that starts with it, a word that holds it; the subject or the sender before the text. Each hit says where it
    /// matched (subject, sender, attachment name or text, the text) with the letters to show, and its snippet is the passage of the
    /// text around the first occurrence. A word hit is never `MeaningOnly`.
    pub fn search_ranked(&self, text: &str, limit: usize) -> Result<Vec<SearchHit>, Error> {
        let facts_of = |id: &str, with_text: bool| {
            let conn = self.shared.snippets.lock().unwrap_or_else(|e| e.into_inner());
            source::hit_facts(&conn, id, with_text).ok().flatten()
        };
        query::run_ranked(&self.shared.index, &self.shared.fields, &self.shared.reader, text, limit, &facts_of)
    }

    /// Documents in the committed index.
    pub fn documents(&self) -> u64 {
        self.shared.reader.searcher().num_docs()
    }

    /// Commits the worker has made since open (a test and status aid).
    pub fn commits(&self) -> u64 {
        self.shared.commits.load(Ordering::Relaxed)
    }

    /// The last error the worker hit, if it has not been cleared by a later success.
    pub fn last_error(&self) -> Option<String> {
        self.shared.last_error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// True when `open` had to build the index from the message table.
    pub fn was_rebuilt(&self) -> bool {
        self.rebuilt
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// sync-full-r1: index these messages again from the table now (their body was kept by an update, which the change feed
    /// reports by table only). Returns at once; the worker commits them with its next step.
    pub fn reindex(&self, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }
        self.shared.pending.lock().unwrap_or_else(|e| e.into_inner()).extend(ids);
        self.wake.raise(WAKE_IDS);
    }

    /// sync-full-r1: ids given to `reindex` and not committed yet.
    pub fn pending(&self) -> usize {
        self.shared.pending.lock().unwrap_or_else(|e| e.into_inner()).len() + usize::from(self.shared.busy.load(Ordering::SeqCst))
    }

    /// Ask the worker to compare the whole table with the index (the slow, exact path).
    pub fn request_repair(&self) {
        self.wake.raise(WAKE_REPAIR);
    }
}

impl Drop for TextIndex {
    fn drop(&mut self) {
        self.store.unsubscribe(self.subscription);
        self.wake.raise(WAKE_STOP);
        if let Some(h) = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = h.join();
        }
    }
}

/// The ids `reindex` was given, indexed again from the table and committed.
fn reindex_pending(engine: &mut Engine, shared: &Shared) -> Result<(), Error> {
    let ids = std::mem::take(&mut *shared.pending.lock().unwrap_or_else(|e| e.into_inner()));
    if ids.is_empty() {
        return Ok(());
    }
    engine.reindex_ids(&ids)?;
    engine.commit()
}

fn step(engine: &mut Engine, shared: &Shared, wake: &Wake, flags: u8) -> Result<(), Error> {
    engine.catch_up(true)?;
    engine.commit()?;
    if flags & WAKE_IDS != 0 {
        reindex_pending(engine, shared)?;
    }
    if flags & (WAKE_DELETE | WAKE_REPAIR) != 0 {
        // The exact comparison, one window at a time; new mail and late attachments are served between windows.
        let mut r = engine.reconcile_start(0)?;
        while !r.done {
            engine.reconcile_window(&mut r)?;
            // A WAKE_IDS taken here is served here: taken and not served, it was a wake-up lost for good (indexer-race: a `reindex`
            // that landed during the comparison left its ids pending with no flag, so `pending()` never reached 0).
            let more = wake.take_now();
            if more & WAKE_STOP != 0 {
                wake.raise(WAKE_STOP);
                return Ok(());
            }
            if more != 0 {
                engine.catch_up(true)?;
                engine.commit()?;
                if more & WAKE_IDS != 0 { reindex_pending(engine, shared)?; }
                if more & (WAKE_DELETE | WAKE_UPDATE) != 0 {
                    // Rows changed under the comparison: restart it so nothing is judged on a stale snapshot.
                    r = engine.reconcile_start(0)?;
                }
            }
        }
    } else if flags & WAKE_UPDATE != 0 {
        engine.recheck_tail()?;
    }
    Ok(())
}

/// sync-full-r1: the calling thread at QOS_CLASS_BACKGROUND (macOS); nothing elsewhere.
#[cfg(target_os = "macos")]
fn background_priority() {
    extern "C" {
        fn pthread_set_qos_class_self_np(qos_class: u32, relative_priority: i32) -> i32;
    }
    const QOS_CLASS_BACKGROUND: u32 = 0x09;
    // SAFETY: sets the calling thread's own QoS class; no pointer is passed.
    let _ = unsafe { pthread_set_qos_class_self_np(QOS_CLASS_BACKGROUND, 0) };
}
#[cfg(not(target_os = "macos"))]
fn background_priority() {}

fn work(mut engine: Engine, shared: Arc<Shared>, wake: Arc<Wake>) {
    loop {
        let flags = wake.take();
        if flags & WAKE_STOP != 0 {
            return;
        }
        shared.busy.store(true, Ordering::SeqCst);
        let done = step(&mut engine, &shared, &wake, flags);
        shared.busy.store(false, Ordering::SeqCst);
        match done {
            Ok(()) => {
                *shared.last_error.lock().unwrap_or_else(|e| e.into_inner()) = None;
                shared.commits.store(engine.commits, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("[search-text] index step failed: {e}");
                *shared.last_error.lock().unwrap_or_else(|e2| e2.into_inner()) = Some(e.to_string());
            }
        }
    }
}
