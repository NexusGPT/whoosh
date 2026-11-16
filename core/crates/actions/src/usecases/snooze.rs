//! Snooze: the row leaves the inbox until its return time. The words "tomorrow morning" and its siblings become a
//! time with the SNOOZE_* row of docs/CONSTANTS.md (constants.rs parses it).
use crate::constants::SnoozeRule;
use crate::engine::{core_first, groups_by_account, load, revert_fn, Applied, Env, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError, SnoozeWhen, UndoAction};
use std::collections::HashMap;
use store::WriteCtx;

/// Set (Some) or lift (None) the snooze of the named messages.
pub(crate) fn apply(ctx: &mut WriteCtx<'_>, ids: &[String], return_at: Option<i64>, now: i64, with_undo: bool) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    let mut prior: HashMap<String, Option<i64>> = HashMap::new();
    for r in &rows {
        prior.insert(r.id.clone(), ctx.snoozed_until(&r.id)?);
    }
    let changed: Vec<_> = rows.into_iter().filter(|r| prior[&r.id] != return_at).collect();
    if changed.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let changed_ids: Vec<String> = changed.iter().map(|r| r.id.clone()).collect();
    for id in &changed_ids {
        ctx.set_snoozed_until(id, return_at, now)?;
    }
    let prior: HashMap<String, Option<i64>> = changed_ids.iter().map(|i| (i.clone(), prior[i])).collect();
    Ok(Ok(Applied {
        undo: (with_undo && return_at.is_some()).then(|| UndoAction::Snooze { message_ids: changed_ids.clone() }),
        groups: groups_by_account(&changed, |a, m| WriteBack::Snooze { account_id: a.into(), messages: m, return_at_ms: return_at }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<String> = ids.iter().filter(|i| prior.contains_key(*i)).cloned().collect();
            for id in &back {
                ctx.set_snoozed_until(id, prior[id], now)?;
            }
            Ok(())
        }),
        created_id: None,
    }))
}

pub struct Snooze {
    env: Env,
}

impl Snooze {
    pub fn new(env: Env) -> Snooze {
        Snooze { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, when: SnoozeWhen) -> Result<CommandOutcome, CoreError> {
        let at = SnoozeRule::from_ledger().resolve(&when, &*self.env.clock);
        if at <= self.env.now() {
            return Err(CoreError::Invalid { field: "when".into(), reason_words: "the return time is not in the future".into() });
        }
        core_first(&self.env, move |ctx, now| apply(ctx, &message_ids, Some(at), now, true))
    }
}
