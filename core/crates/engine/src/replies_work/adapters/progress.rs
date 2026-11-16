//! `ProgressSink` of reply-sources: each report is stored on the source's row (`reply_source_progress`, what `reply/sources` serves) and announced as
//! `CoreEvent::ReplySourceProgress` on the SyncStatus stream (where the fixture core sends it).
use contracts::{CoreEvent, ReplySourceRef, ReplySyncProgress, StreamKind};
use reply_sources::ProgressSink;
use store::Store;

use crate::rust_core::now_ms;
use crate::sync::{emit, Sinks};

/// `gate` runs before every write: the background's pause (a report waits while the background is paused, so a held core writes nothing).
pub(crate) struct Progress<'a> { pub store: &'a Store, pub sinks: &'a Sinks, pub gate: &'a (dyn Fn() + Sync) }

impl ProgressSink for Progress<'_> {
    fn progress(&self, source: &ReplySourceRef, item_count: u64, progress: &ReplySyncProgress) {
        (self.gate)();
        match self.store.reply_source_progress(source.clone(), item_count, progress.clone(), now_ms()) {
            Ok(true) => {}
            // the folder was removed while it synced: its row is gone, nothing to announce
            Ok(false) => return,
            Err(e) => eprintln!("replies: the progress of a source was not kept: {e}"),
        }
        emit(self.sinks, StreamKind::SyncStatus, CoreEvent::ReplySourceProgress { source: source.clone(), item_count, progress: progress.clone() });
    }
}
