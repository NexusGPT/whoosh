//! What retrieval needs from the rest of the core. One method each, named `execute` (the search crate's and the ai-providers crate's habit).
//! Every port answers synchronously: the caller runs the whole retrieval in the background and reports by event, so no view waits on it.
use crate::domain::{ExcerptQuery, FoundExcerpt};
use contracts::CoreError;

/// The model that writes the questions. `prompt` is the retrieval prompt with `{{message}}` and `{{instruction}}` filled, already cut to the
/// caller's budget (`Budgets::question_prompt_chars`). Returns the model's raw text: one question per line, or a JSON list of strings.
/// Anything else is read as garbage and retrieval falls back to the message subject (named in the report).
pub trait QueryWriter: Send + Sync {
    fn execute(&self, prompt: &str) -> Result<String, CoreError>;
}

/// The excerpt index (the background indexer's chunks): search by meaning AND by words for `query.text`, only inside `query.sources`,
/// at most `query.limit` excerpts, best first, each with its source, date, sender, folder, subject, text and score.
/// An excerpt from a source outside `query.sources` is dropped here and counted as a leak (`DropReason::OutsideScope`).
pub trait ExcerptIndex: Send + Sync {
    fn execute(&self, query: &ExcerptQuery) -> Result<Vec<FoundExcerpt>, CoreError>;
}

/// The model that answers the AI filter's question about ONE excerpt. `prompt` holds the owner's question, the excerpt and the message, cut to
/// `Budgets::judge_prompt_chars`. Returns the raw answer; its first word is read as yes (yes, oui, true) or no (no, non, false). Any other answer, or
/// an error, leaves the excerpt KEPT and counted as not judged: the filter removes what the model said no to, never what it could not answer.
pub trait ExcerptJudge: Send + Sync {
    fn execute(&self, prompt: &str) -> Result<String, CoreError>;
}
