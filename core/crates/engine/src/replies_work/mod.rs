//! reply-engine (docs/lanes/reply-engine.md): the working commands of AI replies in the real core, behind the settings lane's one port `ReplyWork`
//! (engine/src/replies.rs). Every command returns at once ("The objective is <50ms for any operation"): it checks what it can check locally (a model
//! is set, the thread exists, no analysis runs) and hands the work to one of two threads, which report by event:
//!   - the INTERACTIVE thread: a draft on demand, a change request, "Essayer sur un message" (the owner waits on these);
//!   - the BACKGROUND thread: the auto use's pass on arrivals, the voice analysis, the sync of the folders of this Mac. It waits while the
//!     background is paused (`Pause`).
//! Nothing is ever sent: a draft is a row of `reply_draft`, and Accept makes an ordinary composer draft through the SaveDraft path.
pub mod adapters;
pub mod limits;
pub mod model;
mod usecases;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::ai_settings::AiJob;
use contracts::{CommandOutcome, CoreError, ReplySourceRef, ReplyTryReport, ReplyUseKind, ReplyVoiceAnalysis};
use store::Store;

pub use adapters::memory_excerpts::MemoryExcerpts;
pub use adapters::stored_excerpts::StoredExcerpts;
pub use adapters::provider::ProviderModel;
pub use adapters::provider::Completer;
pub use model::{set_reply_model, CallKind, ModelCall, TextModel, REPLY_VOICE_RUNNING_CODE};

use crate::bodies::Bodies;
use crate::replies::{install_reply_work, ReplyJob, ReplyWork};
use crate::rust_core::Keyring;
use crate::sync::Sinks;

/// Whether the background is paused now (battery under its floor, heat, the owner acting: the crawl's pause inputs).
pub trait Pause: Send + Sync { fn held(&self) -> bool; }

/// The crawl's pause (sync-full-r1 round 2, `Bodies::paused`: held, battery, heat, the owner acting). Only the BACKGROUND thread waits on it: a draft the
/// owner asks for (⌘J, a change, a try) never does.
struct CrawlPause(std::sync::Weak<Bodies>);
impl Pause for CrawlPause { fn held(&self) -> bool { self.0.upgrade().is_some_and(|b| b.paused().is_some()) } }

fn pauses() -> &'static Mutex<HashMap<PathBuf, Arc<dyn Pause>>> {
    static SET: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn Pause>>>> = OnceLock::new();
    SET.get_or_init(Default::default)
}

/// Sets (Some) or clears (None) the pause the background work of the core at `store_path` obeys, in place of the crawl's (tests, a shell).
pub fn set_reply_pause(store_path: &Path, pause: Option<Arc<dyn Pause>>) {
    let mut g = pauses().lock().unwrap_or_else(|e| e.into_inner());
    match pause { Some(p) => { g.insert(store_path.to_path_buf(), p); } None => { g.remove(store_path); } }
}

/// A pause that holds until released: what `hold_background` sets.
struct Held(AtomicBool);
impl Pause for Held { fn held(&self) -> bool { self.0.load(Ordering::SeqCst) } }

/// FOR TESTS THAT SEED A SOURCE'S COUNT OR PROGRESS BY HAND (engine_replies_settings.rs): holds (true) or releases (false) the background work of the
/// core at `store_path` (the arrival pass, the voice analysis, the walk of folders and calendars, the mail folders' figures), so nothing rewrites what the
/// test seeded. No command and no state path reaches it (only a Rust caller holding the store's path); production never calls it. The opposite case,
/// a hand-seeded count replaced by the real figure when nothing holds, is `engine_reply_work.rs` `without_the_hold_a_hand_seeded_count_is_replaced_by_the_real_figure`.
pub fn hold_background(store_path: &Path, held: bool) {
    set_reply_pause(store_path, if held { Some(Arc::new(Held(AtomicBool::new(true)))) } else { None });
}

pub(crate) enum Job {
    Draft { draft_id: String, thread_id: String, instruction: Option<String>, model: Arc<dyn TextModel> },
    Change { draft: contracts::ReplyDraft, text: String, model: Arc<dyn TextModel> },
    Try { run_id: String, message_id: String, kind: ReplyUseKind, model: Arc<dyn TextModel> },
    Arrived { ids: Vec<String> },
    Voice { sample_size: u32, model: Arc<dyn TextModel> },
    Sources,
    /// tests: answered once every job queued before it on the same thread ran
    Flush(Sender<()>),
}

