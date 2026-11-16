//! Mark read or unread. Opening a thread and marking read write no undo entry (docs/CONTROL-SOCKET.md "## Undo").
use crate::engine::{core_first, groups_by_account, load, revert_fn, set_flag, Applied, Env, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError};
use std::collections::HashSet;
use store::WriteCtx;

pub(crate) fn apply(ctx: &mut WriteCtx<'_>, ids: &[String], read: bool) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let changed: Vec<_> = rows.into_iter().filter(|r| r.read != read).collect();
    if changed.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let changed_ids: Vec<String> = changed.iter().map(|r| r.id.clone()).collect();
    ctx.will_change(&changed_ids)?;
    set_flag(ctx, "read", &changed_ids, read)?;
    let set: HashSet<String> = changed_ids.iter().cloned().collect();
    Ok(Ok(Applied {
        undo: None,
        groups: groups_by_account(&changed, |a, m| WriteBack::SetRead { account_id: a.into(), messages: m, read }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<String> = ids.iter().filter(|i| set.contains(*i)).cloned().collect();
            ctx.will_change(&back)?;
            set_flag(ctx, "read", &back, !read)
        }),
        created_id: None,
    }))
}

pub struct MarkRead {
    env: Env,
}

impl MarkRead {
    pub fn new(env: Env) -> MarkRead {
        MarkRead { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, read: bool) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, _| apply(ctx, &message_ids, read))
    }
}
