//! Delta 23: received attachments (the columns body-r2 asked for, the download command), and excerpts that are not mail (reply-engine's gap 6).
//! Conventions of deltas 20 to 22 hold. Four numbers live in docs/CONSTANTS.md (generated into constants.rs); this file re-exports the three it uses at the crate root.
//!   STATE PATHS  attachment/<attachment id>/transfer     {state, reason_words?}   what a tile re-reads (a store read, never a wait)
//!                reply/source_chunk/<id>                 SourceChunk              one excerpt of a file of a source folder or of a calendar event
use crate::*;

pub const ATTACHMENT_DOWNLOAD_REFUSAL_CODE: &str = "attachment_too_large_to_keep";
pub const SOURCE_CHUNK_REFUSAL_CODE: &str = "bad_source_chunk";
fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }
fn blank(s: &str) -> bool { s.trim().is_empty() }

pub use crate::constants::ATTACHMENT_KEEP_MAX_BYTES;   // generated from docs/CONSTANTS.md (body-r2's proposal)

// ---------------------------------------------------------------- received attachments

/// Where a received file stands on this Mac. `NotOnThisMac` = no copy yet (a store read says so at once: no view waits on the network); `Downloading` = the core is
/// fetching it in the background; `OnThisMac` = `AttachmentFile.local_path` is set; `Failed` = the last fetch failed, in words (it may be asked again).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum AttachmentTransfer { NotOnThisMac, Downloading, OnThisMac, Failed { reason_words: String } }

/// The transfer state of one file from what the core knows: a local path wins (the file is here, whatever happened before), then an active fetch, then the last
/// failure, else "not on this Mac yet". The core reads `local_path` from the store row, so the answer never waits for the network.
#[uniffi::export]
pub fn attachment_transfer_of(file: AttachmentFile, in_flight: bool, last_failure_words: Option<String>) -> AttachmentTransfer {
    if file.local_path.is_some() { return AttachmentTransfer::OnThisMac; }
    if in_flight { return AttachmentTransfer::Downloading; }
    match last_failure_words.filter(|w| !blank(w)) { Some(reason_words) => AttachmentTransfer::Failed { reason_words }, None => AttachmentTransfer::NotOnThisMac }
}

/// What a gallery shows of a mail's files: the real files, in order; the INLINE ones (the logo and pictures the message body draws itself) are left out.
#[uniffi::export]
pub fn gallery_attachments(files: Vec<AttachmentFile>) -> Vec<AttachmentFile> { files.into_iter().filter(|f| !f.inline).collect() }

/// What `DownloadAttachment` does.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPlan { Start, AlreadyOnThisMac, AlreadyDownloading }

/// The one rule of `DownloadAttachment`, answered at once: an unknown file is NotFound; a file already here or already coming is not an error (the command is
/// idempotent: a double click starts one fetch); a file over `ATTACHMENT_KEEP_MAX_BYTES` is refused in words (`attachment_too_large_to_keep`), it would not be kept.
#[uniffi::export]
pub fn plan_download_attachment(attachment_id: String, file: Option<AttachmentFile>, in_flight: bool) -> Result<DownloadPlan, CoreError> {
    let Some(f) = file else { return Err(CoreError::NotFound { kind: "attachment".into(), id: attachment_id }); };
    if f.local_path.is_some() { return Ok(DownloadPlan::AlreadyOnThisMac); }
    if in_flight { return Ok(DownloadPlan::AlreadyDownloading); }
    if f.size_bytes > ATTACHMENT_KEEP_MAX_BYTES { return Err(refuse(ATTACHMENT_DOWNLOAD_REFUSAL_CODE, format!("\"{}\" is too large to keep on this Mac ({} MB at most): open it on the server", f.file_name, ATTACHMENT_KEEP_MAX_BYTES / (1024 * 1024)))); }
    Ok(DownloadPlan::Start)
}

fn js(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
    o
}
/// `attachment/<id>/transfer`.
#[uniffi::export]
pub fn attachment_transfer_json(t: AttachmentTransfer) -> String {
    match t {
        AttachmentTransfer::NotOnThisMac => "{\"state\":\"not_on_this_mac\"}".into(),
        AttachmentTransfer::Downloading => "{\"state\":\"downloading\"}".into(),
        AttachmentTransfer::OnThisMac => "{\"state\":\"on_this_mac\"}".into(),
        AttachmentTransfer::Failed { reason_words } => format!("{{\"state\":\"failed\",\"reason_words\":{}}}", js(&reason_words)),
    }
}

// ---------------------------------------------------------------- excerpts that are not mail

/// Where an excerpt that is not a message came from: a file inside a source folder of this Mac (`folder_path` = the source, `item_path` = the file's path inside it,
/// "/" separated), or an event of a calendar.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum SourceChunkOrigin { File { folder_path: String, item_path: String }, Event { calendar_id: String, event_id: String } }

/// DATA-MAP `source-chunk`: one piece of a file of a source folder or of a calendar event, in its OWN table beside `chunk` (WHY a second table and not a nullable
/// `chunk.message_id`: every reader of `chunk` today assumes a message, so a source row in that table would fail the indexer's pending walk and `Store::chunk`; one
/// type could not say "a message or an item" without changing every literal; and 0014's rows and `chunk_vector_key` stay exactly as they are). Its id is its own
/// (AUTOINCREMENT, never reused). `date_ms` = the file's or the event's date; `title` = the file name or the event title. `embedded_at_ms` None = no vector yet.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct SourceChunk { pub id: i64, pub origin: SourceChunkOrigin, pub chunk_no: u32, pub date_ms: Option<i64>, pub title: String, pub text: String, pub embedded_at_ms: Option<i64> }

