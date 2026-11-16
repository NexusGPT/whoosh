//! The owner's Retry on a failed entry. Delivery rules (including "never twice") live in deliver_entry.
use super::deliver_entry::{self, Outcome};
use crate::ctx::Ctx;
use contracts::CoreError;

pub(crate) fn execute(ctx: &mut Ctx, id: &str) -> Result<Outcome, CoreError> {
    match deliver_entry::execute(ctx, id)? {
        Outcome::Skipped(why) => Err(CoreError::Invalid { field: "outbox_id".into(), reason_words: why.into() }),
        other => Ok(other),
    }
}