/// What the work of one core holds.
pub(crate) struct Shared {
    pub store: Arc<Store>,
    pub sinks: Sinks,
    pub bodies: std::sync::Weak<Bodies>,
    pub keychain: Arc<dyn Keyring>,
    /// the composer's SaveDraft path (actions crate), for Accept
    pub env: actions::Env,
    /// the excerpts of folders of this Mac and of events: kept in the local database (`source_chunk`), read back after a relaunch with no walk
    pub memory: StoredExcerpts,
    pub voice: Mutex<ReplyVoiceAnalysis>,
    pub tries: Mutex<VecDeque<ReplyTryReport>>,
    /// the folders of this Mac walked in this launch
    pub synced: Mutex<HashSet<ReplySourceRef>>,
    /// bumped by every write to `reply_source` (a folder's walk re-reads its switch only then)
    pub sources_epoch: AtomicU64,
    /// bumped by every write to the calendars the shell pushed; with the enabled calendars, what the calendars' excerpts were last built from
    pub calendars_epoch: AtomicU64,
    pub calendars_built: Mutex<Option<(u64, Vec<String>)>>,
    sources_queued: AtomicBool,
    interactive: Mutex<Sender<Job>>,
    background: Mutex<Sender<Job>>,
}

impl Shared {
    pub(crate) fn log(&self, line: &str) {
        eprintln!("replies: {line}");
        crate::log::log_line(&crate::log::path_beside(self.store.path()), &format!("replies: {line}"));
    }

    pub(crate) fn pause(&self) -> Arc<dyn Pause> {
        pauses().lock().unwrap_or_else(|e| e.into_inner()).get(self.store.path()).cloned().unwrap_or_else(|| Arc::new(CrawlPause(self.bodies.clone())))
    }

    /// Waits while the background is paused.
    pub(crate) fn wait_unpaused(&self) {
        let pause = self.pause();
        while pause.held() { std::thread::sleep(Duration::from_millis(limits::REPLY_PAUSE_RECHECK_MS)); }
    }

    /// The model for `job` (`ai_settings::model_for`), or its refusal in words (`ai_no_model`).
    pub(crate) fn model(&self, job: AiJob) -> Result<Arc<dyn TextModel>, CoreError> {
        model::model_for(&self.store, job, &|s, j| adapters::provider::configured(s, &self.keychain, j))
    }

    fn send(&self, interactive: bool, job: Job) {
        let tx = if interactive { &self.interactive } else { &self.background };
        if tx.lock().unwrap_or_else(|e| e.into_inner()).send(job).is_err() { self.log("a reply thread is gone; the work was not started"); }
    }

    fn sources_changed(&self) {
        self.sources_epoch.fetch_add(1, Ordering::SeqCst);
        if !self.sources_queued.swap(true, Ordering::SeqCst) { self.send(false, Job::Sources); }
    }

    fn run(self: &Arc<Self>, job: Job) {
        match job {
            Job::Draft { draft_id, thread_id, instruction, model } => usecases::draft_on_demand::run(self, &draft_id, &thread_id, instruction.as_deref(), &*model),
            Job::Change { draft, text, model } => usecases::request_change::run(self, draft, &text, &*model),
            Job::Try { run_id, message_id, kind, model } => usecases::try_reply::run(self, &run_id, &message_id, kind, &*model),
            Job::Arrived { ids } => { self.wait_unpaused(); usecases::arrival_pass::run(self, &ids) }
            Job::Voice { sample_size, model } => { self.wait_unpaused(); usecases::voice_analysis::run(self, sample_size, &*model) }
            Job::Sources => { self.sources_queued.store(false, Ordering::SeqCst); self.wait_unpaused(); usecases::sync_sources::run(self) }
            Job::Flush(done) => { let _ = done.send(()); }
        }
    }
}

fn worker(shared: std::sync::Weak<Shared>, name: &str) -> Option<Sender<Job>> {
    let (tx, rx) = channel::<Job>();
    let spawned = std::thread::Builder::new().name(name.into()).spawn(move || {
        while let Ok(job) = rx.recv() {
            let Some(s) = shared.upgrade() else { return };
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.run(job))).is_err() { s.log("a reply job stopped unexpectedly; the next one runs"); }
        }
    });
    match spawned { Ok(_) => Some(tx), Err(e) => { eprintln!("replies: the {name} thread could not start: {e}"); None } }
}

fn works() -> &'static Mutex<HashMap<PathBuf, Arc<Shared>>> {
    static SET: OnceLock<Mutex<HashMap<PathBuf, Arc<Shared>>>> = OnceLock::new();
    SET.get_or_init(Default::default)
}

/// The arrival hook (filing_worker.rs, after the label flow filed what the sync stored): the auto use looks at these messages on the background thread.
/// Returns at once.
pub(crate) fn arrived(store: &Store, ids: &[String]) {
    if ids.is_empty() { return; }
    if let Some(s) = works().lock().unwrap_or_else(|e| e.into_inner()).get(store.path()).cloned() {
        s.send(false, Job::Arrived { ids: ids.to_vec() });
        // new mail moves the mail folders' counts on the Sources pane
        s.sources_changed();
    }
}

fn work_at(store_path: &Path) -> Option<Arc<Shared>> { works().lock().unwrap_or_else(|e| e.into_inner()).get(store_path).cloned() }

/// Tests and the hidden check: waits until both reply threads ran every job queued so far (true), or `within` passed (false: a job still runs, or the
/// background is paused).
pub fn settle(store_path: &Path, within: Duration) -> bool {
    let Some(s) = work_at(store_path) else { return true };
    let t0 = std::time::Instant::now();
    for interactive in [true, false] {
        let (tx, rx) = channel();
        s.send(interactive, Job::Flush(tx));
        if rx.recv_timeout(within.saturating_sub(t0.elapsed())).is_err() { return false; }
    }
    true
}