/// The source a chunk belongs to, as `reply_source_key` spells it (a removed source takes its chunks: 0016's trigger).
#[uniffi::export]
pub fn source_chunk_source_key(origin: SourceChunkOrigin) -> String {
    match origin { SourceChunkOrigin::File { folder_path, .. } => reply_source_key(ReplySourceRef::LocalFolder { path: folder_path }), SourceChunkOrigin::Event { calendar_id, .. } => reply_source_key(ReplySourceRef::Calendar { calendar_id }) }
}
/// The item inside the source: the file's path in its folder, or the event id.
#[uniffi::export]
pub fn source_chunk_item_key(origin: SourceChunkOrigin) -> String { match origin { SourceChunkOrigin::File { item_path, .. } => item_path, SourceChunkOrigin::Event { event_id, .. } => event_id } }
/// The origin the two stored keys name; None for a pair that is not one (a reader of a stored row refuses it in words).
#[uniffi::export]
pub fn source_chunk_origin_from_keys(source_key: String, item_key: String) -> Option<SourceChunkOrigin> {
    if blank(&item_key) { return None; }
    match reply_source_from_key(source_key)? {
        ReplySourceRef::LocalFolder { path } => Some(SourceChunkOrigin::File { folder_path: path, item_path: item_key }),
        ReplySourceRef::Calendar { calendar_id } => Some(SourceChunkOrigin::Event { calendar_id, event_id: item_key }),
        ReplySourceRef::MailFolder { .. } => None,
    }
}

/// The meaning index has ONE key space: a mail chunk is filed under `chunk_vector_key(id)` (its id, below 2^62), a source chunk under this (2^62 + its id), so the
/// two ids can never meet. A hit's key says which table to read (`vector_key_is_source_chunk`).
pub const SOURCE_CHUNK_KEY_BASE: u64 = 1 << 62;
#[uniffi::export]
pub fn source_chunk_vector_key(source_chunk_id: i64) -> u64 { SOURCE_CHUNK_KEY_BASE + source_chunk_id.max(0) as u64 }
#[uniffi::export]
pub fn vector_key_is_source_chunk(key: u64) -> bool { key >= SOURCE_CHUNK_KEY_BASE }
/// The source chunk id a vector key names; None for a mail chunk's key.
#[uniffi::export]
pub fn source_chunk_id_from_vector_key(key: u64) -> Option<i64> { if key >= SOURCE_CHUNK_KEY_BASE { Some((key - SOURCE_CHUNK_KEY_BASE) as i64) } else { None } }
/// `ReplyExcerpt.chunk_id` of a source chunk: `s<id>` (a mail chunk's is plain decimal, `chunk_excerpt_id`).
#[uniffi::export]
pub fn source_chunk_excerpt_id(source_chunk_id: i64) -> String { format!("s{source_chunk_id}") }
#[uniffi::export]
pub fn source_chunk_id_from_excerpt_id(excerpt_id: String) -> Option<i64> { excerpt_id.strip_prefix('s').and_then(|n| n.parse::<i64>().ok()).filter(|n| *n >= 1) }

/// Refuses a source chunk with an id below 1, no text, or an origin with an empty part.
#[uniffi::export]
pub fn check_source_chunk(c: SourceChunk) -> Result<(), CoreError> {
    if c.id < 1 { return Err(refuse(SOURCE_CHUNK_REFUSAL_CODE, "a chunk id starts at 1")); }
    if blank(&c.text) { return Err(refuse(SOURCE_CHUNK_REFUSAL_CODE, "a chunk has text")); }
    let (a, b) = match &c.origin { SourceChunkOrigin::File { folder_path, item_path } => (folder_path, item_path), SourceChunkOrigin::Event { calendar_id, event_id } => (calendar_id, event_id) };
    if blank(a) || blank(b) { return Err(refuse(SOURCE_CHUNK_REFUSAL_CODE, "a chunk names its source and its item")); }
    Ok(())
}
/// `reply/source_chunk/<id>`.
#[uniffi::export]
pub fn source_chunk_json(c: SourceChunk) -> String {
    let origin = match &c.origin {
        SourceChunkOrigin::File { folder_path, item_path } => format!("{{\"kind\":\"file\",\"folder_path\":{},\"item_path\":{}}}", js(folder_path), js(item_path)),
        SourceChunkOrigin::Event { calendar_id, event_id } => format!("{{\"kind\":\"event\",\"calendar_id\":{},\"event_id\":{}}}", js(calendar_id), js(event_id)),
    };
    format!("{{\"id\":{},\"origin\":{},\"chunk_no\":{},\"date_ms\":{},\"title\":{},\"text\":{},\"embedded_at_ms\":{}}}", c.id, origin, c.chunk_no,
        c.date_ms.map(|n| n.to_string()).unwrap_or_else(|| "null".into()), js(&c.title), js(&c.text), c.embedded_at_ms.map(|n| n.to_string()).unwrap_or_else(|| "null".into()))
}
