//! Anthropic with an API key from the Console, sent as the `x-api-key` header. This adapter has one way in.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;
use serde_json::{json, Map, Value};

use super::{Ctx, Provider};
use crate::call::enc;
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

pub struct Anthropic {
    ctx: Ctx,
}

impl Anthropic {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> Anthropic {
        Anthropic { ctx: Ctx::new(ProviderId::Anthropic, transport, secrets, base_url) }
    }
}

impl ListModels for Anthropic {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        let mut ids = Vec::new();
        let mut path = "/models".to_string();
        loop {
            let page = self.ctx.get(&path, key.as_ref())?;
            let data = page["data"].as_array().ok_or_else(|| self.ctx.unreadable())?;
            ids.extend(data.iter().filter_map(|m| m["id"].as_str().map(String::from)));
            // The list is paged: follow `has_more` from the last id until the provider says it is done.
            let last = page["last_id"].as_str().map(String::from);
            match (page["has_more"].as_bool(), last) {
                (Some(true), Some(last)) => path = format!("/models?after_id={}", enc(&last)),
                _ => return Ok(ids),
            }
        }
    }
}

impl CompleteText for Anthropic {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        let mut body = Map::new();
        body.insert("model".into(), json!(request.model));
        body.insert("max_tokens".into(), json!(request.max_output_tokens));
        body.insert("messages".into(), json!([{"role": "user", "content": request.prompt}]));
        if let Some(system) = &request.system {
            body.insert("system".into(), json!(system));
        }
        let answer = self.ctx.post("/messages", Value::Object(body), key.as_ref())?;
        let text: String = answer["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "text")
            .filter_map(|c| c["text"].as_str())
            .collect();
        if text.is_empty() { Err(self.ctx.unreadable()) } else { Ok(text) }
    }
}

impl Provider for Anthropic {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