/// Tests: how many excerpts the in-memory index holds for a folder of this Mac (or a calendar).
pub fn indexed_excerpts(store_path: &Path, source: &ReplySourceRef) -> usize { work_at(store_path).map(|s| s.memory.excerpt_count(source)).unwrap_or(0) }

/// Starts the draft engine of a core and plugs it into `ReplyWork` (rust_core.rs `assemble`, one line). The folders of this Mac switched on are walked
/// at once in the background; every later change of the Sources pane is seen in the store's change feed.
pub(crate) fn install(store: &Arc<Store>, sinks: &Sinks, bodies: &Arc<Bodies>, keychain: Arc<dyn Keyring>, env: actions::Env) {
    let shared = Arc::new_cyclic(|me: &std::sync::Weak<Shared>| {
        let dead = || channel::<Job>().0;
        Shared {
            store: store.clone(), sinks: sinks.clone(), bodies: Arc::downgrade(bodies), keychain, env,
            memory: StoredExcerpts::new(store.clone()), voice: Mutex::new(ReplyVoiceAnalysis::Idle), tries: Mutex::new(VecDeque::new()), synced: Mutex::new(HashSet::new()),
            sources_epoch: AtomicU64::new(0), sources_queued: AtomicBool::new(false), calendars_epoch: AtomicU64::new(0), calendars_built: Mutex::new(None),
            interactive: Mutex::new(worker(me.clone(), "whoosh-replies").unwrap_or_else(dead)),
            background: Mutex::new(worker(me.clone(), "whoosh-replies-bg").unwrap_or_else(dead)),
        }
    });
    let watch = Arc::downgrade(&shared);
    store.subscribe_tables(Arc::new(move |c: &store::TableChange| {
        let calendars = c.table == "calendar_event" || c.table == "reply_calendar";
        if c.table != "reply_source" && !calendars { return; }
        if let Some(s) = watch.upgrade() {
            if calendars { s.calendars_epoch.fetch_add(1, Ordering::SeqCst); }
            s.sources_changed();
        }
    }));
    shared.sources_changed();
    works().lock().unwrap_or_else(|e| e.into_inner()).insert(store.path().to_path_buf(), shared.clone());
    install_reply_work(store.path(), Arc::new(Work(shared)));
}

/// The work plugged into the settings lane's port.
struct Work(Arc<Shared>);

fn created(id: String) -> CommandOutcome { CommandOutcome { undo_entry_id: None, created_id: Some(id) } }
const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

impl ReplyWork for Work {
    fn execute(&self, store: &Store, job: ReplyJob) -> Result<CommandOutcome, CoreError> {
        let s = &self.0;
        match job {
            ReplyJob::DraftOnDemand { thread_id, instruction } => {
                let model = s.model(AiJob::Draft)?;
                if store.thread(&thread_id).map_err(CoreError::from)?.is_none() { return Err(CoreError::NotFound { kind: "thread".into(), id: thread_id }); }
                let draft_id = store::new_id("reply");
                s.send(true, Job::Draft { draft_id: draft_id.clone(), thread_id, instruction, model });
                Ok(created(draft_id))
            }
            ReplyJob::RequestChange { draft, text } => {
                let model = s.model(AiJob::Draft)?;
                s.send(true, Job::Change { draft, text, model });
                Ok(OK)
            }
            ReplyJob::TryReply { message_id, kind } => {
                let model = s.model(AiJob::Draft)?;
                let run_id = store::new_id("try");
                s.send(true, Job::Try { run_id: run_id.clone(), message_id, kind, model });
                Ok(created(run_id))
            }
            ReplyJob::RunVoiceAnalysis { sample_size } => {
                let model = s.model(AiJob::VoiceAnalysis)?;
                {
                    let mut v = s.voice.lock().unwrap_or_else(|e| e.into_inner());
                    if matches!(*v, ReplyVoiceAnalysis::Running { .. }) {
                        return Err(crate::sign_in::codes::refusal("An analysis of your sent mail is already running: wait for it to finish.", REPLY_VOICE_RUNNING_CODE, ""));
                    }
                    *v = ReplyVoiceAnalysis::Running { read: 0, of: 0 };
                }
                s.send(false, Job::Voice { sample_size, model });
                Ok(OK)
            }
            ReplyJob::Accept { draft } => usecases::accept_draft::run(s, draft).map(created),
            ReplyJob::Discard { draft } => usecases::discard_draft::run(s, draft).map(|_| OK),
        }
    }

    fn voice_analysis(&self) -> ReplyVoiceAnalysis { self.0.voice.lock().unwrap_or_else(|e| e.into_inner()).clone() }

    fn try_report(&self, run_id: &str) -> Option<ReplyTryReport> {
        self.0.tries.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|t| t.run_id == run_id).cloned()
    }
}
