//! The numbers this crate decides. Each is PROPOSED for docs/CONSTANTS.md (not written there: the ledger is not this lane's); the most
//! text one file contributes is the ledger's ATTACHMENT_TEXT_MAX_CHARS, read through attachment-index, so that number lives once.

/// A file bigger than this is not read (a video export renamed .txt, a 300 MB PDF scan). PROPOSED as REPLY_FILE_MAX_BYTES.
pub const REPLY_FILE_MAX_BYTES: u64 = 20 * 1024 * 1024;

/// An excerpt is at least this long when the text allows it: short paragraphs (a title, a line "Bonjour,") join the next one.
/// PROPOSED as REPLY_EXCERPT_MIN_CHARS.
pub const REPLY_EXCERPT_MIN_CHARS: usize = 200;

/// An excerpt is at most this long: a longer paragraph is cut at a sentence end. PROPOSED as REPLY_EXCERPT_MAX_CHARS.
pub const REPLY_EXCERPT_MAX_CHARS: usize = 1200;

/// The calendar window a draft can read: the next 14 days and the last 7 (brief). PROPOSED as REPLY_CALENDAR_DAYS_AHEAD and
/// REPLY_CALENDAR_DAYS_BEHIND.
pub const REPLY_CALENDAR_DAYS_AHEAD: i64 = 14;
pub const REPLY_CALENDAR_DAYS_BEHIND: i64 = 7;

/// Folders never walked: dependency and build caches that hold thousands of files nobody writes to. Hidden folders (".git") are
/// skipped by their dot. PROPOSED as REPLY_SKIP_DIRS.
pub const REPLY_SKIP_DIRS: &[&str] = &["node_modules", "bower_components", "__pycache__", "venv", "Pods", "DerivedData"];

/// Folders that are macOS packages (an app, a photo library): one thing to the owner, thousands of files to a walk.
pub const REPLY_SKIP_DIR_EXT: &[&str] = &["app", "photoslibrary", "musiclibrary", "framework", "bundle", "xcodeproj", "xcworkspace"];
