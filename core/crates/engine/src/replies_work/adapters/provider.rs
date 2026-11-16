//! `TextModel` over the model Settings, AI and agents chose for the job (`engine::ai_settings::model_for`: the Writer's provider and model, built
//! through the ai-providers table over that module's HTTPS, the key read from the Keychain at the call). What this file adds is the reply side only: the
//! prompt cut to the job's room, the answer's ceiling per call, ONE retry when the provider says a later try can pass, the provider's own words when it
//! fails. A missing or unconnected model is `model_for`'s refusal (code `ai_no_model`), passed on unchanged.
use std::sync::Arc;
use std::time::Duration;

use contracts::CoreError;
use store::Store;

use crate::ai_settings::{model_for, AiJob, ModelChoice};
use crate::replies_work::limits::*;
use crate::replies_work::model::{CallKind, ModelCall, TextModel};
use crate::rust_core::Keyring;

/// The first `n` characters of `s` (never cuts inside a character).
pub(crate) fn cut(s: &str, n: usize) -> String { s.chars().take(n).collect() }

fn output_tokens(kind: CallKind) -> u32 {
    match kind { CallKind::Draft => REPLY_DRAFT_OUTPUT_TOKENS, CallKind::Voice => REPLY_VOICE_OUTPUT_TOKENS, CallKind::Decide | CallKind::Questions | CallKind::Judge => REPLY_SHORT_OUTPUT_TOKENS }
}

/// One completion of the chosen model (`ModelChoice::complete`); a test passes a scripted one.
pub trait Completer: Send + Sync {
    fn complete(&self, system: Option<String>, prompt: String, max_output_tokens: u32) -> Result<String, CoreError>;
}

impl Completer for ModelChoice {
    fn complete(&self, system: Option<String>, prompt: String, max_output_tokens: u32) -> Result<String, CoreError> { ModelChoice::complete(self, system, prompt, max_output_tokens) }
}

/// The chosen model as the draft engine calls it.
pub struct ProviderModel { inner: Box<dyn Completer>, room: usize }

impl ProviderModel {
    /// `on_device` = a model on this Mac (`ModelChoice::on_device`): the smaller room.
    pub fn new(inner: Box<dyn Completer>, on_device: bool) -> ProviderModel {
        ProviderModel { inner, room: if on_device { REPLY_LOCAL_PROMPT_CHARS } else { REPLY_CLOUD_PROMPT_CHARS } }
    }
}

/// Whether a later try can pass: the provider's 429 or 5xx, or no answer at all (both come back retryable from ai-providers' words).
fn retryable(e: &CoreError) -> bool { matches!(e, CoreError::Action { failure } if failure.retryable) }

impl TextModel for ProviderModel {
    fn execute(&self, call: &ModelCall) -> Result<String, CoreError> {
        let once = || self.inner.complete(call.system.clone(), cut(&call.prompt, self.room), output_tokens(call.kind));
        match once() {
            Err(e) if retryable(&e) => {
                std::thread::sleep(Duration::from_millis(REPLY_RETRY_PAUSE_MS));
                once()
            }
            other => other,
        }
    }
    fn prompt_chars(&self) -> usize { self.room }
}

/// The model Settings, AI and agents chose for `job`, or `model_for`'s refusal in words (`ai_no_model`).
pub(crate) fn configured(store: &Store, keychain: &Arc<dyn Keyring>, job: AiJob) -> Result<Arc<dyn TextModel>, CoreError> {
    let choice = model_for(store, keychain, job)?;
    let on_device = choice.on_device;
    Ok(Arc::new(ProviderModel::new(Box::new(choice), on_device)))
}
