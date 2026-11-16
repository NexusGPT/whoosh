//! Engine-actions: `Mute { message_ids, muted }` in the real core. The thread follows its messages: every message of the named
//! messages' threads gets the stored `muted` flag (or loses it), so the thread leaves every inbox list and count and the Muted
//! view (`Mailbox::Muted`) holds it. The next message that ARRIVES in a muted thread arrives read and muted: the store's insert
//! applies the contract's `arrival_in_thread` (store/src/ops.rs `apply_muted_arrival`), so the sync needs no code of its own.
//! State read: `message/<id>/muted` ("true" or "false"), the fixture core's word.
use contracts::{CommandOutcome, CoreError, UndoAction};
use store::Store;

use crate::reminders::store_failed;

/// `Mute` and `Unmute`. An id that names no message is refused (`NotFound`), as the fixture core does; nothing is written then. Delta 19:
/// the command records ONE undo entry (`UndoAction::Mute`): the ids of every message of the threads and which of them were muted BEFORE,
/// so Z puts each message's earlier flag back (the same entry undoes a mute and an unmute). No entry when no message changed.
pub(crate) fn run(store: &Store, message_ids: Vec<String>, muted: bool) -> Result<CommandOutcome, CoreError> {
    for id in &message_ids {
        if store.message_account(id).map_err(|e| store_failed("the message could not be read", e))?.is_none() {
            return Err(CoreError::NotFound { kind: "message".into(), id: id.clone() });
        }
    }
    let now = crate::rust_core::now_ms();
    let entry = store
        .write(move |ctx| {
            let members = ctx.thread_member_ids(&message_ids)?;
            let mut was_muted = Vec::new();
            for id in &members {
                if ctx.tx.query_row("SELECT muted FROM message WHERE id = ?1", [id], |r| r.get::<_, i64>(0))? != 0 {
                    was_muted.push(id.clone());
                }
            }
            let changed = if muted { members.len() != was_muted.len() } else { !was_muted.is_empty() };
            ctx.mute_threads(&message_ids, muted)?;
            if !changed {
                return Ok(None);
            }
            Ok(Some(actions::usecases::undo::push_entry(ctx, &UndoAction::Mute { message_ids: members, was_muted }, now)?))
        })
        .map_err(|e| store_failed("the mute could not be stored", e))?;
    Ok(CommandOutcome { undo_entry_id: entry, created_id: None })
}

/// `message/<id>/muted`; None for any other path (and for a message that is not stored).
pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        ["message", id, "muted"] => store.is_muted(id).ok().flatten().map(|m| m.to_string()),
        // delta 19: `message/<id>/spam`, "true" while the message carries the local Spam word (MarkSpam sets it, MarkNotSpam takes it away)
        ["message", id, "spam"] => store.message(id).ok().flatten().map(|m| (m.folder == actions::constants::SPAM_FOLDER).to_string()),
        _ => None,
    }
}
