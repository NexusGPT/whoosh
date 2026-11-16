//! "Send": the draft leaves the composer and waits in the outbox for the undo window.
use super::resolve_reply;
use crate::ctx::Ctx;
use crate::domain::Record;
use crate::ports::DraftEffect;
use contracts::{CoreError, Draft};

/// How a message is queued. `key` is the caller's idempotency key (a retried Send carries the key of the first);
/// without one a fresh key is minted. `forward_of` is the thread being forwarded.
#[derive(Debug, Clone, Default)]
pub(crate) struct Enqueue {
    pub key: Option<String>,
    pub forward_of: Option<String>,
}

/// Refuse a draft that cannot be sent, or that already waits in the outbox (a double tap of Send must not
/// make two messages).
pub(crate) fn check_sendable(ctx: &Ctx, draft: &Draft) -> Result<(), CoreError> {
    if draft.to.is_empty() {
        return Err(CoreError::Invalid { field: "to".into(), reason_words: "add a recipient before sending".into() });
    }
    if ctx.ports.list_open.execute()?.iter().any(|r| r.entry.draft.id == draft.id) {
        return Err(CoreError::Invalid { field: "draft_id".into(), reason_words: "this message is already in the outbox".into() });
    }
    Ok(())
}

/// The entry this key already made, if any. A retried Send finds the first one instead of making a second.
/// A cancelled entry never left, so its key may be used again (undo, edit, send again).
pub(crate) fn existing(ctx: &Ctx, key: &str) -> Result<Option<Record>, CoreError> {
    Ok(ctx.ports.find_by_key.execute(key)?.into_iter().find(|r| !matches!(r.entry.state, contracts::OutboxState::Cancelled)))
}

pub(crate) fn execute(ctx: &mut Ctx, draft: Draft, how: Enqueue) -> Result<Record, CoreError> {
    if let Some(found) = how.key.as_deref().map(|k| existing(ctx, k)).transpose()?.flatten() {
        return Ok(found);
    }
    check_sendable(ctx, &draft)?;
    let threading = resolve_reply::execute(ctx, &draft, how.forward_of.as_deref())?;
    let key = how.key.unwrap_or_else(|| ctx.new_key());
    let now = ctx.now();
    let id = ctx.new_id(&draft.id, now);
    let window_ms = i64::from(ctx.cfg.undo_window_s()) * 1000;
    let draft_id = draft.id.clone();
    let record = Record::undoable(id, draft, now, window_ms, key, threading);
    ctx.commit(&record, DraftEffect::Remove { draft_id }, false)?;
    Ok(record)
}
