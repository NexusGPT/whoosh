//! Contracts delta 22 in the real core: the files of a draft (`outgoing_attachment`, migration 0015). `AddDraftAttachment` and
//! `RemoveDraftAttachment` change a saved draft's list one file at a time; `Forward` opens a draft that already carries every file of the
//! original; the reads `draft/<id>/attachments` and `outbox/<id>/attachments` answer what the composer draws and what a queued message
//! carries. A file of this Mac is only LOOKED AT when it is added (its size, that it is there) and READ when the message is built
//! (`DraftFiles`, the outbox's `AttachmentFilesPort`), so a file edited in between goes as it is then. A forwarded file is read from this
//! Mac when its bytes are here, else its message is fetched from the server and the file taken out of it; a fetch that fails fails the
//! send in words, never a message sent without its file. The rules are the contract's (`plan_add_attachment`, `forward_attachments`,
//! `check_draft_attachments_sendable`, `check_attachment_draft_open`); the caps are the contract's constants, never retyped.
use std::path::PathBuf;
use std::sync::Arc;

use contracts::{
    check_attachment_draft_open, check_draft_attachments_sendable, forward_attachments, forward_body, outgoing_attachments_json, plan_add_attachment, prefixed_subject,
    ActionFailed, AttachmentAddPlan, AttachmentSource, CommandOutcome, CoreError, Draft, OutgoingAttachment, OutgoingAttachmentState, ATTACHMENT_GONE_CODE,
};
use store::Store;
use sync_imap::ImapMailAdapter;

use crate::sync::SyncDeps;

// attach-engine (delta 23): `DownloadAttachment` and `attachment/<id>/transfer`, in their own file beside this one (the fetch itself is `Bodies::attachment_bytes`, the
// one `stored_bytes` below calls for a forwarded file).
#[path = "attachment_download.rs"]
pub mod download;

fn unexpected(words: impl Into<String>) -> CoreError { CoreError::Unexpected { reason_words: words.into() } }
fn store_failed(what: &str, e: store::StoreError) -> CoreError { unexpected(format!("{what}: {e}")) }

/// A path as the contract takes it (absolute, or under `~/`) as a path of this Mac.
pub fn expand(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(rest),
        None => PathBuf::from(path),
    }
}

/// The size of a file of this Mac, None when it is not there (or is a folder).
fn disk_size(path: &str) -> Option<u64> {
    std::fs::metadata(expand(path)).ok().filter(|m| m.is_file()).map(|m| m.len())
}

/// The media type a file of this Mac goes with, from its extension (application/octet-stream when the extension says nothing).
pub fn media_type_of(file_name: &str) -> String {
    let ext = file_name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => "application/pdf", "txt" | "text" | "md" => "text/plain", "csv" => "text/csv", "html" | "htm" => "text/html", "ics" => "text/calendar",
        "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "gif" => "image/gif", "heic" => "image/heic", "webp" => "image/webp", "svg" => "image/svg+xml",
        "zip" => "application/zip", "json" => "application/json", "xml" => "application/xml", "rtf" => "application/rtf", "eml" => "message/rfc822",
        "doc" => "application/msword", "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel", "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint", "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "mp3" => "audio/mpeg", "m4a" => "audio/mp4", "wav" => "audio/wav", "mp4" => "video/mp4", "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }.to_string()
}

/// The files with the state of the disk NOW: a file of this Mac that is no longer there reads `Gone` (the fixture core's rule).
fn refreshed(list: Vec<OutgoingAttachment>) -> Vec<OutgoingAttachment> {
    list.into_iter().map(|mut a| {
        if let AttachmentSource::LocalFile { path } = &a.source {
            if disk_size(path).is_none() { a.state = OutgoingAttachmentState::Gone; }
        }
        a
    }).collect()
}

/// An unknown draft is NotFound; a draft that was queued or sent is refused (`attachment_draft_closed`).
fn draft_open(store: &Store, draft_id: &str) -> Result<(), CoreError> {
    let (open, queued) = store.draft_open_or_queued(draft_id).map_err(|e| store_failed("the draft could not be read", e))?;
    if !open && !queued { return Err(CoreError::NotFound { kind: "draft".into(), id: draft_id.to_string() }); }
    check_attachment_draft_open(open)
}

