//! Archive: the row leaves the inbox at once; the server is told after. Z puts it back at the same place
//! (the inbox orders by time, so un-archiving restores the position).
use crate::engine::{core_first, groups_by_account, load, revert_fn, set_flag, Applied, Env, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError, UndoAction};
use std::collections::HashSet;
use store::WriteCtx;

pub(crate) fn apply(ctx: &mut WriteCtx<'_>, ids: &[String], archived: bool, with_undo: bool) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let changed: Vec<_> = rows.into_iter().filter(|r| r.archived != archived).collect();
    if changed.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let changed_ids: Vec<String> = changed.iter().map(|r| r.id.clone()).collect();
    ctx.will_change(&changed_ids)?;
    set_flag(ctx, "archived", &changed_ids, archived)?;
    let set: HashSet<String> = changed_ids.iter().cloned().collect();
    Ok(Ok(Applied {
        undo: (with_undo && archived).then(|| UndoAction::Archive { message_ids: changed_ids.clone() }),
        groups: groups_by_account(&changed, |a, m| WriteBack::SetArchived { account_id: a.into(), messages: m, archived }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<String> = ids.iter().filter(|i| set.contains(*i)).cloned().collect();
            ctx.will_change(&back)?;
            set_flag(ctx, "archived", &back, !archived)
        }),
        created_id: None,
    }))
}

pub struct Archive {
    env: Env,
}

impl Archive {
    pub fn new(env: Env) -> Archive {
        Archive { env }
    }

    pub fn execute(&self, message_ids: Vec<String>) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, _| apply(ctx, &message_ids, true, true))
    }
}
