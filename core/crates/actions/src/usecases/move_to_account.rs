//! Move messages to another account. The core moves them first; the adapter copies into the target and deletes from the
//! source only after the target accepted. A refusal puts every row back exactly as it was.
use crate::engine::{core_first, load, revert_fn, Applied, Env, Group, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError, UndoAction};
use store::WriteCtx;

pub(crate) fn apply(ctx: &mut WriteCtx<'_>, ids: &[String], target: &str, with_undo: bool) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    if !ctx.account_exists(target)? {
        return Ok(Err(CoreError::NotFound { kind: "account".into(), id: target.into() }));
    }
    let moving: Vec<_> = rows.into_iter().filter(|r| r.account_id != target).collect();
    if moving.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let source = moving[0].account_id.clone();
    if moving.iter().any(|r| r.account_id != source) {
        return Ok(Err(CoreError::Invalid { field: "message_ids".into(), reason_words: "move messages from one account at a time".into() }));
    }
    let moved = ctx.move_to_account(&moving.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), target)?;
    let moved_ids = moved.moved_ids();
    let dst = target.to_string();
    Ok(Ok(Applied {
        undo: with_undo.then(|| UndoAction::Move { message_ids: moved_ids.clone(), from_account_id: source.clone() }),
        groups: vec![Group { request: WriteBack::Move { from_account_id: source, to_account_id: dst, messages: moving.iter().map(|r| r.as_ref()).collect() }, message_ids: moved_ids }],
        revert: revert_fn(move |ctx, ids| {
            ctx.restore_account_move(&moved, ids)?;
            Ok(())
        }),
        created_id: None,
    }))
}

pub struct MoveToAccount {
    env: Env,
}

impl MoveToAccount {
    pub fn new(env: Env) -> MoveToAccount {
        MoveToAccount { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, account_id: String) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, _| apply(ctx, &message_ids, &account_id, true))
    }
}
