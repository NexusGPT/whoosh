//! Ollama, a local server: no key, the models the owner has pulled.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{compat, Ctx, Provider};
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

pub struct Ollama {
    ctx: Ctx,
}

impl Ollama {
    /// `base_url` is the owner's address for the server when it is not the table's.
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> Ollama {
        Ollama { ctx: Ctx::new(ProviderId::Ollama, transport, secrets, base_url) }
    }
}

impl ListModels for Ollama {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        compat::list(&self.ctx, key.as_ref())
    }
}

impl CompleteText for Ollama {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        compat::chat(&self.ctx, key.as_ref(), request)
    }
}

impl Provider for Ollama {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
