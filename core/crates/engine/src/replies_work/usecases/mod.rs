//! One file per use case of the draft engine. Orchestration only: the outside answers come through the adapters' ports, the rules are the contract's.
pub mod accept_draft;
pub mod arrival_pass;
pub mod calendar_free;
pub mod decide;
pub mod discard_draft;
pub mod draft_on_demand;
pub mod request_change;
pub mod sync_sources;
pub mod try_reply;
pub mod voice_analysis;
pub mod write_draft;

use contracts::{CoreError, CoreEvent, ReplyDraft, ReplyStepResult, StreamKind};

use super::Shared;
use crate::sync::emit;

/// A step let the message through (passed, or skipped because its card is off).
pub(crate) fn through(r: ReplyStepResult) -> bool { matches!(r, ReplyStepResult::Passed | ReplyStepResult::Skipped) }

/// The new draft of a thread (the Thread stream, where the fixture core sends it).
pub(crate) fn announce_ready(s: &Shared, d: &ReplyDraft) {
    emit(&s.sinks, StreamKind::Thread, CoreEvent::ReplyDraftReady { thread_id: d.thread_id.clone(), draft_id: d.id.clone() });
}

/// A draft of a thread changed (rewritten, taken, discarded, replaced): the row, whole.
pub(crate) fn announce_changed(s: &Shared, d: &ReplyDraft) {
    emit(&s.sinks, StreamKind::Thread, CoreEvent::ReplyDraftChanged { thread_id: d.thread_id.clone(), draft: d.clone() });
}

/// The words of a failure, as the log keeps them.
pub(crate) fn words(e: &CoreError) -> String { crate::bodies::words(e) }
