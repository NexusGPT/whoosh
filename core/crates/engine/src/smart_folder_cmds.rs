//! real-folders (SPEC A143, A153): the smart folders on the real core. A smart folder is a saved SEARCH in the engine's own query language
//! (the owner's search parser, `search_cmd::parse_query`, so `from:@<domain>`, `from:`, `to:`, `subject:`, `label:`, `before:` and the rest are
//! kept in the folder's rule words exactly as typed). Kept on this Mac in table `smart_folder` (store `delta12.rs`); nothing here touches the network.
//!
//!  - Commands: SaveSmartFolder, RenameSmartFolder, UpdateSmartFolder, DeleteSmartFolder (the test engine's behaviour, FixtureCore.swift and
//!    FixtureLabels.swift: the same checks in the same order, the same words). None records an undo entry (the test engine records none).
//!  - Reads: `smart_folders()` (the sidebar's list, parents before children), the list of a folder's mails (`inbox` with `smart_folder_id`),
//!    and the state paths `smart_folder/count`, `smart_folder/<id>/{count,unread,threads,rule,parent}`, `smart_folder/<id or name>/{rule,exists}`.
//! - One reading of the rule for the list, `count`, `unread` and `threads`: the exact group of the owner's search for the rule
//!   (`search_cmd::exact_matches`, the one the smart-folder editor's "N matches" reads), kept to the mail the folder lists
//!   (`Store::smart_folder_members`: still in the inbox, not muted or snoozed, no sent copy, not from the owner's own address), so the sidebar's
//!   figure and the list cannot disagree. Newest first. At most SEARCH_RANKED_MAX (1000) mails, the search's own bound.
use contracts::{account_in, passes_filters, Clock, Command, CommandOutcome, CoreError, InboxQuery, InboxRow, SmartFolder};
use store::Store;

use super::search_cmd::{self, Parts};
use super::{now_ms, RustCore};

pub(crate) fn is_command(c: &Command) -> bool {
    matches!(c, Command::SaveSmartFolder { .. } | Command::RenameSmartFolder { .. } | Command::UpdateSmartFolder { .. } | Command::DeleteSmartFolder { .. })
}

fn unexpected(e: store::StoreError) -> CoreError { CoreError::Unexpected { reason_words: e.to_string() } }

/// Serves the four commands on the store's writer: one transaction each, committed before the answer (it survives a restart).
pub(crate) fn run(store: &Store, command: Command) -> Result<CommandOutcome, CoreError> {
    let now = now_ms();
    let done = |created_id: Option<String>| CommandOutcome { undo_entry_id: None, created_id };
    match command {
        Command::SaveSmartFolder { name, rule_words } => store.smart_folder_save(&name, &rule_words, now).map_err(unexpected)?.map(|id| done(Some(id))),
        Command::RenameSmartFolder { folder_id, name } => store.smart_folder_rename(&folder_id, &name, now).map_err(unexpected)?.map(|_| done(None)),
        Command::UpdateSmartFolder { folder_id, rule_words, parent_id } => {
            store.smart_folder_update(&folder_id, &rule_words, parent_id.as_deref(), now).map_err(unexpected)?.map(|_| done(None))
        }
        Command::DeleteSmartFolder { folder_id } => store.smart_folder_delete(&folder_id).map_err(unexpected)?.map(|_| done(None)),
        _ => Err(super::codes::refusal("The local engine does not do this yet.", super::codes::NOT_BUILT_YET, "")),
    }
}

/// The sidebar's list: every folder, parents before children.
pub(crate) fn folders(store: &Store) -> Vec<SmartFolder> {
    store.smart_folders().unwrap_or_else(|e| {
        eprintln!("smart_folders: the mail database could not be read: {e}");
        Vec::new()
    })
}

