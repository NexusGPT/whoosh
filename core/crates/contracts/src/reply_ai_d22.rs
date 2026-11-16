//! Delta 22: attachments on a draft (docs/lanes/probes-r1.md "Forward re-attach", docs/lanes/reply-actions.md F3) and the refusal codes and failure event
//! reply-engine asked for (docs/lanes/reply-engine.md "Contract gaps"). Conventions of deltas 20 and 21 hold: English words with a code, reads are state paths,
//! the pure rules are exported once for the Rust core and the Swift FixtureCore.
//!
//! WHY SEPARATE COMMANDS, NOT A LIST ON SaveDraft / UpdateDraft: those two replace the draft's text fields whole and have many callers (every `Draft` and
//! `SaveDraft` literal in Rust and Swift); a list there makes "leave the files as they are" and "no files" the same empty list (the three-nulls trap) and a
//! composer that only fixes a typo would re-send every path. A file also costs the disk (size, existence), which a text save never does. So: `Draft.attachment_ids`
//! (a field that already exists) holds the ids of the draft's `OutgoingAttachment` rows, and `AddDraftAttachment` / `RemoveDraftAttachment` change the list
//! one file at a time. A composer with no saved draft saves one first (`SaveDraft`), then attaches.
//!
//! OUT OF SCOPE: the inline images of a reply's quoted text (they stay part of the quoted body, never an `OutgoingAttachment`).
//!   STATE PATHS  draft/<draft id>/attachments        [OutgoingAttachment]   what the composer draws
//!                outbox/<outbox id>/attachments      [OutgoingAttachment]   what the queued message carries (the rows are re-owned at send)
use crate::*;

pub const ATTACHMENT_GONE_CODE: &str = "attachment_file_gone";
pub const ATTACHMENT_TOO_LARGE_CODE: &str = "attachment_too_large";
pub const ATTACHMENT_TOTAL_TOO_LARGE_CODE: &str = "attachment_total_too_large";
pub const ATTACHMENT_BAD_SOURCE_CODE: &str = "bad_attachment";
pub const ATTACHMENT_DRAFT_CLOSED_CODE: &str = "attachment_draft_closed";
/// THE ONE code for "no model is set for this job" (docs/lanes/ai-settings-core.md "Contract proposals"; reply-engine switches from its `reply_no_model` to it): an AI draft,
/// a change, a try, the voice analysis, the label AI. The words say which setting to make. A SEPARATE code stays only for "an analysis is already running".
pub const AI_NO_MODEL_CODE: &str = "ai_no_model";
/// `RunVoiceAnalysis` while one runs.
pub const REPLY_VOICE_RUNNING_CODE: &str = "reply_voice_running";

pub use crate::constants::{ATTACHMENT_FILE_MAX_BYTES, ATTACHMENT_TOTAL_MAX_BYTES};   // generated from docs/CONSTANTS.md (proposed by replies-contract delta 22)

fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }
fn blank(s: &str) -> bool { s.trim().is_empty() }

/// Where an attachment's bytes come from: a file of this Mac (read when the message is built), or an attachment of a stored message (the forward case: the
/// core fetches it when the message is built, the owner never downloads it by hand).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AttachmentSource { LocalFile { path: String }, StoredMessage { message_id: String, attachment_id: String } }

/// Where an attachment stands. `Ready` = its bytes can be read now. `WillDownload` = a stored message's attachment not on this Mac yet: fetched when the message
/// is built. `Gone` = the file of this Mac is no longer there. `TooLarge` = over the per-file cap (a forwarded attachment can arrive so: it is listed, never silently dropped).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutgoingAttachmentState { Ready, WillDownload, Gone, TooLarge }

/// DATA-MAP `outgoing-attachment`: one file on a draft or on a queued message. `id` is its own; `owner_id` is the draft id, and after `Send` the outbox entry id
/// (the rows are re-owned, so the draft can be removed and an undo of the send brings the rows back). `media_type` is the MIME type ("application/pdf").
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct OutgoingAttachment {
    pub id: String,
    pub owner_id: String,
    pub source: AttachmentSource,
    pub file_name: String,
    pub media_type: String,
    pub size_bytes: u64,
    pub state: OutgoingAttachmentState,
}

