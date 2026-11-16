//! A delivered message whose Sent copy was not saved: retry only the copy (never the delivery), and flush
//! records the other devices have not received yet. The copy and the reason live in the entry's row, so a
//! restart loses neither.
use crate::ctx::Ctx;
use crate::ports::DraftEffect;
use send::AppendSentRequest;

pub(crate) fn execute(ctx: &mut Ctx) -> usize {
    let mut stored = 0;
    for mut record in ctx.ports.pending_copies.execute().unwrap_or_default() {
        let Some(copy) = record.copy_pending.clone() else { continue };
        let request = AppendSentRequest { account_id: record.entry.draft.from_account_id.clone(), rfc822: copy.rfc822 };
        if ctx.ports.append_sent.execute(&request).is_ok() {
            record.copy_saved(ctx.now());
            if ctx.commit(&record, DraftEffect::Keep, false).is_ok() {
                stored += 1;
            }
        }
    }
    let waiting: Vec<_> = ctx.st.unpublished.values().cloned().collect();
    for record in waiting {
        ctx.publish(&record);
    }
    stored
}
