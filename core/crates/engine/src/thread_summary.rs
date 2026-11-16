//! lane fix-aicore, part A: the reader's thread summary in the real engine (the reader's `mac-main.reader.summary`, ReaderSummary.swift).
//!
//! - Served at `thread/<id>/summary`: the kept summary as a JSON string, or null when none is kept. The read is ONE indexed read of the store
//!   (`Store::thread_summary`): it never calls a model and never waits for one ("The objective is <50ms for any operation").
//! - Made on first open, then kept (SPEC is silent on when; the brief's rule): the reader's read of the path is the open. When no summary is
//!   kept, or a newer message joined the thread since it was made (`ThreadSummaryRow::is_stale`), the read hands the thread to ONE background
//!   worker per store and answers what is kept now (null, or the previous summary); the next read answers the new one.
//! - Made by the owner's WRITING MODEL (Settings, AI and agents: the Writer's provider and model, `ai_settings::model_for(AiJob::Draft)`), through
//!   the reply engine's own model port (`replies_work::ProviderModel` over the same `ModelChoice`, the same HTTPS client `ProviderHttps`): no new
//!   network path. A hosted provider is asked only with its key connected (`model_for`) AND the owner's "Allow" for it (`agent_consent/<id>`,
//!   `Store::agent_consent_at`); a model on this Mac needs neither, and takes its turn at the on-device gate the label flow and the junk filter
//!   share (`decision_model::ModelGate`), when the shell handed one. No writing model: nothing is asked and the path answers null (the reader draws
//!   its missing-setup dot).
//! - Kept on this Mac: migration 0030's `thread.summary_text` and `thread.summary_of` (store/src/thread_summary.rs). A mail's text never reaches a
//!   log line: the log names the thread id and the model's failure words only.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use store::thread_summary::SummaryMessage;
use store::Store;

use crate::ai_settings::{model_for, AiJob};
use crate::decision_model::ModelGate;
use crate::replies_work::{CallKind, ModelCall, ProviderModel, TextModel};
use crate::rust_core::Keyring;

/// The newest messages of a thread a summary reads. PROPOSED for docs/CONSTANTS.md (named here once until the ledger holds the row).
pub const SUMMARY_MESSAGES: usize = 20;
/// The characters of one message's text a summary's prompt carries (the prompt as a whole is cut again to the model's room by `ProviderModel`).
/// PROPOSED for docs/CONSTANTS.md.
pub const SUMMARY_MESSAGE_CHARS: usize = 4_000;

const SYSTEM: &str = "You summarise an email thread for the person reading it, in at most two short sentences: what is asked or offered, \
the figures and dates that matter, and what the reader must do and by when. Write in the language of the thread. Answer with the summary only.";

enum Job {
    Make { thread_id: String, keychain: Arc<dyn Keyring>, gate: Option<Arc<ModelGate>> },
    /// tests: answered once every job queued before it ran
    Flush(Sender<()>),
}

struct Worker {
    store: Weak<Store>,
    tx: Mutex<Sender<Job>>,
    /// the threads queued and not made yet: a second read while one waits queues nothing
    queued: Mutex<HashSet<String>>,
}

fn workers() -> &'static Mutex<HashMap<PathBuf, Arc<Worker>>> {
    static SET: OnceLock<Mutex<HashMap<PathBuf, Arc<Worker>>>> = OnceLock::new();
    SET.get_or_init(Default::default)
}

/// The worker of this store, started on the first need. A store reopened at the same path (a restart) gets a new one; the old one's thread
/// ends when its sender is dropped.
fn worker_for(store: &Arc<Store>) -> Option<Arc<Worker>> {
    let mut g = workers().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(w) = g.get(store.path()) {
        if w.store.upgrade().is_some_and(|s| Arc::ptr_eq(&s, store)) { return Some(w.clone()); }
    }
    let (tx, rx) = channel::<Job>();
    let w = Arc::new(Worker { store: Arc::downgrade(store), tx: Mutex::new(tx), queued: Mutex::new(HashSet::new()) });
    let me = Arc::downgrade(&w);
    let spawned = std::thread::Builder::new().name("whoosh-thread-summary".into()).spawn(move || {
        while let Ok(job) = rx.recv() {
            let Some(w) = me.upgrade() else { return };
            match job {
                Job::Flush(done) => { let _ = done.send(()); }
                Job::Make { thread_id, keychain, gate } => {
                    if let Some(store) = w.store.upgrade() {
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| make(&store, &keychain, gate.as_ref(), &thread_id))).is_err() {
                            log(&store, &format!("thread summary {thread_id}: stopped unexpectedly; the next one runs"));
                        }
                    }
                    w.queued.lock().unwrap_or_else(|e| e.into_inner()).remove(&thread_id);
                }
            }
        }
    });
    if let Err(e) = spawned { eprintln!("thread summary: the worker could not start: {e}"); return None; }
    g.insert(store.path().to_path_buf(), w.clone());
    Some(w)
}

