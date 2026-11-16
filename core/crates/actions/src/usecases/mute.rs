//! Mute a conversation: every message of the thread gets the system label MUTED_LABEL and leaves the inbox. Unmute lifts
//! the label only; the thread stays archived. The sync engine keeps new mail of a muted thread out of the inbox by that label.
use super::{archive, label};
use crate::constants::MUTED_LABEL;
use crate::engine::{core_first, load, Applied, Env, Step};
use contracts::{CommandOutcome, CoreError};
use store::WriteCtx;

pub struct Mute {
    env: Env,
}

impl Mute {
    pub fn new(env: Env) -> Mute {
        Mute { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, muted: bool) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, now| {
            if let Err(e) = load(ctx, &message_ids)? {
                return Ok(Err(e));
            }
            let members = ctx.thread_member_ids(&message_ids)?;
            let rows = match load(ctx, &members)? {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            if !muted {
                return label::unassign_named(ctx, &rows, MUTED_LABEL, now);
            }
            let labelled = match label::assign(ctx, &rows, MUTED_LABEL, true, now, false)? {
                Ok(a) => a,
                Err(e) => return Ok(Err(e)),
            };
            let archived = match archive::apply(ctx, &members, true, false)? {
                Ok(a) => a,
                Err(e) => return Ok(Err(e)),
            };
            Ok(Ok(Applied::merge(labelled, archived)))
        })
    }
}

/// Z after a mute or an unmute (delta 19): every message of `message_ids` loses the stored `muted` flag, then each id of `was_muted`
/// takes it back, so each message ends with the flag it had before the command. The real core's `Mute` (engine/src/mute.rs) records the
/// entry; the flag is the store's `muted` column, the one the inbox lists and counts read. No server state, so no write-back group.
pub(crate) fn reverse(ctx: &mut WriteCtx<'_>, message_ids: &[String], was_muted: &[String]) -> Step {
    ctx.set_muted(message_ids, false)?;
    ctx.set_muted(was_muted, true)?;
    Ok(Ok(Applied::nothing()))
}
