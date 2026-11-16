//! One file per use case.
pub mod enabled_sources;
pub mod mail_scope;
pub mod mail_tiles;
pub mod sync_calendar;
pub mod sync_local_folder;

/// How a sync ended. `Stopped` = `keep_going` said no (the folder was removed or switched off mid-sync): nothing more was written
/// and no final progress was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    UpToDate,
    Failed { reason_words: String },
    Stopped,
}