/// `AddDraftAttachment`. The disk (or the stored attachment) is read first, the contract's rule decides, the row is written with the draft's
/// `attachment_ids` in one transaction. The same source twice answers the existing id.
pub fn add(store: &Store, draft_id: &str, source: AttachmentSource) -> Result<CommandOutcome, CoreError> {
    draft_open(store, draft_id)?;
    let (stored_name, size, local, media) = match &source {
        AttachmentSource::LocalFile { path } => (None, disk_size(path), false, None),
        AttachmentSource::StoredMessage { message_id, attachment_id } if !message_id.trim().is_empty() && !attachment_id.trim().is_empty() => {
            let (a, ..) = store.stored_attachment(message_id, attachment_id).map_err(|e| store_failed("the attachment could not be read", e))?
                .ok_or_else(|| CoreError::NotFound { kind: "attachment".into(), id: attachment_id.clone() })?;
            let here = a.local_path.as_deref().is_some_and(|p| disk_size(p).is_some());
            (Some(a.file_name), Some(a.size_bytes), here, Some(a.mime_type))
        }
        AttachmentSource::StoredMessage { .. } => (None, None, false, None),
    };
    let draft = draft_id.to_string();
    let outcome = store.write(move |ctx| {
        let existing = ctx.draft_files_in(&draft)?;
        let plan = match plan_add_attachment(existing, source.clone(), stored_name, size, local) { Ok(p) => p, Err(e) => return Ok(Err(e)) };
        match plan {
            AttachmentAddPlan::AlreadyThere { attachment_id } => Ok(Ok(attachment_id)),
            AttachmentAddPlan::Add { state, file_name } => {
                let id = format!("{draft}/{}", ctx.draft_file_next_order(&draft)?);
                let media_type = media.unwrap_or_else(|| media_type_of(&file_name));
                // fix-realcore round 7: the name kept and shown is the composed form (a picked or pasted file's name comes decomposed from macOS)
                let file_name = parse::nfc(&file_name);
                let row = OutgoingAttachment { id: id.clone(), owner_id: draft.clone(), source, file_name, media_type, size_bytes: size.unwrap_or(0), state };
                ctx.draft_file_insert(&row)?;
                ctx.outgoing_sync_draft_ids(&draft)?;
                Ok(Ok(id))
            }
        }
    }).map_err(|e| store_failed("the file was not added to the draft", e))??;
    Ok(CommandOutcome { undo_entry_id: None, created_id: Some(outcome) })
}

/// `RemoveDraftAttachment`: the row goes (the file of this Mac stays where it is). An unknown file is NotFound.
pub fn remove(store: &Store, draft_id: &str, attachment_id: &str) -> Result<CommandOutcome, CoreError> {
    draft_open(store, draft_id)?;
    let (draft, id) = (draft_id.to_string(), attachment_id.to_string());
    let gone = store.write(move |ctx| {
        let gone = ctx.draft_file_delete(&draft, &id)?;
        if gone { ctx.outgoing_sync_draft_ids(&draft)?; }
        Ok(gone)
    }).map_err(|e| store_failed("the file was not removed from the draft", e))?;
    if !gone { return Err(CoreError::NotFound { kind: "attachment".into(), id: attachment_id.to_string() }); }
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

/// `draft/<id>/attachments` and `outbox/<id>/attachments` (the contract's JSON, the disk re-read). Every other path: None.
pub fn state(store: &Store, path: &str) -> Option<String> {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        // engine-act-r6 round 6: the whole text of a saved draft (DraftSource.swift reopens a draft with it; `drafts()` carries its first lines only)
        ["draft", id, "body"] => store.draft_body(id).ok().flatten().map(|b| crate::reads::json_text(&b)),
        ["draft", id, "attachments"] => store.draft_files(id).ok().map(|l| outgoing_attachments_json(refreshed(l))),
        ["outbox", id, "attachments"] => store.outbox_files(id).ok().map(|l| outgoing_attachments_json(refreshed(l))),
        // attach-engine (delta 23): a received file's transfer
        ["attachment", _, "transfer"] => download::state(store, path),
        // fix-realcore round 3 (lead task 1ed1447c): what the hidden `--slice-attachsend` reads to prove a received file end to end: the files of
        // a stored message (id, name, size, inline, on this Mac) and where a file on this Mac sits with its size. Store reads only, no network.
        ["message", id, "files"] => store.attachments(id).ok().map(|l| files_json(&l)),
        ["attachment", id, "local"] => Some(local_json(store, id)),
        _ => None,
    }
}

