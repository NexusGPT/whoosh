//! The key check: one model listing with a key the owner just pasted, BEFORE the key is kept anywhere. The pasted key lives only in
//! this call (a one-item secrets port in memory); a refusal carries the provider's own words with the key scrubbed out of them.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::{ReadSecret, Secret};

use crate::ports::ListModels;
use crate::providers::build;
use crate::table::{ProviderId, PROVIDERS};
use crate::transport::Transport;

/// The pasted key, answered for the one reference this check passes. Debug never prints it.
struct Candidate(Secret);

impl ReadSecret for Candidate {
    fn execute(&self, _reference: &KeychainRef) -> Result<Secret, CoreError> {
        Ok(self.0.clone())
    }
}

/// The reference the check reads the pasted key by. It names no Keychain item: nothing is read from or written to the Keychain.
fn candidate_ref() -> KeychainRef {
    KeychainRef { service: "pasted-key-check".into(), account: "candidate".into() }
}

/// The table row whose slug is `slug` (`"anthropic"`, `"openai"`, ...), if any.
pub fn provider_by_slug(slug: &str) -> Option<ProviderId> {
    PROVIDERS.iter().find(|p| p.slug == slug).map(|p| p.id)
}

/// Asks the provider which models `key` can use. `Ok` = the provider accepted the key (its model ids, in its order);
/// `Err(CoreError::Action)` = the provider refused it (`retryable` false: a refused key; true: a 429, a 5xx or no answer),
/// in the provider's words; `Err(CoreError::Invalid)` = the key is empty. `base_url` replaces the table's address (a test's local fake).
pub fn check_key(id: ProviderId, transport: Arc<dyn Transport>, key: &str, base_url: Option<String>) -> Result<Vec<String>, CoreError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(CoreError::Invalid { field: "key".into(), reason_words: "paste the key first".into() });
    }
    let provider = build(id, transport, Arc::new(Candidate(Secret::new(key.as_bytes().to_vec()))), base_url);
    ListModels::execute(&*provider, Some(&candidate_ref()))
}
