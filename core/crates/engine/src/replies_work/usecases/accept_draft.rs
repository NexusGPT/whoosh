//! Keep (J10.8 to J10.10): the draft's text becomes an ordinary composer draft through the SaveDraft path (to the person the reply answers, "Re: "
//! the subject, the reply's account and thread), then the reply draft reads Edited (`reply_draft_after_accept`, against the row as it is now) and
//! `ReplyDraftChanged`. Two local writes, on the command's thread. Nothing is sent.
use contracts::{prefixed_subject, CoreError, ReplyDraft};

use super::announce_changed;
use crate::replies_work::Shared;
use crate::rust_core::now_ms;

/// Returns the composer draft's id.
pub(crate) fn run(s: &Shared, draft: ReplyDraft) -> Result<String, CoreError> {
    let message = s.store.message(&draft.message_id)?.ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: draft.message_id.clone() })?;
    let own: Vec<String> = s.store.account_summaries()?.into_iter().map(|a| a.address.to_ascii_lowercase()).collect();
    let to = if own.contains(&message.from_address.to_ascii_lowercase()) { message.to.clone() } else { vec![message.from_address.clone()] };
    let subject = prefixed_subject(message.subject.clone(), "Re".into());
    let out = actions::SaveDraft::new(s.env.clone()).execute(Some(message.account_id.clone()), to, vec![], subject, draft.text.clone(), Some(draft.thread_id.clone()))?;
    let composer = out.created_id.ok_or_else(|| CoreError::Unexpected { reason_words: "the composer draft was saved without an id".into() })?;
    let taken = s.store.reply_draft_accept(draft.id.clone(), now_ms())?;
    announce_changed(s, &taken);
    Ok(composer)
}