/// `message/<id>/files`: `[{"id","file_name","size_bytes","inline","on_this_mac"}]` in the store's order.
fn files_json(list: &[contracts::AttachmentFile]) -> String {
    let rows: Vec<String> = list.iter().map(|a| {
        let here = a.local_path.as_deref().is_some_and(|p| std::path::Path::new(p).is_file());
        format!(r#"{{"id":{},"file_name":{},"size_bytes":{},"inline":{},"on_this_mac":{}}}"#,
                crate::reads::json_text(&a.id), crate::reads::json_text(&a.file_name), a.size_bytes, a.inline, here)
    }).collect();
    format!("[{}]", rows.join(","))
}

/// `attachment/<id>/local`: `{"path": <the file on this Mac>, "size": <its bytes>}`, or `{"path":null}` when it is not on this Mac (or unknown).
fn local_json(store: &Store, attachment_id: &str) -> String {
    let file = store.attachment_source(attachment_id).ok().flatten()
        .and_then(|src| store.stored_attachment(&src.message_id, attachment_id).ok().flatten())
        .and_then(|(a, ..)| a.local_path)
        .and_then(|p| std::fs::metadata(&p).ok().filter(|m| m.is_file()).map(|m| (p, m.len())));
    match file {
        Some((p, n)) => format!(r#"{{"path":{},"size":{}}}"#, crate::reads::json_text(&p), n),
        None => r#"{"path":null}"#.to_string(),
    }
}

/// Before a draft is queued: a file gone since it was added, a file over the cap, the total over the cap are refused in words and nothing is
/// queued (the contract's `check_draft_attachments_sendable` on the files as the disk is now).
pub fn check_sendable(store: &Store, draft_id: &str) -> Result<(), CoreError> {
    let files = store.draft_files(draft_id).map_err(|e| store_failed("the draft's files could not be read", e))?;
    check_draft_attachments_sendable(refreshed(files))
}

/// "Sat 7 Nov, 10:00" in this Mac's time (the forward header's date, as the fixture core writes it).
fn when_words(ms: i64, offset_s: i32) -> String {
    let secs = ms.div_euclid(1000) + offset_s as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // days since 1970-01-01 to a civil date (H. Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    format!("{} {} {}, {:02}:{:02}", DAYS[days.rem_euclid(7) as usize], day, MONTHS[(month - 1) as usize], rem / 3600, (rem % 3600) / 60)
}

/// `Forward { message_id }`: a draft with no recipient, "Fwd: " before the subject, the original below the forward header with its files
/// listed by name, AND every file of the original on the draft (`forward_attachments`: in the original's order, ids `<draft id>/<n>`, one
/// over the per-file cap listed `TooLarge`), all in ONE transaction. `created_id` is the draft.
pub fn forward(store: &Store, message_id: &str, now_ms: i64, offset_s: i32) -> Result<CommandOutcome, CoreError> {
    let m = store.message(message_id).map_err(|e| store_failed("the message could not be read", e))?
        .ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: message_id.to_string() })?;
    let original = store.attachments(message_id).map_err(|e| store_failed("the message's files could not be read", e))?;
    let text = store.body_parts(message_id).ok().flatten().and_then(|p| p.body_text.or_else(|| p.body_html.as_deref().map(parse::safe_html_to_text))).unwrap_or_else(|| m.preview.clone());
    let names: Vec<String> = original.iter().map(|a| a.file_name.clone()).collect();
    let body = forward_body(m.sender_name.clone(), m.from_address.clone(), m.subject.clone(), when_words(m.received_at_ms, offset_s), names, text);
    let subject = prefixed_subject(m.subject.clone(), "Fwd".into());
    let draft_id = store::new_id("draft");
    let (id, account) = (draft_id.clone(), m.account_id.clone());
    store.write(move |ctx| {
        ctx.tx.execute(
            "INSERT INTO draft (id, from_account_id, to_json, cc_json, subject, body, reply_to_thread_id, updated_at_ms) VALUES (?1, ?2, '[]', '[]', ?3, ?4, NULL, ?5)",
            rusqlite::params![id, account, subject, body, now_ms],
        )?;
        for row in forward_attachments(id.clone(), original) { ctx.draft_file_insert(&row)?; }
        ctx.outgoing_sync_draft_ids(&id)?;
        Ok(())
    }).map_err(|e| store_failed("the forward draft was not saved", e))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: Some(draft_id) })
}

/// The outbox's `AttachmentFilesPort`: a queued draft's files, READ when its message is built.
pub struct DraftFiles { pub store: Arc<Store>, pub deps: Arc<SyncDeps>, pub runtime: tokio::runtime::Handle }

