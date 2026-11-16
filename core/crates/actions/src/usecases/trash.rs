//! Trash: the message leaves the inbox and sits in the Trash folder. The sync lanes write the folder word TRASH_FOLDER.
use crate::engine::{core_first, groups_by_account, load, revert_fn, Applied, Env, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError};
use std::collections::HashMap;
use store::WriteCtx;

/// Put the messages in `folder` (Trash or Spam). The write-back carries the folder each came from.
pub(crate) fn move_to_folder(ctx: &mut WriteCtx<'_>, ids: &[String], folder: &str) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let changed: Vec<_> = rows.into_iter().filter(|r| r.folder != folder).collect();
    if changed.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let prior: HashMap<String, String> = changed.iter().map(|r| (r.id.clone(), r.folder.clone())).collect();
    let changed_ids: Vec<String> = changed.iter().map(|r| r.id.clone()).collect();
    ctx.set_folder(&changed_ids, folder)?;
    let target = folder.to_string();
    // a delete (to Trash) is ONE undo entry for the whole selection (contracts delta 10); Spam keeps its own path
    // each message's origin (folder, archived mark) rides the entry, so Z puts it back where it was (contracts-d11)
    let origins: Vec<contracts::MessageOrigin> = changed.iter().map(|r| contracts::MessageOrigin { message_id: r.id.clone(), folder: r.folder.clone(), archived: r.archived }).collect();
    let undo = (folder == crate::constants::TRASH_FOLDER).then(|| contracts::UndoAction::DeleteMessages { message_ids: changed_ids.clone(), origins });
    Ok(Ok(Applied {
        undo,
        groups: groups_by_account(&changed, |a, m| WriteBack::MoveToFolder { account_id: a.into(), messages: m, folder: target.clone() }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<(String, String)> = ids.iter().filter_map(|i| prior.get(i).map(|f| (i.clone(), f.clone()))).collect();
            ctx.set_folders(&back)?;
            Ok(())
        }),
        created_id: None,
    }))
}

/// Z after a delete: each message goes back to its recorded folder and archived mark (INBOX when no origin was recorded).
pub(crate) fn restore_origins(ctx: &mut WriteCtx<'_>, ids: &[String], origins: &[contracts::MessageOrigin]) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let prior: HashMap<String, (String, bool)> = rows.iter().map(|r| (r.id.clone(), (r.folder.clone(), r.archived))).collect();
    let back: Vec<(String, String)> = ids
        .iter()
        .map(|id| (id.clone(), origins.iter().find(|o| &o.message_id == id).map_or_else(|| store::INBOX_FOLDER.to_string(), |o| o.folder.clone())))
        .collect();
    ctx.set_folders(&back)?;
    let archived: Vec<String> = origins.iter().filter(|o| o.archived && ids.contains(&o.message_id)).map(|o| o.message_id.clone()).collect();
    if !archived.is_empty() {
        ctx.will_change(&archived)?;
        crate::engine::set_flag(ctx, "archived", &archived, true)?;
    }
    // the server is asked to move each message back to its own folder: one write-back per account and folder
    let mut folders: Vec<String> = back.iter().map(|(_, f)| f.clone()).collect();
    folders.sort();
    folders.dedup();
    let mut groups = Vec::new();
    for f in folders {
        let sub: Vec<_> = rows.iter().filter(|r| back.iter().any(|(i, g)| i == &r.id && g == &f)).cloned().collect();
        groups.extend(groups_by_account(&sub, |a, m| WriteBack::MoveToFolder { account_id: a.into(), messages: m, folder: f.clone() }));
    }
    Ok(Ok(Applied {
        undo: None,
        groups,
        revert: revert_fn(move |ctx, ids| {
            let again: Vec<(String, String)> = ids.iter().filter_map(|i| prior.get(i).map(|(f, _)| (i.clone(), f.clone()))).collect();
            ctx.set_folders(&again)?;
            Ok(())
        }),
        created_id: None,
    }))
}

pub struct Trash {
    env: Env,
}

impl Trash {
    pub fn new(env: Env) -> Trash {
        Trash { env }
    }

    pub fn execute(&self, message_ids: Vec<String>) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, _| move_to_folder(ctx, &message_ids, crate::constants::TRASH_FOLDER))
    }
}
