//! Where a source stands: NotStarted until its first sync, Syncing{percent} while one runs, then UpToDate, or Failed with words.
//! A tracker sends a change only when the figure moves, so a folder of 20,000 files sends at most about a hundred events.
use crate::ports::ProgressSink;
use contracts::{sync_percent, ReplySource, ReplySourceRef, ReplySyncProgress};

/// A source just added or switched on: enabled, nothing read yet, NotStarted.
pub fn fresh_source(source: ReplySourceRef) -> ReplySource {
    ReplySource { source, enabled: true, item_count: 0, progress: ReplySyncProgress::NotStarted }
}

pub struct ProgressTracker<'a> {
    source: ReplySourceRef,
    sink: &'a dyn ProgressSink,
    state: ReplySyncProgress,
}

impl<'a> ProgressTracker<'a> {
    pub fn new(source: ReplySourceRef, sink: &'a dyn ProgressSink) -> Self {
        Self { source, sink, state: ReplySyncProgress::NotStarted }
    }

    pub fn state(&self) -> &ReplySyncProgress {
        &self.state
    }

    fn set(&mut self, state: ReplySyncProgress, item_count: u64) {
        if self.state != state {
            self.state = state;
            self.sink.progress(&self.source, item_count, &self.state);
        }
    }

    /// `done` of `of` items handled; `item_count` = the items that hold text so far.
    pub fn advance(&mut self, done: u64, of: u64, item_count: u64) {
        self.set(ReplySyncProgress::Syncing { percent: sync_percent(done, of) }, item_count);
    }

    pub fn up_to_date(&mut self, item_count: u64) {
        self.set(ReplySyncProgress::UpToDate, item_count);
    }

    pub fn failed(&mut self, item_count: u64, reason_words: String) {
        self.set(ReplySyncProgress::Failed { reason_words }, item_count);
    }
}
