//! The one model port the draft engine calls (the decision, the questions, an excerpt's judgement, the draft, the voice), and how the model is
//! chosen: the one a test (or a shell) set for this core, else the Writer agent's provider and model from Settings, IA et agents. None of either =
//! the refusal in words of `ai_settings::model_for` (code `ai_no_model`), never a silent nothing.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use contracts::CoreError;
use store::Store;

use crate::ai_settings::AiJob;

/// The refusal code of a second voice analysis while one runs. Not in the contract yet (same finding).
pub const REPLY_VOICE_RUNNING_CODE: &str = "reply_voice_running";

/// Which of the up to three calls of a reply (plus the voice analysis) this is: each has its own answer ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallKind { Decide, Questions, Judge, Draft, Voice }

/// One call: the system text (None = none) and the prompt, already cut to `TextModel::prompt_chars`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCall { pub kind: CallKind, pub system: Option<String>, pub prompt: String }

/// The model. Blocking: every caller runs on the engine's reply threads, never on a command's thread.
pub trait TextModel: Send + Sync {
    /// The model's text, or the failure in words (the provider's own words when it refused).
    fn execute(&self, call: &ModelCall) -> Result<String, CoreError>;
    /// The characters of prompt one call may carry.
    fn prompt_chars(&self) -> usize;
}

fn overrides() -> &'static Mutex<HashMap<PathBuf, Arc<dyn TextModel>>> {
    static SET: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn TextModel>>>> = OnceLock::new();
    SET.get_or_init(Default::default)
}

/// Sets (Some) or clears (None) the model of the core over the store at `store_path`, in place of the Writer agent's: the tests' fake model, or a
/// shell that holds a model of its own. Takes effect at the next call.
pub fn set_reply_model(store_path: &std::path::Path, model: Option<Arc<dyn TextModel>>) {
    let mut g = overrides().lock().unwrap_or_else(|e| e.into_inner());
    match model { Some(m) => { g.insert(store_path.to_path_buf(), m); } None => { g.remove(store_path); } }
}

/// The model for this store now: the one set, else the one Settings, AI and agents chose for `job`, else the refusal.
pub(crate) fn model_for(store: &Store, job: AiJob, configured: &dyn Fn(&Store, AiJob) -> Result<Arc<dyn TextModel>, CoreError>) -> Result<Arc<dyn TextModel>, CoreError> {
    if let Some(m) = overrides().lock().unwrap_or_else(|e| e.into_inner()).get(store.path()).cloned() { return Ok(m); }
    configured(store, job)
}
