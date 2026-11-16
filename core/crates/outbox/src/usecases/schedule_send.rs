//! "Send later": the entry waits for its time and travels to the other devices through sync.
use super::queue_send::{check_sendable, existing, Enqueue};
use super::resolve_reply;
use crate::ctx::Ctx;
use crate::domain::Record;
use crate::ports::DraftEffect;
use contracts::{CoreError, Draft};

pub(crate) fn execute(ctx: &mut Ctx, draft: Draft, at_ms: i64, how: Enqueue) -> Result<Record, CoreError> {
    if let Some(found) = how.key.as_deref().map(|k| existing(ctx, k)).transpose()?.flatten() {
        return Ok(found);
    }
    let now = ctx.now();
    if at_ms <= now {
        return Err(CoreError::Invalid { field: "at_ms".into(), reason_words: "pick a time in the future".into() });
    }
    check_sendable(ctx, &draft)?;
    let threading = resolve_reply::execute(ctx, &draft, how.forward_of.as_deref())?;
    let key = how.key.unwrap_or_else(|| ctx.new_key());
    let id = ctx.new_id(&draft.id, now);
    let draft_id = draft.id.clone();
    let record = Record::scheduled(id, draft, now, at_ms, key, threading);
    ctx.commit(&record, DraftEffect::Remove { draft_id }, true)?;
    Ok(record)
}
