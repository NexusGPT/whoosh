//! Pin or unpin one message. Pinned rows sit first in their tab, newest pin first.
use crate::engine::{core_first, revert_fn, Applied, Env, Group, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError, UndoAction};
use store::WriteCtx;

pub(crate) fn apply(ctx: &mut WriteCtx<'_>, message_id: &str, pinned: bool, now: i64, with_undo: bool) -> Step {
    let ids = vec![message_id.to_string()];
    let rows = match crate::engine::load(ctx, &ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let row = rows[0].clone();
    let prior: Option<i64> = ctx.pinned_at(message_id)?;
    if prior.is_some() == pinned {
        return Ok(Ok(Applied::nothing()));
    }
    ctx.set_pinned_at(message_id, pinned.then_some(now))?;
    let id = message_id.to_string();
    Ok(Ok(Applied {
        undo: with_undo.then(|| UndoAction::Pin { message_id: id.clone(), was_pinned: prior.is_some() }),
        groups: vec![Group { request: WriteBack::Pin { account_id: row.account_id.clone(), message: row.as_ref(), pinned }, message_ids: ids }],
        revert: revert_fn(move |ctx, ids| {
            if !ids.contains(&id) {
                return Ok(());
            }
            ctx.set_pinned_at(&id, prior)
        }),
        created_id: None,
    }))
}

pub struct Pin {
    env: Env,
}

impl Pin {
    pub fn new(env: Env) -> Pin {
        Pin { env }
    }

    pub fn execute(&self, message_id: String, pinned: bool) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, now| apply(ctx, &message_id, pinned, now, true))
    }
}