/// What `plan_add_attachment` decided.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum AttachmentAddPlan { AlreadyThere { attachment_id: String }, Add { state: OutgoingAttachmentState, file_name: String } }

/// The name a path shows ("~/Documents/Devis.pdf" -> "Devis.pdf"): its last component.
#[uniffi::export]
pub fn attachment_file_name_of_path(path: String) -> String { path.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string() }

/// The sum of the sizes (what the total cap counts).
#[uniffi::export]
pub fn attachments_total_bytes(list: Vec<OutgoingAttachment>) -> u64 { list.iter().map(|a| a.size_bytes).sum() }

/// The one rule of `AddDraftAttachment`. The core reads the disk (or the stored attachment) and passes what it found: `size_bytes` None for a local file means it
/// is not there (`attachment_file_gone`). Refused in words: no source (`bad_attachment`: a path that is not absolute or under `~/`, a stored attachment with no ids),
/// a file gone, a file over `ATTACHMENT_FILE_MAX_BYTES`, a total over `ATTACHMENT_TOTAL_MAX_BYTES`. The same source twice is not an error: it answers `AlreadyThere`
/// (adding is idempotent, a double drop adds nothing). A stored attachment (forward) not yet on this Mac plans `WillDownload`; one on this Mac plans `Ready`.
/// `local_on_this_mac` is only read for a stored attachment.
#[uniffi::export]
pub fn plan_add_attachment(existing: Vec<OutgoingAttachment>, source: AttachmentSource, stored_file_name: Option<String>, size_bytes: Option<u64>, local_on_this_mac: bool) -> Result<AttachmentAddPlan, CoreError> {
    if let Some(a) = existing.iter().find(|a| a.source == source) { return Ok(AttachmentAddPlan::AlreadyThere { attachment_id: a.id.clone() }); }
    let (file_name, state) = match &source {
        AttachmentSource::LocalFile { path } => {
            let p = check_local_folder_path(path.clone()).map_err(|_| refuse(ATTACHMENT_BAD_SOURCE_CODE, "a file of this Mac starts at the root (/...) or at your home (~/...)"))?;
            let name = attachment_file_name_of_path(p);
            if name.is_empty() { return Err(refuse(ATTACHMENT_BAD_SOURCE_CODE, "pick a file, not a folder")); }
            if size_bytes.is_none() { return Err(refuse(ATTACHMENT_GONE_CODE, format!("\"{name}\" is no longer there"))); }
            (name, OutgoingAttachmentState::Ready)
        }
        AttachmentSource::StoredMessage { message_id, attachment_id } => {
            if blank(message_id) || blank(attachment_id) { return Err(refuse(ATTACHMENT_BAD_SOURCE_CODE, "a stored attachment needs its message and its id")); }
            (stored_file_name.clone().filter(|n| !blank(n)).unwrap_or_else(|| "attachment".to_string()), if local_on_this_mac { OutgoingAttachmentState::Ready } else { OutgoingAttachmentState::WillDownload })
        }
    };
    let size = size_bytes.unwrap_or(0);
    if size > ATTACHMENT_FILE_MAX_BYTES { return Err(refuse(ATTACHMENT_TOO_LARGE_CODE, format!("\"{file_name}\" is over the {} MB a message can carry one file of", ATTACHMENT_FILE_MAX_BYTES / (1024 * 1024)))); }
    if attachments_total_bytes(existing) + size > ATTACHMENT_TOTAL_MAX_BYTES { return Err(refuse(ATTACHMENT_TOTAL_TOO_LARGE_CODE, format!("the files together would be over the {} MB a message can carry", ATTACHMENT_TOTAL_MAX_BYTES / (1024 * 1024)))); }
    Ok(AttachmentAddPlan::Add { state, file_name })
}

/// What `Forward` puts on the forward draft BY DEFAULT: every attachment of the original, as a `StoredMessage` source, in the original's order. One over the per-file
/// cap is listed as `TooLarge` (never silently dropped; the owner removes it, and sending is refused until then). One already on this Mac is `Ready`, else `WillDownload`.
/// Ids are `<draft id>/<n>` from 1.
#[uniffi::export]
pub fn forward_attachments(draft_id: String, original: Vec<AttachmentFile>) -> Vec<OutgoingAttachment> {
    original.into_iter().enumerate().map(|(i, a)| OutgoingAttachment {
        id: format!("{draft_id}/{}", i + 1), owner_id: draft_id.clone(), source: AttachmentSource::StoredMessage { message_id: a.message_id.clone(), attachment_id: a.id.clone() },
        file_name: a.file_name, media_type: a.mime_type,
        state: if a.size_bytes > ATTACHMENT_FILE_MAX_BYTES { OutgoingAttachmentState::TooLarge } else if a.local_path.is_some() { OutgoingAttachmentState::Ready } else { OutgoingAttachmentState::WillDownload },
        size_bytes: a.size_bytes,
    }).collect()
}

