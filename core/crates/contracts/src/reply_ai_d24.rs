//! Delta 24: a mail folder removed on the server (replies-settings round 4). The row stays, switched off, with its last count, and says it is GONE.
//!
//! WHY a case of `ReplySyncProgress` (`Gone`) and not a `missing: bool` on `ReplySource`: it is the status the row's line already draws (up to date, syncing %, failed);
//! a new field would force every `ReplySource { .. }` literal in the workspace to change (18 outside the contract), a new enum case breaks only the exhaustive matches
//! (a handful, and the Sources pane's switch, which is the lane that must draw it). In the store the fact is two columns (0018), and the progress is derived on read.
//!
//! THE OWNER'S CHOICE IS REMEMBERED SEPARATELY: `SourcePresence { enabled, owner_enabled, gone }`. `enabled` is what the core acts on (forced off while gone);
//! `owner_enabled` holds the switch the owner last chose, written the moment the folder goes (and rewritten if the owner flips the switch of a gone row), and is None
//! while the folder is present. When the folder comes back, `enabled` returns to `owner_enabled`. All of it is `source_presence_after_list` / `_after_owner_switch`.
use crate::*;

/// What the store keeps about one source's presence: the effective switch, the owner's remembered choice while gone, and gone itself.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePresence { pub enabled: bool, pub owner_enabled: Option<bool>, pub gone: bool }

/// The legal shapes (the store's triggers repeat them): a gone source is switched off; the owner's remembered choice exists only while gone.
#[uniffi::export]
pub fn source_presence_is_legal(p: SourcePresence) -> bool { (!p.gone || !p.enabled) && (p.gone || p.owner_enabled.is_none()) }

/// What a LIST of the account's folders did to a source: `present` = the server still holds it. Gone while present: nothing. Present after gone: the owner's choice returns
/// (a folder never chosen reads off). Missing while present: the switch is remembered and forced off. Missing while already gone: nothing (the memory is kept).
#[uniffi::export]
pub fn source_presence_after_list(cur: SourcePresence, present: bool) -> SourcePresence {
    match (present, cur.gone) {
        (false, false) => SourcePresence { enabled: false, owner_enabled: Some(cur.enabled), gone: true },
        (true, true) => SourcePresence { enabled: cur.owner_enabled.unwrap_or(false), owner_enabled: None, gone: false },
        _ => cur,
    }
}

/// What the owner's switch does: on a present source it sets `enabled`; on a gone one it only changes the remembered choice (the source stays off until it comes back).
#[uniffi::export]
pub fn source_presence_after_owner_switch(cur: SourcePresence, enabled: bool) -> SourcePresence {
    if cur.gone { SourcePresence { owner_enabled: Some(enabled), ..cur } } else { SourcePresence { enabled, ..cur } }
}

/// The row as the Sources pane reads it: switched off and `Gone` while gone (the last count stays), else the progress as the crawl reports it.
#[uniffi::export]
pub fn reply_source_with_presence(source: ReplySource, p: SourcePresence) -> ReplySource {
    if p.gone { ReplySource { enabled: false, progress: ReplySyncProgress::Gone, ..source } } else { ReplySource { enabled: p.enabled, ..source } }
}

/// A gone source must be off (the record's own rule, for a reader of a stored row).
#[uniffi::export]
pub fn reply_source_is_legal(source: ReplySource) -> bool { !(source.progress == ReplySyncProgress::Gone && source.enabled) }
