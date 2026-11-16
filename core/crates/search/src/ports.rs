//! What the search needs from the rest of the core. One method each, named `execute`.
//! Text and meaning both answer in `contracts::SearchHit`, best first; the fusion reads only the order.
use crate::domain::{AccountState, Filters};
use contracts::{CoreError, Message, SearchHit};

/// Exact words: every bare word must appear; a quoted phrase is that exact run of words. (crate `search-text`)
pub trait TextPort: Send + Sync {
    fn execute(&self, words: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError>;
}

/// Meaning: the closest messages to the words, whatever words they use. (crate `search-vector`)
pub trait MeaningPort: Send + Sync {
    fn execute(&self, words: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError>;
}

/// The stored facts of these messages (thread, account, sender, date, read, attachments). A gone id is left out.
pub trait FactsPort: Send + Sync {
    fn execute(&self, ids: &[String]) -> Result<Vec<Message>, CoreError>;
}

/// Messages matching the filters alone, newest first: the answer to a query that is only chips.
pub trait FilterScanPort: Send + Sync {
    fn execute(&self, filters: &Filters, limit: usize) -> Result<Vec<String>, CoreError>;
}

/// Every account with its display name and whether its mail is in the indexes yet.
pub trait AccountsPort: Send + Sync {
    fn execute(&self) -> Result<Vec<AccountState>, CoreError>;
}

/// Attachments: what the files of the mail say (text, OCR, scene labels, transcripts) and their names, as hits that name the
/// attachment (`matched_in`, `attachment_name`, `attachment_note`), one per attachment, best first. The real core answers it
/// with `attachment_hits` over its catalog and the vector index's text keys (contracts delta 10 hook).
pub trait AttachmentPort: Send + Sync {
    fn execute(&self, words: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError>;
}
