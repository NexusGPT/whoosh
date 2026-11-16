//! At start: an entry stored as Sending has an unknown outcome (the app closed between the ledger and the
//! receipt). It is never sent again by itself: it becomes Failed with words, and the owner decides.
use crate::ctx::Ctx;
use crate::ports::DraftEffect;
use contracts::{ActionFailed, CoreError, OutboxState};

pub(crate) fn execute(ctx: &mut Ctx) -> Result<usize, CoreError> {
    let mut n = 0;
    for mut record in ctx.ports.list_open.execute()? {
        if record.entry.state != OutboxState::Sending {
            continue;
        }
        let account_name = ctx.ports.account_names.execute(&record.entry.draft.from_account_id);
        let failure = ActionFailed {
            account_name,
            reason_words: "Whoosh closed while this was being sent; look in Sent before you send it again".into(),
            retryable: true,
            code: None,
        };
        let now = ctx.now();
        record.mark_failed(failure, false, now);
        ctx.commit(&record, DraftEffect::Keep, false)?;
        n += 1;
    }
    Ok(n)
}