/// Refuses a send the files cannot go with, in words: a file gone (`attachment_file_gone`), a file over the cap (`attachment_too_large`), the total over the cap
/// (`attachment_total_too_large`). The first problem is named. The core calls it before the message is built, so nothing is queued half-attached.
#[uniffi::export]
pub fn check_draft_attachments_sendable(list: Vec<OutgoingAttachment>) -> Result<(), CoreError> {
    if let Some(a) = list.iter().find(|a| a.state == OutgoingAttachmentState::Gone) { return Err(refuse(ATTACHMENT_GONE_CODE, format!("\"{}\" is no longer there: remove it or attach it again", a.file_name))); }
    if let Some(a) = list.iter().find(|a| a.state == OutgoingAttachmentState::TooLarge || a.size_bytes > ATTACHMENT_FILE_MAX_BYTES) { return Err(refuse(ATTACHMENT_TOO_LARGE_CODE, format!("\"{}\" is too large to send: remove it", a.file_name))); }
    if attachments_total_bytes(list) > ATTACHMENT_TOTAL_MAX_BYTES { return Err(refuse(ATTACHMENT_TOTAL_TOO_LARGE_CODE, "the files together are too large to send: remove one")); }
    Ok(())
}

/// Refuses touching the files of a draft that is no longer open for editing (queued, sent or gone): `attachment_draft_closed`. `open` = the core found the draft in
/// the draft list. (A queued message's files change only through Undo.)
#[uniffi::export]
pub fn check_attachment_draft_open(open: bool) -> Result<(), CoreError> {
    if open { Ok(()) } else { Err(refuse(ATTACHMENT_DRAFT_CLOSED_CODE, "this draft was already sent: undo the send to change its files")) }
}

fn js(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
    o
}
/// The state spelling of an attachment state.
#[uniffi::export]
pub fn outgoing_attachment_state_key(s: OutgoingAttachmentState) -> String {
    match s { OutgoingAttachmentState::Ready => "ready", OutgoingAttachmentState::WillDownload => "will_download", OutgoingAttachmentState::Gone => "gone", OutgoingAttachmentState::TooLarge => "too_large" }.to_string()
}
#[uniffi::export]
pub fn outgoing_attachment_state_from_key(key: String) -> Option<OutgoingAttachmentState> {
    [OutgoingAttachmentState::Ready, OutgoingAttachmentState::WillDownload, OutgoingAttachmentState::Gone, OutgoingAttachmentState::TooLarge].into_iter().find(|s| outgoing_attachment_state_key(*s) == key)
}
/// `draft/<id>/attachments` and `outbox/<id>/attachments`: what the composer draws and what a queued message carries, with the total.
#[uniffi::export]
pub fn outgoing_attachments_json(list: Vec<OutgoingAttachment>) -> String {
    let rows: Vec<String> = list.iter().map(|a| {
        let src = match &a.source {
            AttachmentSource::LocalFile { path } => format!("{{\"kind\":\"local_file\",\"path\":{}}}", js(path)),
            AttachmentSource::StoredMessage { message_id, attachment_id } => format!("{{\"kind\":\"stored_message\",\"message_id\":{},\"attachment_id\":{}}}", js(message_id), js(attachment_id)),
        };
        format!("{{\"id\":{},\"owner_id\":{},\"source\":{},\"file_name\":{},\"media_type\":{},\"size_bytes\":{},\"state\":{}}}", js(&a.id), js(&a.owner_id), src, js(&a.file_name), js(&a.media_type), a.size_bytes, js(&outgoing_attachment_state_key(a.state)))
    }).collect();
    format!("{{\"total_bytes\":{},\"attachments\":[{}]}}", attachments_total_bytes(list), rows.join(","))
}