fn log(store: &Store, line: &str) {
    crate::log::log_line(&crate::log::path_beside(store.path()), line);
}

/// `thread/<id>/summary`: the kept summary (a JSON string) or null. Hands a missing or stale summary to the background worker; never waits.
/// `gate`: the on-device gate, asked only when a summary is queued.
pub(crate) fn state(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, gate: &dyn Fn() -> Option<Arc<ModelGate>>, path: &str) -> Option<String> {
    let thread_id = path.strip_prefix("thread/")?.strip_suffix("/summary")?;
    if thread_id.is_empty() || thread_id.contains('/') { return None; }
    let row = match store.thread_summary(thread_id) {
        Ok(r) => r,
        Err(e) => { log(store, &format!("thread summary {thread_id}: not read: {e}")); None }
    };
    let Some(row) = row else { return Some("null".into()) };
    if row.is_stale() { queue(store, keychain, gate(), thread_id); }
    Some(row.text.map(|t| crate::ai_settings::json_string(&t)).unwrap_or_else(|| "null".into()))
}

fn queue(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, gate: Option<Arc<ModelGate>>, thread_id: &str) {
    let Some(w) = worker_for(store) else { return };
    if !w.queued.lock().unwrap_or_else(|e| e.into_inner()).insert(thread_id.to_string()) { return; }
    let job = Job::Make { thread_id: thread_id.to_string(), keychain: keychain.clone(), gate };
    if w.tx.lock().unwrap_or_else(|e| e.into_inner()).send(job).is_err() {
        w.queued.lock().unwrap_or_else(|e| e.into_inner()).remove(thread_id);
    }
}

/// Tests and the hidden check: waits until the summary worker of the store at `store_path` ran every job queued so far (true), or `within`
/// passed (false). True at once when no worker started. Production never calls it.
pub fn settle(store_path: &Path, within: Duration) -> bool {
    let Some(w) = workers().lock().unwrap_or_else(|e| e.into_inner()).get(store_path).cloned() else { return true };
    let (tx, rx) = channel();
    if w.tx.lock().unwrap_or_else(|e| e.into_inner()).send(Job::Flush(tx)).is_err() { return true; }
    rx.recv_timeout(within).is_ok()
}

/// The prompt: the newest message first, so the cut to the model's room (`ProviderModel`, which keeps the start) drops the oldest first.
fn prompt(messages: &[SummaryMessage]) -> String {
    let mut out = String::from("The thread, newest message first:\n\n");
    for m in messages.iter().rev() {
        let text: String = m.text.chars().take(SUMMARY_MESSAGE_CHARS).collect();
        out.push_str(&format!("From: {}\nSubject: {}\n\n{}\n\n---\n\n", m.sender, m.subject, text.trim()));
    }
    out
}

/// One summary of one thread, on the worker's thread: the writing model is resolved NOW (a key saved, removed or a consent withdrawn counts at
/// the next summary), asked once, and its answer kept with the newest message it read.
fn make(store: &Arc<Store>, keychain: &Arc<dyn Keyring>, gate: Option<&Arc<ModelGate>>, thread_id: &str) {
    let Ok(Some(row)) = store.thread_summary(thread_id) else { return };
    if !row.is_stale() { return; }
    let Some(newest) = row.newest.clone() else { return };
    // no writing model: nothing is asked (the refusal in words is `model_for`'s, `ai_no_model`; the reader draws the missing-setup dot)
    let Ok(choice) = model_for(store, keychain, AiJob::Draft) else { return };
    let on_device = choice.on_device;
    if !on_device {
        // a hosted provider sees no mail before the owner's "Allow" for it
        let slug = ai_providers::table::info(choice.provider).slug;
        if !matches!(store.agent_consent_at(slug), Ok(Some(_))) { return; }
    }
    let messages = match store.thread_summary_messages(thread_id, SUMMARY_MESSAGES) {
        Ok(m) if !m.is_empty() => m,
        _ => return,
    };
    let model = ProviderModel::new(Box::new(choice), on_device);
    let call = ModelCall { kind: CallKind::Judge, system: Some(SYSTEM.into()), prompt: prompt(&messages) };
    // a model on this Mac takes its turn at the gate the background askers share; a hosted one does not need it
    let answer = {
        let _turn = if on_device { gate.map(|g| g.turn()) } else { None };
        model.execute(&call)
    };
    match answer {
        Ok(text) if !text.trim().is_empty() => {
            if let Err(e) = store.thread_summary_put(thread_id, text.trim(), &newest) { log(store, &format!("thread summary {thread_id}: not kept: {e}")); }
        }
        Ok(_) => log(store, &format!("thread summary {thread_id}: the model answered nothing")),
        Err(e) => log(store, &format!("thread summary {thread_id}: not made: {e}")),
    }
}
