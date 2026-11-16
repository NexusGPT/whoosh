//! Take in what the other devices did to scheduled entries. Never publishes (no echo).
//! A record replaces the local one only if it is further along (rank), then if newer.
use crate::ctx::Ctx;
use crate::ports::DraftEffect;
use contracts::{CoreError, OutboxState};

pub(crate) fn execute(ctx: &mut Ctx) -> Result<usize, CoreError> {
    let Some(sync) = &ctx.ports.sync else { return Ok(0) };
    let remote = sync.pull.execute()?;
    let mut taken = 0;
    for incoming in remote {
        let take = match ctx.ports.get.execute(incoming.id())? {
            Some(local) => local.replaced_by(&incoming),
            // An entry this device has never seen: keep it if it can still matter or is history.
            None => matches!(incoming.entry.state, OutboxState::Scheduled { .. } | OutboxState::Sent { .. } | OutboxState::Cancelled),
        };
        if take {
            ctx.commit(&incoming, DraftEffect::Keep, false)?;
            taken += 1;
        }
    }
    Ok(taken)
}
