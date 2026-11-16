//! One file per provider. Each implements `ListModels` and `CompleteText` over the `Transport` seam.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;
use serde_json::Value;

use crate::call;
use crate::key::Key;
use crate::ports::{CompleteText, ListModels};
use crate::table::{info, ProviderId, ProviderInfo};
use crate::transport::{Body, Method, Request, Transport};
use crate::words;

mod anthropic;
mod chatgpt;
mod compat;
mod gemini;
mod lmstudio;
mod ollama;
mod openai;
mod openrouter;

pub use anthropic::Anthropic;
pub use chatgpt::ChatGptSignIn;
pub use gemini::Gemini;
pub use lmstudio::LmStudio;
pub use ollama::Ollama;
pub use openai::OpenAi;
pub use openrouter::OpenRouter;

/// A provider as the settings screen and the agents see it.
pub trait Provider: ListModels + CompleteText {
    fn info(&self) -> &'static ProviderInfo;
}

/// The adapter for `id`. `base_url` replaces the table's address (a local server on another port, a proxy).
pub fn build(id: ProviderId, transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> Box<dyn Provider> {
    match id {
        ProviderId::OpenAi => Box::new(OpenAi::new(transport, secrets, base_url)),
        ProviderId::Anthropic => Box::new(Anthropic::new(transport, secrets, base_url)),
        ProviderId::Gemini => Box::new(Gemini::new(transport, secrets, base_url)),
        ProviderId::OpenRouter => Box::new(OpenRouter::new(transport, secrets, base_url)),
        ProviderId::Ollama => Box::new(Ollama::new(transport, secrets, base_url)),
        ProviderId::LmStudio => Box::new(LmStudio::new(transport, secrets, base_url)),
        ProviderId::ChatGptSignIn => Box::new(ChatGptSignIn::new(transport, secrets, base_url, None)),
    }
}

/// What every adapter shares: its table row, its address, the seam and the secrets port.
pub(crate) struct Ctx {
    pub(crate) info: &'static ProviderInfo,
    base: String,
    extra: Vec<(String, String)>,
    transport: Arc<dyn Transport>,
    secrets: Arc<dyn ReadSecret>,
}

impl Ctx {
    pub(crate) fn new(id: ProviderId, transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> Ctx {
        let info = info(id);
        let base = base_url.unwrap_or_else(|| info.base_url.to_string()).trim_end_matches('/').to_string();
        Ctx { info, base, extra: Vec::new(), transport, secrets }
    }

    pub(crate) fn with_header(mut self, name: &str, value: String) -> Ctx {
        self.extra.push((name.to_string(), value));
        self
    }

    /// The key for this call. A provider that needs none accepts None; one that needs a key refuses None before any request.
    pub(crate) fn key(&self, reference: Option<&KeychainRef>) -> Result<Option<Key>, CoreError> {
        match (reference, self.info.needs_key) {
            (Some(r), _) => Key::read(&*self.secrets, r).map(Some),
            (None, true) => Err(CoreError::Invalid { field: "keychain_ref".into(), reason_words: format!("{} needs a key", self.info.name) }),
            (None, false) => Ok(None),
        }
    }

    fn headers(&self, key: Option<&Key>) -> Vec<(String, String)> {
        let mut h = self.info.auth.headers(key.map(Key::as_str));
        h.extend(self.info.extra_headers.iter().map(|(k, v)| (k.to_string(), v.to_string())));
        h.extend(self.extra.iter().cloned());
        h
    }

    pub(crate) fn get(&self, path: &str, key: Option<&Key>) -> Result<Value, CoreError> {
        let request = Request { method: Method::Get, url: format!("{}{path}", self.base), headers: self.headers(key), body: Body::None };
        call::json(&*self.transport, self.info.name, request, key)
    }

    pub(crate) fn post(&self, path: &str, body: Value, key: Option<&Key>) -> Result<Value, CoreError> {
        let mut headers = self.headers(key);
        headers.push(("Content-Type".to_string(), "application/json".to_string()));
        let request = Request { method: Method::Post, url: format!("{}{path}", self.base), headers, body: Body::Json(body.to_string()) };
        call::json(&*self.transport, self.info.name, request, key)
    }

    pub(crate) fn unreadable(&self) -> CoreError {
        words::unreadable(self.info.name)
    }
}

/// The text of an OpenAI Responses answer: every `output_text` part of every `message` item, in order.
pub(crate) fn responses_text(answer: &Value) -> Option<String> {
    let items = answer["output"].as_array()?;
    let text: String = items
        .iter()
        .filter(|i| i["type"] == "message")
        .flat_map(|i| i["content"].as_array().into_iter().flatten())
        .filter(|c| c["type"] == "output_text")
        .filter_map(|c| c["text"].as_str())
        .collect();
    if text.is_empty() { None } else { Some(text) }
}

/// A model id goes into a URL path (Gemini): refuse anything that could change the path.
pub(crate) fn check_model_id(model: &str) -> Result<(), CoreError> {
    let ok = !model.is_empty() && model.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'));
    if ok { Ok(()) } else { Err(CoreError::Invalid { field: "model".into(), reason_words: "a model id has letters, digits and - . _ only".into() }) }
}
