use std::sync::Arc;

use ai_providers::CompleteText;
use contracts::{CoreError, KeychainRef};

/// A provider ready for one call: its adapter, its key reference, and whether it runs on this device.
pub struct BoundProvider {
    /// the provider's name in words ("OpenAI")
    pub name: String,
    /// true for a model that runs on this Mac (Ollama, LM Studio, Apple's on-device model): no mail leaves the device.
    pub on_device: bool,
    pub key: Option<KeychainRef>,
    pub client: Arc<dyn CompleteText>,
}

/// The provider an agent names. An unknown or disconnected provider is an error in words.
pub trait ResolveProvider: Send + Sync {
    fn execute(&self, provider_id: &str) -> Result<BoundProvider, CoreError>;
}
