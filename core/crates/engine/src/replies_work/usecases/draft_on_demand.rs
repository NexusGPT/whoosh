//! ⌘J (J10.9), on the interactive thread: the thread's newest message (the newest one the owner did not write), retrieval with the instruction,
//! the voice of the on-demand use, its prompt, ONE call, then the draft stored under the id the command already answered, the thread's old live draft
//! marked Discarded in the same transaction (`Store::reply_draft_put`), and `ReplyDraftReady`. A failure stores nothing (no half draft) and is logged
//! in words.
use contracts::{ReplyDraft, ReplyDraftState, ReplyMadeBy, ReplyUseKind};

use super::write_draft::{assemble, excerpt_ids, write, Ports};
use super::{announce_changed, announce_ready, words};
use crate::replies_work::adapters::excerpt_index::Index;
use crate::replies_work::adapters::mail_reads::MailReads;
use crate::replies_work::limits::SHARE_THREAD;
use crate::replies_work::model::TextModel;
use crate::replies_work::Shared;
use crate::rust_core::now_ms;

/// A new draft for `message_id` of `thread_id`, written and stored (the auto use shares it, with its pass in the same transaction).
pub(crate) fn draft_for(s: &Shared, draft_id: &str, message_id: &str, kind: ReplyUseKind, instruction: Option<&str>, model: &dyn TextModel, pass: Option<contracts::ReplyPass>) -> Result<ReplyDraft, contracts::CoreError> {
    let reads = MailReads::open(&s.store)?;
    let f = reads.facts(message_id, model.prompt_chars() / SHARE_THREAD)?.ok_or_else(|| contracts::CoreError::NotFound { kind: "message".into(), id: message_id.to_string() })?;
    let index = Index { store: &s.store, bodies: s.bodies.upgrade(), memory: &s.memory };
    let ports = Ports { store: &s.store, index: &index, sent: &reads, model, memory: &s.memory };
    let assembled = assemble(&ports, kind, &f, instruction)?;
    let text = write(model, &assembled.prompt)?;
    let now = now_ms();
    let made_by = if kind == ReplyUseKind::Auto { ReplyMadeBy::Auto } else { ReplyMadeBy::OnDemand };
    let draft = ReplyDraft {
        id: draft_id.to_string(), thread_id: f.thread_id.clone(), message_id: f.message_id.clone(), text, made_by, excerpt_ids: excerpt_ids(&assembled.retrieval),
        instruction: instruction.map(str::to_string), state: ReplyDraftState::Proposed, revision: 0, created_at_ms: now, updated_at_ms: now,
    };
    let pass = pass.map(|p| contracts::ReplyPass { draft_id: Some(draft.id.clone()), ..p });
    if let Some(old) = s.store.reply_draft_put(draft.clone(), pass)? { announce_changed(s, &old); }
    announce_ready(s, &draft);
    Ok(draft)
}

pub(crate) fn run(s: &Shared, draft_id: &str, thread_id: &str, instruction: Option<&str>, model: &dyn TextModel) {
    let message = match MailReads::open(&s.store).and_then(|r| r.newest_in_thread(thread_id)) {
        Ok(Some(m)) => m,
        Ok(None) => return s.log(&format!("the draft {draft_id} was not written: the thread {thread_id} holds no message")),
        Err(e) => return s.log(&format!("the draft {draft_id} was not written: {}", words(&e))),
    };
    if let Err(e) = draft_for(s, draft_id, &message, ReplyUseKind::OnDemand, instruction, model, None) {
        s.log(&format!("the draft {draft_id} for the thread {thread_id} was not written: {}", words(&e)));
    }
}
