//! Star or unstar. The message's `starred` column (migration 0005) is the truth every list reads (the ⇧S filter's facts, the Starred view,
//! the label flow's starred fact) and the one the sync writes from the server's `\Flagged`; the system label STARRED_LABEL is kept beside it.
//! The server hears the CHANGE of the column: one `WriteBack::Label` STARRED_LABEL group (engine/src/writeback.rs turns it into `\Flagged`)
//! for the messages whose column changed, so a message the server starred (column set, no label) is unstarred on the server too, and a
//! message already starred sends nothing again. No undo entry.
//!
//! engine-act-r6 round 6: Star used to assign the label only, so a message starred with S was missing from ⇧S and the Starred view until a
//! resync brought `\Flagged` back, and the server heard nothing when a message it had starred was unstarred (no label to remove).
use std::collections::HashSet;

use super::label::{assign, unassign_named};
use crate::constants::STARRED_LABEL;
use crate::engine::{core_first, groups_by_account, load, revert_fn, set_flag, Env};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError};

pub struct Star {
    env: Env,
}

impl Star {
    pub fn new(env: Env) -> Star {
        Star { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, starred: bool) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, now| {
            let rows = match load(ctx, &message_ids)? {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            let mut changed = Vec::new();
            {
                let mut st = ctx.tx.prepare_cached("SELECT starred FROM message WHERE id = ?1")?;
                for r in &rows {
                    if (st.query_row([&r.id], |x| x.get::<_, i64>(0))? != 0) != starred { changed.push(r.clone()); }
                }
            }
            let changed_ids: Vec<String> = changed.iter().map(|r| r.id.clone()).collect();
            ctx.will_change(&changed_ids)?;   // the Starred view and the filters are told, as archive does
            set_flag(ctx, "starred", &changed_ids, starred)?;
            let label = if starred { assign(ctx, &rows, STARRED_LABEL, true, now, false)? } else { unassign_named(ctx, &rows, STARRED_LABEL, now)? };
            let mut applied = match label { Ok(a) => a, Err(e) => return Ok(Err(e)) };
            applied.groups = groups_by_account(&changed, |a, m| WriteBack::Label { account_id: a.into(), messages: m, label_name: STARRED_LABEL.to_string(), applied: starred });
            let (label_back, set): (_, HashSet<String>) = (applied.revert.clone(), changed_ids.into_iter().collect());
            applied.revert = revert_fn(move |ctx, ids| {
                label_back(ctx, ids)?;
                let back: Vec<String> = ids.iter().filter(|i| set.contains(*i)).cloned().collect();
                set_flag(ctx, "starred", &back, !starred)
            });
            Ok(Ok(applied))
        })
    }
}
