//! One account's card on the Sources pane: its folders as tiles, in the pane's order (the inbox, then sent, then the others by name,
//! so "Clients / Oranta" sits under "Clients"), each with its message count and the owner's switch and sync state.
use contracts::{reply_account_sources, ReplyAccountSources, ReplySource, ReplySourceRef, ReplySyncProgress};
use std::collections::HashMap;

/// A folder of the account as the store counts it. `role` is the folder's role as the account setup maps it ("inbox", "sent", ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailFolderCount {
    pub folder: String,
    pub role: Option<String>,
    pub count: u64,
}

fn rank(f: &MailFolderCount) -> u8 {
    match f.role.as_deref() {
        Some("inbox") => 0,
        _ if f.folder.eq_ignore_ascii_case("INBOX") => 0,
        Some("sent") => 1,
        _ => 2,
    }
}

/// The card of `account_id`: every folder of `folders` as a tile; `stored` = the reply-source rows (switch and sync state), a folder
/// with no row is off and NotStarted. A stored row whose folder is gone from the account is not a tile.
pub fn mail_folder_tiles(account_id: &str, mut folders: Vec<MailFolderCount>, stored: &[ReplySource]) -> ReplyAccountSources {
    let rows: HashMap<&str, &ReplySource> = stored
        .iter()
        .filter_map(|s| match &s.source {
            ReplySourceRef::MailFolder { account_id: a, folder } if a == account_id => Some((folder.as_str(), s)),
            _ => None,
        })
        .collect();
    folders.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.folder.to_lowercase().cmp(&b.folder.to_lowercase())));
    let tiles = folders
        .into_iter()
        .map(|f| {
            let row = rows.get(f.folder.as_str());
            ReplySource {
                source: ReplySourceRef::MailFolder { account_id: account_id.to_string(), folder: f.folder },
                enabled: row.is_some_and(|r| r.enabled),
                item_count: f.count,
                progress: row.map(|r| r.progress.clone()).unwrap_or(ReplySyncProgress::NotStarted),
            }
        })
        .collect();
    reply_account_sources(account_id.to_string(), tiles)
}
