//! OpenAI over the Responses API with the owner's key.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;
use serde_json::{json, Map, Value};

use super::{compat, responses_text, Ctx, Provider};
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

pub struct OpenAi {
    ctx: Ctx,
}

impl OpenAi {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> OpenAi {
        OpenAi { ctx: Ctx::new(ProviderId::OpenAi, transport, secrets, base_url) }
    }
}

impl ListModels for OpenAi {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        compat::list(&self.ctx, key.as_ref())
    }
}

impl CompleteText for OpenAi {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        let mut body = Map::new();
        body.insert("model".into(), json!(request.model));
        body.insert("input".into(), json!(request.prompt));
        body.insert("max_output_tokens".into(), json!(request.max_output_tokens));
        if let Some(system) = &request.system {
            body.insert("instructions".into(), json!(system));
        }
        let answer = self.ctx.post("/responses", Value::Object(body), key.as_ref())?;
        responses_text(&answer).ok_or_else(|| self.ctx.unreadable())
    }
}

impl Provider for OpenAi {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