fn parts(core: &RustCore) -> Parts {
    Parts { store: core.store.clone(), indexer: core.deps.bodies.indexer.clone(), roles: core.deps.roles.clone(), sinks: core.sinks.clone(), utc_offset_s: super::SystemClock.utc_offset_s() }
}

/// The mails a rule lists, newest first, each with whether it is unread: the one reading behind the list and every figure.
fn members(core: &RustCore, rule_words: &str) -> Result<Vec<(String, bool)>, String> {
    let hits = search_cmd::exact_matches(&parts(core), rule_words)?;
    let ids: Vec<String> = hits.into_iter().map(|h| h.message_id).collect();
    core.store.smart_folder_members(&ids).map_err(|e| e.to_string())
}

/// The list of one folder (`InboxQuery.smart_folder_id`): its mails newest first, kept to the query's account set and filters (as every
/// list); the tab, mailbox and labels of the query are ignored. An unknown folder lists nothing.
pub(crate) fn inbox(core: &RustCore, q: &InboxQuery) -> Vec<InboxRow> {
    let Some(id) = q.smart_folder_id.as_deref() else { return Vec::new() };
    let Some(folder) = core.store.smart_folder(id).ok().flatten() else { return Vec::new() };
    let ids: Vec<String> = match members(core, &folder.rule_words) {
        Ok(m) => m.into_iter().map(|(id, _)| id).collect(),
        Err(e) => {
            eprintln!("smart folder list: the mail database could not be read: {e}");
            return Vec::new();
        }
    };
    let mut rows = core.store.rows_for_messages(&ids).unwrap_or_default();
    rows.retain(|r| account_in(q, &r.account_id));
    crate::reminders::mark_rows(&core.store, &mut rows, now_ms());
    if !q.filters.is_empty() {
        let ids: Vec<String> = rows.iter().map(|r| r.message_id.clone()).collect();
        let facts = core.store.filter_facts(&ids).unwrap_or_default();
        rows.retain(|r| facts.get(&r.message_id).is_some_and(|f| passes_filters(*f, q.filters.clone())));
    }
    rows
}

fn find<'a>(all: &'a [SmartFolder], key: &str) -> Option<&'a SmartFolder> {
    all.iter().find(|f| f.id == key).or_else(|| all.iter().find(|f| f.name.to_lowercase() == key.to_lowercase()))
}

/// The state paths of smart folders (the test engine's: Sources/CoreBridge/FixtureCore.swift `state(path:)`). A folder asked for by an id
/// that is not saved answers nothing here (the pre-existing `state_extra.rs` line then answers `0` for its count, as before).
pub(crate) fn state(core: &RustCore, path: &str) -> Option<String> {
    let rest = path.strip_prefix("smart_folder/")?;
    if rest == "count" { return Some(folders(&core.store).len().to_string()); }
    // a name may hold dots, spaces and slashes: read the middle whole
    if let Some(key) = rest.strip_suffix("/rule").filter(|k| !k.is_empty()) {
        return find(&folders(&core.store), key).map(|f| crate::reads::json_text(&f.rule_words));
    }
    if let Some(key) = rest.strip_suffix("/exists").filter(|k| !k.is_empty()) {
        return Some(find(&folders(&core.store), key).is_some().to_string());
    }
    let (id, what) = rest.split_once('/')?;
    let folder = core.store.smart_folder(id).ok().flatten()?;
    match what {
        "parent" => folder.parent_id.as_deref().map(crate::reads::json_text),
        "count" | "unread" | "threads" => {
            let m = match members(core, &folder.rule_words) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("smart_folder/{what}: the mail database could not be read: {e}");
                    return None;
                }
            };
            Some(match what {
                "count" => m.len().to_string(),
                "unread" => m.iter().filter(|(_, unread)| *unread).count().to_string(),
                _ => format!("[{}]", m.iter().map(|(id, _)| crate::reads::json_text(id)).collect::<Vec<_>>().join(",")),
            })
        }
        _ => None,
    }
}
