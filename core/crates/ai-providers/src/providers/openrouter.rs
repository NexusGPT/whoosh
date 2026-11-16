//! OpenRouter: one key, many vendors, over the OpenAI-compatible endpoint.
//! Its model list answers without a key, so the key is checked first against `GET /key`, where a refused key gets the provider's words.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{compat, Ctx, Provider};
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

pub struct OpenRouter {
    ctx: Ctx,
}

impl OpenRouter {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> OpenRouter {
        OpenRouter { ctx: Ctx::new(ProviderId::OpenRouter, transport, secrets, base_url) }
    }
}

impl ListModels for OpenRouter {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        self.ctx.get("/key", key.as_ref())?;
        compat::list(&self.ctx, key.as_ref())
    }
}

impl CompleteText for OpenRouter {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        compat::chat(&self.ctx, key.as_ref(), request)
    }
}

impl Provider for OpenRouter {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
