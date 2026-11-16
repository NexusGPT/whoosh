//! The threading headers of a message being queued: which message it answers (a reply, from the draft's
//! thread) or forwards (the thread the owner forwarded from). Resolved once, at queue time, and kept in the
//! entry, so a retry, an offline wait and a second device all send the same headers.
use crate::ctx::Ctx;
use contracts::{CoreError, Draft};
use send::{ReplyContext, ThreadKind};

pub(crate) fn execute(ctx: &Ctx, draft: &Draft, forward_of_thread: Option<&str>) -> Result<Option<ReplyContext>, CoreError> {
    let (thread_id, kind) = match (forward_of_thread, draft.reply_to_thread_id.as_deref()) {
        (Some(thread), _) => (thread, ThreadKind::Forward),
        (None, Some(thread)) => (thread, ThreadKind::Reply),
        (None, None) => return Ok(None),
    };
    // A reply sent without its headers lands outside its conversation for the recipient, so a failure here
    // stops the send with words instead of sending an unthreaded message.
    match ctx.ports.reply_context.execute(thread_id, kind) {
        Ok(found) => Ok(Some(found)),
        Err(CoreError::Action { failure }) => Err(CoreError::Invalid { field: "reply_to_thread_id".into(), reason_words: format!("the message you reply to could not be read, so the reply cannot be threaded yet: {}", failure.reason_words) }),
        Err(other) => Err(other),
    }
}
