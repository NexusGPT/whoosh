//! An in-memory mail corpus behind every port: the fixtures' stand-in for the store and the two indexes, and a
//! preview source for the UI lanes. Its text half follows `search-text`'s rules (every bare word must appear,
//! a quoted phrase is that exact run of words, whole words only); its meaning half answers from a ranked table
//! the caller supplies, since meaning comes from the vector lane's embedder, which this crate does not run.
use crate::domain::{AccountState, Filters};
use crate::ports::{AccountsPort, FactsPort, FilterScanPort, MeaningPort, TextPort};
use crate::usecases::highlight::mark_words;
use crate::usecases::parse_query::tokens;
use contracts::constants::SEARCH_SNIPPET_CHARS;
use contracts::{CoreError, Message, SearchHit};
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct MemoryCorpus {
    pub messages: Vec<Message>,
    /// Full bodies by message id (a `Message` carries only its preview).
    pub bodies: HashMap<String, String>,
    pub accounts: Vec<AccountState>,
    /// Meaning answers: the words asked, and the ids the vector index would return for them, best first.
    pub meaning: HashMap<String, Vec<String>>,
}

fn holds(hay: &str, term: &str) -> bool {
    mark_words(hay, &[term.to_string()]).iter().any(|s| s.marked)
}

impl MemoryCorpus {
    fn newest_first(&self) -> Vec<&Message> {
        let mut all: Vec<&Message> = self.messages.iter().collect();
        all.sort_by(|a, b| b.received_at_ms.cmp(&a.received_at_ms).then(a.id.cmp(&b.id)));
        all
    }
    fn body(&self, m: &Message) -> String {
        self.bodies.get(&m.id).cloned().unwrap_or_else(|| m.preview.clone())
    }
}

impl TextPort for MemoryCorpus {
    fn execute(&self, words: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError> {
        let terms: Vec<String> = tokens(words).into_iter().map(|t| t.trim_matches('"').trim().to_string()).filter(|t| !t.is_empty()).collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let hits = self.newest_first().into_iter().filter(|m| {
            let hay = format!("{}\n{} {}\n{}\n{}", m.subject, m.sender_name, m.from_address, self.body(m), m.attachment_names.join(" "));
            terms.iter().all(|t| holds(&hay, t))
        });
        Ok(hits.take(limit).map(|m| SearchHit { message_id: m.id.clone(), score: 1.0, snippet: self.body(m).chars().take(SEARCH_SNIPPET_CHARS as usize).collect(), matched_in: contracts::MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None }).collect())
    }
}

impl MeaningPort for MemoryCorpus {
    fn execute(&self, words: &str, limit: usize) -> Result<Vec<SearchHit>, CoreError> {
        let ids = self.meaning.get(words.trim()).cloned().unwrap_or_default();
        Ok(ids.into_iter().take(limit).map(|id| SearchHit { message_id: id, score: 0.0, snippet: String::new(), matched_in: contracts::MatchedIn::MeaningOnly, attachment_name: None, attachment_note: None }).collect())
    }
}

impl FactsPort for MemoryCorpus {
    fn execute(&self, ids: &[String]) -> Result<Vec<Message>, CoreError> {
        Ok(ids.iter().filter_map(|id| self.messages.iter().find(|m| &m.id == id).cloned()).collect())
    }
}

impl FilterScanPort for MemoryCorpus {
    fn execute(&self, filters: &Filters, limit: usize) -> Result<Vec<String>, CoreError> {
        Ok(self.newest_first().into_iter().filter(|m| filters.admits(m)).take(limit).map(|m| m.id.clone()).collect())
    }
}

impl AccountsPort for MemoryCorpus {
    fn execute(&self) -> Result<Vec<AccountState>, CoreError> {
        Ok(self.accounts.clone())
    }
}
