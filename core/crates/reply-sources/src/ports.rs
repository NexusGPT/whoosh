//! The ports this crate reads and writes through. Plain Rust traits: the crate and its tests run with no disk, no EventKit and no index.
//! The document reader is the attachments' `attachment_index::TextReader`, re-exported from the crate root.
use crate::excerpt::{ItemStamp, SourceExcerpt};
use contracts::{CoreError, ReplySourceRef, ReplySyncProgress};
use std::path::Path;

/// What one entry of a folder is. A link is `Link` whatever it points to: the walk never follows one (a link out of the folder would
/// read what the owner did not pick; a link inside it would read its target twice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeEntryKind {
    File { size: u64, modified_ms: i64 },
    Dir,
    Link,
    /// a socket, a device, a pipe
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeEntry {
    /// the entry's own name, never a path
    pub name: String,
    pub kind: TreeEntryKind,
}

/// The disk, as the walk sees it (answered by `adapters::DiskTree`). A path may start with "~/" (the owner's home), as the Sources pane
/// stores it.
pub trait FileTree: Send + Sync {
    /// The entries of one folder, read without following links (lstat).
    fn read_dir(&self, dir: &Path) -> Result<Vec<TreeEntry>, CoreError>;
    /// At most `max_bytes` of one file.
    fn read(&self, file: &Path, max_bytes: u64) -> Result<Vec<u8>, CoreError>;
}

/// The index of the sources other than mail: excerpts per item, and each item's stamp. Same shape as the mail index's chunks (lane
/// sync-full-r1), keyed by source and item instead of by message (the proposal in docs/lanes/reply-sources.md).
pub trait ExcerptSink: Send + Sync {
    /// The items the index holds for one source, with their stamps.
    fn stamps(&self, source: &ReplySourceRef) -> Result<Vec<ItemStamp>, CoreError>;
    /// Replaces every excerpt of one item and its stamp, in one write (an empty list = the item is known and holds no text).
    fn put_item(&self, source: &ReplySourceRef, stamp: &ItemStamp, excerpts: &[SourceExcerpt]) -> Result<(), CoreError>;
    /// Removes one item: its excerpts and its stamp.
    fn remove_item(&self, source: &ReplySourceRef, item_key: &str) -> Result<(), CoreError>;
}

/// Where progress goes: the core turns each call into `CoreEvent::ReplySourceProgress { source, item_count, progress }`.
pub trait ProgressSink: Send + Sync {
    fn progress(&self, source: &ReplySourceRef, item_count: u64, progress: &ReplySyncProgress);
}

/// A calendar macOS gives (the Agendas pane lists them; the Sources pane switches them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarInfo {
    pub calendar_id: String,
    pub name: String,
    /// the account the calendar belongs to, as macOS names it ("assem@nexus-example.com", "iCloud")
    pub account_label: String,
    /// "#RRGGBB"
    pub colour: String,
}

/// One event, or one occurrence of a repeating event (an occurrence keeps its series' `event_id`; its `start_ms` tells it apart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent {
    pub event_id: String,
    pub calendar_id: String,
    pub title: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub all_day: bool,
    /// names, or addresses when an attendee has no name
    pub attendees: Vec<String>,
    pub location: Option<String>,
}

/// The calendars of this Mac, read-only (answered by the app over EventKit, lane settingscalendars-mac). Never writes an event.
pub trait CalendarStore: Send + Sync {
    fn calendars(&self) -> Result<Vec<CalendarInfo>, CoreError>;
    /// The events of one calendar that overlap [from_ms, to_ms).
    fn events(&self, calendar_id: &str, from_ms: i64, to_ms: i64) -> Result<Vec<CalendarEvent>, CoreError>;
}
