//! A smart folder is a saved query (C-J2.5): it stores the line as chips plus words, and lists exactly what
//! that line finds. Writing it to the store is the store's job; this checks the name and fixes the words.
use crate::domain::SearchOutcome;
use crate::usecases::run_search::{SearchRequest, Searcher};
use contracts::{CoreError, SmartFolder};

/// A new smart folder for `query`. Refused when the name is empty or a sibling already has it (case-blind).
pub fn new_smart_folder(searcher: &Searcher, id: String, name: &str, parent_id: Option<String>, req: &SearchRequest<'_>, existing: &[SmartFolder]) -> Result<SmartFolder, CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Invalid { field: "name".into(), reason_words: "a smart folder needs a name".into() });
    }
    if existing.iter().any(|f| f.parent_id == parent_id && f.name.to_lowercase() == name.to_lowercase()) {
        return Err(CoreError::Invalid { field: "name".into(), reason_words: format!("a smart folder named \"{name}\" already exists here") });
    }
    let rule_words = searcher.parse(req).canonical_text();
    if rule_words.is_empty() {
        return Err(CoreError::Invalid { field: "rule_words".into(), reason_words: "an empty search cannot be saved".into() });
    }
    Ok(SmartFolder { id, name: name.to_string(), rule_words, parent_id })
}

/// What the folder lists now: its saved words, searched.
pub fn run_smart_folder(searcher: &Searcher, folder: &SmartFolder, req: &SearchRequest<'_>) -> Result<SearchOutcome, CoreError> {
    searcher.run(&SearchRequest { text: &folder.rule_words, today: req.today, utc_offset_min: req.utc_offset_min })
}

/// What a smart folder shows (contracts delta 14, `InboxQuery.smart_folder_id`): one list row per matching message the
/// inbox still shows, newest first; their ids (`smart_folder/<id>/threads` names these, the list's row ids); and the
/// count, the number of those rows, so the sidebar's figure is the list's (an archive takes one out of both).
#[derive(Debug, Clone, PartialEq)]
pub struct SmartFolderView { pub rows: Vec<contracts::InboxRow>, pub message_ids: Vec<String>, pub count: u32, pub unread: u32 }

/// The real core's answer to a smart folder: the store's saved words, searched, then the store's own rows for them.
pub fn smart_folder_rows(searcher: &Searcher, store: &store::Store, folder_id: &str, req: &SearchRequest<'_>) -> Result<SmartFolderView, CoreError> {
    let folder = store.smart_folder(folder_id).map_err(|e| CoreError::Unexpected { reason_words: e.to_string() })?
        .ok_or_else(|| CoreError::NotFound { kind: "smart folder".into(), id: folder_id.into() })?;
    let outcome = run_smart_folder(searcher, &folder, req)?;
    let mut seen = std::collections::HashSet::new();
    let ids: Vec<String> = outcome.threads.iter().flat_map(|t| t.matches.iter().map(|m| m.message_id.clone())).filter(|id| seen.insert(id.clone())).collect();
    let mut rows = store.rows_for(&ids).map_err(|e| CoreError::Unexpected { reason_words: e.to_string() })?;
    rows.sort_by(|a, b| b.received_at_ms.cmp(&a.received_at_ms).then_with(|| a.message_id.cmp(&b.message_id)));
    let message_ids: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect();
    let unread = rows.iter().filter(|r| r.unread).count() as u32;
    Ok(SmartFolderView { count: rows.len() as u32, rows, message_ids, unread })
}

