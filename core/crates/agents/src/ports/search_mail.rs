use contracts::{CoreError, SearchHit};

/// The best `limit` messages for these words, best first. Bound to the search crate (hybrid text and vector search).
pub trait SearchMail: Send + Sync {
    fn execute(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError>;
}
