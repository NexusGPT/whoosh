//! Google Gemini through the Generative Language API with the owner's key, sent in a header (never in the URL).
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;
use serde_json::{json, Map, Value};

use super::{check_model_id, Ctx, Provider};
use crate::call::enc;
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

const GENERATE: &str = "generateContent";

pub struct Gemini {
    ctx: Ctx,
}

impl Gemini {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> Gemini {
        Gemini { ctx: Ctx::new(ProviderId::Gemini, transport, secrets, base_url) }
    }
}

impl ListModels for Gemini {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        let mut ids = Vec::new();
        let mut path = "/models".to_string();
        loop {
            let page = self.ctx.get(&path, key.as_ref())?;
            let models = page["models"].as_array().ok_or_else(|| self.ctx.unreadable())?;
            for m in models {
                // Only models that can write text; a model with no method list is kept.
                let can_generate = m["supportedGenerationMethods"].as_array().map_or(true, |a| a.iter().any(|g| g == GENERATE));
                if let (true, Some(name)) = (can_generate, m["name"].as_str()) {
                    ids.push(name.strip_prefix("models/").unwrap_or(name).to_string());
                }
            }
            match page["nextPageToken"].as_str().filter(|t| !t.is_empty()) {
                Some(token) => path = format!("/models?pageToken={}", enc(token)),
                None => return Ok(ids),
            }
        }
    }
}

impl CompleteText for Gemini {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        let model = request.model.strip_prefix("models/").unwrap_or(&request.model);
        check_model_id(model)?;
        let mut body = Map::new();
        body.insert("contents".into(), json!([{"role": "user", "parts": [{"text": request.prompt}]}]));
        body.insert("generationConfig".into(), json!({"maxOutputTokens": request.max_output_tokens}));
        if let Some(system) = &request.system {
            body.insert("systemInstruction".into(), json!({"parts": [{"text": system}]}));
        }
        let answer = self.ctx.post(&format!("/models/{model}:{GENERATE}"), Value::Object(body), key.as_ref())?;
        let text: String = answer["candidates"][0]["content"]["parts"].as_array().into_iter().flatten().filter_map(|p| p["text"].as_str()).collect();
        if text.is_empty() { Err(self.ctx.unreadable()) } else { Ok(text) }
    }
}

impl Provider for Gemini {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
