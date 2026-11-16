//! A page of messages and its folder cursor in ONE transaction (the sync engine's gap, docs/lanes/core-sync-engine.md
//! finding 4). The anchor and the cursor move in the same commit as the messages they cover, so a stop between the two
//! can no longer leave a cursor past a message that is not stored, nor a stored page whose cursor never moved.
use crate::error::StoreError;
use crate::ops::NewMessage;
use crate::Store;
use contracts::SyncCursor;
use rusqlite::params;

/// The state of one folder after a page: where the next page starts, whether nothing lies below it, and the change cursor.
#[derive(Debug, Clone, Default)]
pub struct FolderProgress {
    pub account_id: String,
    pub folder: String,
    /// The position of the oldest message in the page (an IMAP UID, or an ordinal); stored in `last_uid`.
    pub oldest: Option<i64>,
    /// True when nothing lies below `oldest`: the full pass time is stored.
    pub done: bool,
    pub cursor: Option<SyncCursor>,
    pub now_ms: i64,
}

fn cursor_columns(c: &SyncCursor) -> (&'static str, Option<i64>, Option<i64>, Option<&str>) {
    match c {
        SyncCursor::Imap { uid_validity, highest_mod_seq } => ("imap", Some(*uid_validity as i64), Some(*highest_mod_seq as i64), None),
        SyncCursor::GraphDelta { token } => ("graph_delta", None, None, Some(token)),
        SyncCursor::GmailHistory { history_id } => ("gmail_history", None, None, Some(history_id)),
        SyncCursor::Jmap { state } => ("jmap", None, None, Some(state)),
    }
}

impl Store {
    /// Store `messages` and the folder's progress in one transaction. Messages already stored are left as they are. The
    /// account's `messages_indexed` grows by the number of messages that were new, and its `last_synced_ms` moves to
    /// `now_ms`. Returns how many messages were new. An error undoes all of it.
    pub fn write_page(&self, messages: Vec<NewMessage>, progress: FolderProgress) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            let inserted = if messages.is_empty() { 0 } else { ctx.insert_batch(&messages)? };
            let FolderProgress { account_id: aid, folder: f, oldest, done, cursor, now_ms } = progress;
            let tx = ctx.tx;
            tx.execute("INSERT INTO folder_sync_state (account_id, folder) VALUES (?1, ?2) ON CONFLICT(account_id, folder) DO NOTHING", params![aid, f])?;
            tx.execute("UPDATE folder_sync_state SET last_fetched_ms = ?3 WHERE account_id = ?1 AND folder = ?2", params![aid, f, now_ms])?;
            if let Some(o) = oldest {
                tx.execute("UPDATE folder_sync_state SET last_uid = ?3 WHERE account_id = ?1 AND folder = ?2", params![aid, f, o])?;
            }
            if done {
                tx.execute("UPDATE folder_sync_state SET last_full_pass_ms = ?3 WHERE account_id = ?1 AND folder = ?2", params![aid, f, now_ms])?;
            }
            if let Some(c) = &cursor {
                let (kind, validity, mod_seq, value) = cursor_columns(c);
                tx.execute(
                    "UPDATE folder_sync_state SET cursor_kind = ?3, imap_uid_validity = ?4, imap_highest_mod_seq = ?5, cursor_value = ?6 WHERE account_id = ?1 AND folder = ?2",
                    params![aid, f, kind, validity, mod_seq, value],
                )?;
            }
            tx.execute("INSERT INTO account_sync_state (account_id) VALUES (?1) ON CONFLICT(account_id) DO NOTHING", params![aid])?;
            tx.execute(
                "UPDATE account_sync_state SET messages_indexed = messages_indexed + ?2, last_synced_ms = ?3 WHERE account_id = ?1",
                params![aid, inserted as i64, now_ms],
            )?;
            Ok(inserted)
        })
    }
}