fn gone(name: &str) -> CoreError { CoreError::Invalid { field: ATTACHMENT_GONE_CODE.into(), reason_words: format!("\"{name}\" is no longer there: remove it or attach it again") } }

impl DraftFiles {
    /// A forwarded file: from this Mac when its bytes are here, else its message from the server and the file taken out of it.
    fn stored_bytes(&self, row: &OutgoingAttachment, message_id: &str, attachment_id: &str) -> Result<Vec<u8>, CoreError> {
        let (a, folder, uid, account) = self.store.stored_attachment(message_id, attachment_id).map_err(|e| store_failed("the attachment could not be read", e))?
            .ok_or_else(|| CoreError::NotFound { kind: "attachment".into(), id: attachment_id.to_string() })?;
        if let Some(bytes) = a.local_path.as_deref().and_then(|p| std::fs::read(expand(p)).ok()) { return Ok(bytes); }
        let fail = |why: String| CoreError::Action { failure: ActionFailed { account_name: account.clone(), reason_words: format!("\"{}\" could not be fetched from the mail server, so the message was not sent: {why}", row.file_name), retryable: true, code: None } };
        // body-r3: a row the sync wrote knows its part number: ONE `BODY.PEEK[<part>]` for that file, never the whole message (`Bodies::attachment_bytes`,
        // which also keeps the file on this Mac). A row with no part number (written by hand, or a sent copy's) takes the whole-message way below.
        if self.store.attachment_source(attachment_id).ok().flatten().is_some_and(|s| !s.part.is_empty()) {
            let (bodies, id) = (self.deps.bodies.clone(), attachment_id.to_string());
            return std::thread::scope(|s| s.spawn(|| self.runtime.block_on(async { bodies.attachment_bytes(&id).await })).join())
                .map_err(|_| fail("the server call stopped unexpectedly".into()))?
                .map_err(|e| fail(e.to_string()));
        }
        let uid: u32 = uid.as_deref().and_then(|u| u.parse().ok()).ok_or_else(|| fail("its message has no place on the server".into()))?;
        let adapter = self.deps.adapters.lock().unwrap_or_else(|e| e.into_inner()).get(&account).cloned().ok_or_else(|| fail("this account is not connected to its mail server yet".into()))?;
        let raw = std::thread::scope(|s| s.spawn(|| self.runtime.block_on(async { adapter.fetch_raw(&folder, uid).await })).join())
            .map_err(|_| fail("the server call stopped unexpectedly".into()))?
            .map_err(|e| fail(e.to_string()))?.0;
        // the nth attachment of that name: two files of one message may share a name
        let same: Vec<String> = self.store.attachments(message_id).unwrap_or_default().into_iter().filter(|x| x.file_name == a.file_name).map(|x| x.id).collect();
        let nth = same.iter().position(|x| x == attachment_id).unwrap_or(0);
        send::part_bytes(&raw, &a.file_name, nth).ok_or_else(|| fail("the server's copy of the message holds no such file".into()))
    }
}

impl outbox::AttachmentFilesPort for DraftFiles {
    fn execute(&self, draft: &Draft) -> Result<Vec<send::OutgoingFile>, CoreError> {
        let rows = self.store.outgoing_attachments_by_ids(&draft.attachment_ids).map_err(|e| store_failed("the message's files could not be read", e))?;
        if rows.len() != draft.attachment_ids.len() { return Err(unexpected("a file of this message is no longer listed, so it was not sent")); }
        check_draft_attachments_sendable(refreshed(rows.clone()))?;
        rows.iter().map(|row| {
            let bytes = match &row.source {
                AttachmentSource::LocalFile { path } => std::fs::read(expand(path)).map_err(|_| gone(&row.file_name))?,
                AttachmentSource::StoredMessage { message_id, attachment_id } => self.stored_bytes(row, message_id, attachment_id)?,
            };
            Ok(send::OutgoingFile { file_name: row.file_name.clone(), media_type: row.media_type.clone(), bytes })
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::when_words;
    #[test]
    fn the_forward_date_reads_as_the_fixture_writes_it() {
        // 2026-11-07 10:00 UTC was a Saturday
        assert_eq!(when_words(1_794_045_600_000, 0), "Sat 7 Nov, 10:00");
        assert_eq!(when_words(1_794_045_600_000, 3600), "Sat 7 Nov, 11:00");
    }
}
