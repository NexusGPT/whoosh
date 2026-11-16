//! Spam: the message leaves the inbox and sits in the Spam folder. NotSpam (delta 19) is the reverse: the message leaves Spam and comes
//! back to the inbox; the junk state it carried is cleared. The junk LESSON is the engine's (it reads the text where it is, before this runs).
use super::trash::move_to_folder;
use crate::engine::{core_first, groups_by_account, load, revert_fn, Applied, Env, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError};
use std::collections::HashMap;
use store::WriteCtx;

pub struct Spam {
    env: Env,
}

/// The messages named that carry the local Spam word go back to INBOX. A message in any other folder is left as it is.
/// engine-act-r5: MarkSpam's write-back moved the server copy to the Junk folder and the write-back worker recorded where (the server's
/// COPYUID answer, `store::ServerPlace`). A message with a recorded place gets a `MoveToFolder` INBOX group, and loses the server id of
/// its old INBOX copy (UIDs belong to a folder; the worker sets the new INBOX UID when the move is applied). A message with no recorded
/// place has no known copy in Junk (its move is still waiting, or the server named no place): no group, as before. A row the sync stored
/// under the server's Junk folder NAME is moved by the engine (`MarkNotSpam` arm), which knows the account's roles.
pub(crate) fn restore_from_spam(ctx: &mut WriteCtx<'_>, ids: &[String]) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let spam = crate::constants::SPAM_FOLDER;
    let changed: Vec<_> = rows.into_iter().filter(|r| r.folder == spam).collect();
    if changed.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let prior: HashMap<String, String> = changed.iter().map(|r| (r.id.clone(), r.folder.clone())).collect();
    let changed_ids: Vec<String> = changed.iter().map(|r| r.id.clone()).collect();
    let mut placed = Vec::new();
    for r in &changed {
        if ctx.place_of(&r.id)?.is_some() {
            placed.push(r.clone());
        }
    }
    let placed_ids: Vec<String> = placed.iter().map(|r| r.id.clone()).collect();
    ctx.clear_remote_ids(&placed_ids)?;
    ctx.set_folder(&changed_ids, store::INBOX_FOLDER)?;
    for id in &changed_ids {
        ctx.junk_clear_state(id)?;
    }
    Ok(Ok(Applied {
        undo: None,
        groups: groups_by_account(&placed, |a, m| WriteBack::MoveToFolder { account_id: a.into(), messages: m, folder: store::INBOX_FOLDER.to_string() }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<(String, String)> = ids.iter().filter_map(|i| prior.get(i).map(|f| (i.clone(), f.clone()))).collect();
            ctx.set_folders(&back)?;
            Ok(())
        }),
        created_id: None,
    }))
}

impl Spam {
    pub fn new(env: Env) -> Spam {
        Spam { env }
    }

    pub fn execute(&self, message_ids: Vec<String>) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, _| move_to_folder(ctx, &message_ids, crate::constants::SPAM_FOLDER))
    }

    /// `MarkNotSpam`: the messages leave Spam for the inbox.
    pub fn not_spam(&self, message_ids: Vec<String>) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, _| restore_from_spam(ctx, &message_ids))
    }
}
