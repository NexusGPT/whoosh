//! Engine-actions: `Unarchive { message_ids }` (mark not done, shift-E) in the real core. The messages come back to the inbox: one the
//! owner archived by the flag loses the flag; one that sits in its account's Archive folder is put in INBOX (store/src/delta10.rs
//! `unarchive`). Which folders are Archive folders is what the server named at its last LIST (`reads::folders_with_role`), the rule
//! the Archive mailbox view already uses.
//!
//! Delta 19: THE SERVER'S COPY IS MOVED, through the write-back ledger engine-act built (the actions crate's own `Ledger`, in the same
//! transaction as the local change): a message that sat in an Archive folder gets a `move_to_folder` row to INBOX, a flag-archived one a
//! `set_archived` row with `archived: false`. The rows are read BEFORE the store clears the UID of the moved ones (the UID belongs to the
//! Archive folder). A message with no UID (imported, never synced) has no server copy and gets no row (`writeback::pending_rows`).
//! The command also records ONE undo entry (`UndoAction::Unarchive`): Z archives the messages again.
use actions::{MessageRef, WriteBack, WriteBackLedger};
use contracts::{CommandOutcome, CoreError, UndoAction};
use store::Store;
use sync_imap::FolderRole;

use crate::imap_source::Roles;
use crate::reads::folders_with_role;
use crate::reminders::store_failed;
use crate::writeback::Ledger;

/// `Unarchive`. An unknown id is refused (`NotFound`) before anything is written. A message that is neither archived by the flag nor in an
/// Archive folder is left as it is: it gets no row and joins no undo entry.
pub(crate) fn run(store: &Store, roles: &Roles, message_ids: Vec<String>) -> Result<CommandOutcome, CoreError> {
    run_recording(store, roles, message_ids, true)
}

/// `run`, and `record_undo` false for a return the owner did not ask for (a fired reminder brings a thread back, engine/src/reminders.rs
/// `fire_due_and_return`): the same moves and server write-backs, but no undo entry, so Z never archives a thread the owner did not put away
/// by that action (fix-core, gap-remind finding F2).
pub(crate) fn run_recording(store: &Store, roles: &Roles, message_ids: Vec<String>, record_undo: bool) -> Result<CommandOutcome, CoreError> {
    for id in &message_ids {
        if store.message_account(id).map_err(|e| store_failed("the message could not be read", e))?.is_none() {
            return Err(CoreError::NotFound { kind: "message".into(), id: id.clone() });
        }
    }
    let accounts: Vec<String> = store.account_summaries().map_err(|e| store_failed("the accounts could not be read", e))?.into_iter().map(|a| a.id).collect();
    let archive = folders_with_role(store, roles, &accounts, FolderRole::Archive);
    let now = crate::rust_core::now_ms();
    let message_ids = {
        let mut seen = std::collections::HashSet::new();
        message_ids.into_iter().filter(|id| seen.insert(id.clone())).collect::<Vec<_>>()
    };
    let entry = store
        .write(move |ctx| {
            // what the server must hear, read before the store clears the moved messages' UIDs
            let mut moved: Vec<(String, MessageRef)> = Vec::new();
            let mut flagged: Vec<(String, MessageRef)> = Vec::new();
            for id in &message_ids {
                let row = ctx.tx.query_row("SELECT account_id, thread_id, remote_id, folder, archived FROM message WHERE id = ?1", [id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)? != 0))
                })?;
                let (account, thread, remote, folder, archived) = row;
                let reference = MessageRef { message_id: id.clone(), thread_id: thread, remote_id: remote, folder: folder.clone() };
                // fix-realcore (main92 s2-008.log `unarchive-moved-the-message-back-to-the-servers-inbox`): once the sync heard the INBOX UID
                // vanish it drops it from an archived row whose server place the move recorded (sync.rs `keep_archived`), so the row has no
                // UID and the write-back ledger made no row: the server never heard ⇧E. The way back names the recorded place (the Archive
                // copy and its UID) and moves it to INBOX.
                let place = ctx.place_of(id)?;
                let at_server = place.as_ref().map(|p| p.folder.clone()).unwrap_or_else(|| folder.clone());
                let reference = match place {
                    Some(p) => MessageRef { folder: p.folder, remote_id: Some(p.uid.to_string()), ..reference },
                    None => reference,
                };
                if archive.iter().any(|(a, f)| *a == account && (*f == folder || *f == at_server)) {
                    moved.push((account, reference));
                } else if archived {
                    flagged.push((account, reference));
                }
            }
            let changed = ctx.unarchive(&message_ids, &archive)?;
            let mut accounts_seen: Vec<String> = moved.iter().chain(&flagged).map(|(a, _)| a.clone()).collect();
            accounts_seen.sort();
            accounts_seen.dedup();
            for account in accounts_seen {
                let of = |list: &[(String, MessageRef)]| -> Vec<MessageRef> { list.iter().filter(|(a, _)| *a == account).map(|(_, m)| m.clone()).collect() };
                let (to_inbox, to_unflag) = (of(&moved), of(&flagged));
                if !to_inbox.is_empty() {
                    Ledger.execute(ctx, &WriteBack::MoveToFolder { account_id: account.clone(), messages: to_inbox, folder: store::INBOX_FOLDER.to_string() }, now)?;
                }
                if !to_unflag.is_empty() {
                    Ledger.execute(ctx, &WriteBack::SetArchived { account_id: account.clone(), messages: to_unflag, archived: false }, now)?;
                }
            }
            if changed.is_empty() || !record_undo {
                return Ok(None);
            }
            Ok(Some(actions::usecases::undo::push_entry(ctx, &UndoAction::Unarchive { message_ids: changed }, now)?))
        })
        .map_err(|e| store_failed("the messages could not be moved back to the inbox", e))?;
    Ok(CommandOutcome { undo_entry_id: entry, created_id: None })
}
