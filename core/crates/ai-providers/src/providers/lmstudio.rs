//! LM Studio, a local server: no key needed, an optional one is sent when the owner set it.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{compat, Ctx, Provider};
use crate::ports::{CompleteText, ListModels, TextRequest};
use crate::table::{ProviderId, ProviderInfo};
use crate::transport::Transport;

pub struct LmStudio {
    ctx: Ctx,
}

impl LmStudio {
    /// `base_url` is the owner's address for the server when it is not the table's.
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, base_url: Option<String>) -> LmStudio {
        LmStudio { ctx: Ctx::new(ProviderId::LmStudio, transport, secrets, base_url) }
    }
}

impl ListModels for LmStudio {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError> {
        let key = self.ctx.key(key)?;
        compat::list(&self.ctx, key.as_ref())
    }
}

impl CompleteText for LmStudio {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError> {
        let key = self.ctx.key(key)?;
        compat::chat(&self.ctx, key.as_ref(), request)
    }
}

impl Provider for LmStudio {
    fn info(&self) -> &'static ProviderInfo {
        self.ctx.info
    }
}
