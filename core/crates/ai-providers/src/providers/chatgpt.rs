//! The ChatGPT sign-in token: the access token the sign-in lane stored in the Keychain, sent as a bearer to the
//! ChatGPT backend that the Codex CLI uses. OpenAI publishes no policy for this in other apps and can change the
//! route at any time, so the table row carries the note "It can stop working at any time" and every failure here
//! is shown in OpenAI's words. Token refresh belongs to the sign-in lane, not to this adapter.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;
use serde_json::{json, Value};

use super::{compat, responses_text, Ctx, Provider};
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

pub struct ChatGptSignIn {
    ctx: Ctx,
}

impl ChatGptSignIn {
    /// `account_id` is the ChatGPT account the token belongs to (not a secret); the sign-in lane reads it from the token.
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>, account_id: Option<String>) -> ChatGptSignIn {
        let mut ctx = Ctx::new(ProviderId::ChatGptSignIn, transport, secrets, base_url);
        if let Some(id) = account_id {
            ctx = ctx.with_header("chatgpt-account-id", id);
        }
        ChatGptSignIn { ctx }
    }
}

impl ListModels for ChatGptSignIn {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        compat::list(&self.ctx, key.as_ref())
    }
}

impl CompleteText for ChatGptSignIn {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        let body = json!({
            "model": request.model,
            "instructions": request.system.clone().unwrap_or_default(),
            "input": [{"role": "user", "content": [{"type": "input_text", "text": request.prompt}]}],
            "store": false,
        });
        let answer: Value = self.ctx.post("/responses", body, key.as_ref())?;
        responses_text(&answer).ok_or_else(|| self.ctx.unreadable())
    }
}

impl Provider for ChatGptSignIn {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
