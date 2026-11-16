//! What a tick sends: undo windows that elapsed, scheduled times that came (by this device's role),
//! and entries that waited for the network.
use super::deliver_entry::{self, Outcome};
use crate::config::DeviceRole;
use crate::ctx::Ctx;
use crate::outbox::TickReport;
use contracts::{CoreError, OutboxState};

pub(crate) fn execute(ctx: &mut Ctx, report: &mut TickReport) -> Result<(), CoreError> {
    let now = ctx.now();
    for record in ctx.ports.list_open.execute()? {
        let due = match record.entry.state {
            OutboxState::Undoable { until_ms } => now >= until_ms,
            OutboxState::Scheduled { at_ms } => match ctx.cfg.role {
                DeviceRole::Primary => now >= at_ms,
                DeviceRole::Standby { after_ms } => now >= at_ms + after_ms,
            },
            // fix-realcore round 15: a broken connection's next try is due at its time
            OutboxState::Failed { .. } => record.waiting || record.retry_at_ms.is_some_and(|t| now >= t),
            // root delta 4 (2026-11-07): the contract gained Late and WaitingForNetwork; this crate does not produce them yet.
            OutboxState::Late { .. } | OutboxState::WaitingForNetwork { .. } => true,
            OutboxState::Sending | OutboxState::Sent { .. } | OutboxState::Cancelled => false,
        };
        if !due {
            continue;
        }
        match deliver_entry::execute(ctx, record.id())? {
            Outcome::Delivered => report.delivered.push(record.id().to_string()),
            Outcome::Waiting => report.waiting += 1,
            Outcome::Failed => report.failed.push(record.id().to_string()),
            Outcome::Skipped(_) => {}
        }
    }
    Ok(())
}
