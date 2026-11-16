//! Throw away: the draft reads Discarded (`reply_draft_after_discard`, against the row as it is now) and `ReplyDraftChanged`. One local write, on the
//! command's thread. Sends nothing, changes no message.
use contracts::{CoreError, ReplyDraft};

use super::announce_changed;
use crate::replies_work::Shared;
use crate::rust_core::now_ms;

pub(crate) fn run(s: &Shared, draft: ReplyDraft) -> Result<ReplyDraft, CoreError> {
    let gone = s.store.reply_draft_discard(draft.id, now_ms())?;
    announce_changed(s, &gone);
    Ok(gone)
}
