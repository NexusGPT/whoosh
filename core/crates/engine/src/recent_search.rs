//! engine-act-r6 round 6: the recent searches (contracts delta 8). `RecordSearch { query }` (Enter on a search, SearchMacSave.swift) keeps the
//! words in `search_history` (migration 0001): newest first, one row per distinct words in any case, at most SEARCH_RECENT_MAX; empty words
//! are refused. State `search/recent` = that list as a JSON array (SearchMacModel.swift). Device-local, never synced.
use contracts::{CommandOutcome, CoreError};
use store::Store;

use crate::reads::json_text;

fn keep() -> usize { contracts::constants::SEARCH_RECENT_MAX as usize }

pub(crate) fn record(store: &Store, query: &str, now_ms: i64) -> Result<CommandOutcome, CoreError> {
    let q = query.trim();
    if q.is_empty() { return Err(CoreError::Invalid { field: "query".into(), reason_words: "type something to search for".into() }); }
    store.record_search(q, now_ms, keep()).map_err(|e| CoreError::Unexpected { reason_words: format!("the search was not kept: {e}") })?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    (path == "search/recent").then(|| store.recent_searches(keep()).ok()).flatten()
        .map(|list| format!("[{}]", list.iter().map(|q| json_text(q)).collect::<Vec<_>>().join(",")))
}
