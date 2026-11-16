//! The real core's attachment port (contracts delta 11): the store names the candidate messages (attachment names and
//! read-out text holding every word) and holds their catalog; the vector index answers by meaning under its text keys
//! ("<message_id>/<attachment_id>#part"). `attachment_hits` fuses the three lists, one row per attachment, like FixtureCore.
use crate::ports::AttachmentPort;
use crate::usecases::attachment_hits::attachment_hits;
use attachment_index::{AttachmentRows, VectorMatch};
use contracts::{CoreError, SearchHit};
use std::sync::Arc;

pub struct StoreAttachments {
    pub store: Arc<store::Store>,
    /// None: no meaning index yet (no model); names and words still answer
    pub vectors: Option<Arc<search_vector::VectorIndex>>,
}

fn store_err(e: store::StoreError) -> CoreError {
    CoreError::Unexpected { reason_words: format!("the attachment search could not read the store: {e}") }
}

impl AttachmentPort for StoreAttachments {
    fn execute(&self, words: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError> {
        let vectors: Vec<VectorMatch> = match &self.vectors {
            Some(v) => v.search_keys(words, limit)?.into_iter().map(|h| VectorMatch { key: h.key, score: h.score }).collect(),
            None => Vec::new(),
        };
        let mut ids = self.store.attachment_candidates(words, limit).map_err(store_err)?;
        for m in &vectors {
            if let Some((message, _)) = m.key.split_once('/') {
                if !ids.iter().any(|i| i == message) {
                    ids.push(message.to_string());
                }
            }
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let catalog = self.store.catalog(&ids)?;
        let mut hits = attachment_hits(words, &catalog, &vectors);
        hits.truncate(limit);
        Ok(hits)
    }
}
