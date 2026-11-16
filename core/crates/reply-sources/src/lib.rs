//! What AI replies may read beside the mail index, kept on this Mac (docs/lanes/reply-sources.brief.md, J10 Sources pane).
//! Folders of this Mac are walked and read into paragraph-sized excerpts, and kept up to date (only a changed file is read again);
//! the mail folders of an account become the pane's tiles, and `MailScope` says which messages retrieval may read (this account or
//! all); calendar events become one excerpt each; every sync reports `ReplySyncProgress`. Pure logic behind the ports in `ports`:
//! the disk (`FileTree`, answered by `adapters::DiskTree`), the document readers (`attachment_index::TextReader`, already answered by
//! the app), the calendars (`CalendarStore`, answered by the app over EventKit), the index (`ExcerptSink`) and the events
//! (`ProgressSink`). Nothing here touches the network or a model; every sync is background work.
pub mod adapters;
mod constants;
mod dates;
mod excerpt;
mod ports;
mod progress;
mod split;
mod text;
mod usecases;
mod walk;

pub use constants::*;
pub use excerpt::{excerpt_id, ItemStamp, SourceExcerpt};
pub use ports::{CalendarEvent, CalendarInfo, CalendarStore, ExcerptSink, FileTree, ProgressSink, TreeEntry, TreeEntryKind};
pub use progress::{fresh_source, ProgressTracker};
pub use split::split_excerpts;
pub use text::{read_file_text, FileText};
pub use usecases::enabled_sources::enabled_sources;
pub use usecases::mail_scope::MailScope;
pub use usecases::mail_tiles::{mail_folder_tiles, MailFolderCount};
pub use usecases::sync_calendar::{calendar_window, event_excerpt, sync_calendar, CalendarSyncReport};
pub use usecases::sync_local_folder::{sync_local_folder, FolderPorts, FolderSyncReport};
pub use usecases::SyncOutcome;
pub use walk::{walk_folder, FoundFile, Walk, WalkSkips};

/// Re-exported so a caller names the document reader from here (the attachments' port, answered by Sources/Features/AttachmentIndex).
pub use attachment_index::TextReader;
