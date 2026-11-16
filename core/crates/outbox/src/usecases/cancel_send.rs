//! Undo inside the window, or cancel a scheduled, waiting or failed entry. The draft goes back to editing.
use crate::ctx::Ctx;
use crate::domain::Record;
use crate::ports::DraftEffect;
use contracts::{CoreError, OutboxState};

fn refuse(why: &str) -> CoreError {
    CoreError::Invalid { field: "outbox_id".into(), reason_words: why.into() }
}

pub(crate) fn execute(ctx: &mut Ctx, id: &str) -> Result<Record, CoreError> {
    let mut record = ctx.get(id)?;
    let now = ctx.now();
    match record.entry.state {
        OutboxState::Cancelled => return Ok(record),
        OutboxState::Sent { .. } => return Err(refuse("this message was already sent")),
        OutboxState::Sending => return Err(refuse("this message is being sent now")),
        OutboxState::Undoable { until_ms } if now >= until_ms => return Err(refuse("the undo window is over")),
        OutboxState::Undoable { .. } | OutboxState::Scheduled { .. } | OutboxState::Failed { .. } => {}
        // root delta 4 (2026-11-07): the contract gained Late and WaitingForNetwork; this crate does not produce them yet.
        OutboxState::Late { .. } | OutboxState::WaitingForNetwork { .. } => {}
    }
    record.mark_cancelled(now);
    let draft = record.entry.draft.clone();
    ctx.commit(&record, DraftEffect::Restore(draft), true)?;
    Ok(record)
}
