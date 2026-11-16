//! Where converted messages go. The pipeline writes through this port; the real adapter is the local database the sync writes to.
use store::{NewMessage, Store};

use crate::error::ImportError;

/// One method. Returns how many of the batch were new rows (a message already stored changes nothing).
pub trait MessageSink {
    fn insert(&mut self, batch: Vec<NewMessage>) -> Result<usize, ImportError>;
}

/// The store's own insert: `INSERT OR IGNORE` on the message id, so the same message stored again is a no-op.
pub struct StoreSink<'a> {
    pub store: &'a Store,
}

impl MessageSink for StoreSink<'_> {
    fn insert(&mut self, batch: Vec<NewMessage>) -> Result<usize, ImportError> {
        self.store.insert_messages(batch).map_err(|e| ImportError::Store { words: format!("the local database refused the imported mail: {e}") })
    }
}
