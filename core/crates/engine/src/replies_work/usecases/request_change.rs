//! The field under a draft (J10.10), on the interactive thread: the draft's context assembled again (the same message, use, instruction and prompt),
//! the current text and the owner's change, ONE call, then the row rewritten through `reply_draft_after_change` against the row as it is NOW (a draft
//! discarded while the model wrote stays discarded) and `ReplyDraftChanged`, revision + 1. A failure leaves the draft as it was.
use contracts::{CoreError, ReplyDraft, ReplyMadeBy, ReplyUseKind};

use super::write_draft::{assemble, change_prompt, write, Ports};
use super::{announce_changed, words};
use crate::replies_work::adapters::excerpt_index::Index;
use crate::replies_work::adapters::mail_reads::MailReads;
use crate::replies_work::limits::SHARE_THREAD;
use crate::replies_work::model::TextModel;
use crate::replies_work::Shared;
use crate::rust_core::now_ms;

fn rewrite(s: &Shared, draft: &ReplyDraft, change: &str, model: &dyn TextModel) -> Result<ReplyDraft, CoreError> {
    let reads = MailReads::open(&s.store)?;
    let f = reads.facts(&draft.message_id, model.prompt_chars() / SHARE_THREAD)?.ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: draft.message_id.clone() })?;
    let index = Index { store: &s.store, bodies: s.bodies.upgrade(), memory: &s.memory };
    let kind = if draft.made_by == ReplyMadeBy::Auto { ReplyUseKind::Auto } else { ReplyUseKind::OnDemand };
    let assembled = assemble(&Ports { store: &s.store, index: &index, sent: &reads, model, memory: &s.memory }, kind, &f, draft.instruction.as_deref())?;
    let text = write(model, &change_prompt(model, &assembled.prompt, &draft.text, change))?;
    s.store.reply_draft_rewrite(draft.id.clone(), text, now_ms())
}

pub(crate) fn run(s: &Shared, draft: ReplyDraft, change: &str, model: &dyn TextModel) {
    match rewrite(s, &draft, change, model) {
        Ok(next) => announce_changed(s, &next),
        Err(e) => s.log(&format!("the change asked of the draft {} was not made: {}", draft.id, words(&e))),
    }
}
