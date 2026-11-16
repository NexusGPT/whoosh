//! body-r3: the files of a received message. When a body is fetched and parsed, each attachment part of the message becomes an `attachment` row
//! (name, type, size, content id, inline flag, the part's IMAP number, its transfer encoding): `rows_of`. The bytes are another matter.
//!
//! WHAT THE FETCH BRINGS: the body fetch is `UID FETCH ... BODY.PEEK[]`, the WHOLE message, every attachment in base64 included (about 1.37 times
//! the file). So the bytes are in memory when the rows are written, and keeping them costs disk, not network. A 10 MB file is about 13.7 MB on the
//! wire and 10 MB decoded on disk (the base64 raw message is never kept). The rule (`ATTACHMENT_KEEP_MAX_BYTES` per file):
//!   - a body the READER asked for (the owner is looking at that message) keeps its files on this Mac, at once, up to the cap per file;
//!   - the prefetch, the crawl and the backfill (background: every mail of every folder) write ROWS ONLY: a mailbox's files are never all stored;
//!   - any other file is fetched when it is wanted: `Bodies::attachment_bytes` asks the server for that one part (`BODY.PEEK[<part>]`), decodes it,
//!     keeps it (`local_path`) and answers; the next call reads the file. A view never waits for it: `RustCore::attachments` reads the rows
//!     (a row with no `local_path` is "not on this Mac yet") and `Bodies::request_attachment` fetches in the background.
//!
//! ROWS FOR MAIL ALREADY STORED: no raw message is stored, only the text and the sanitised html, so a stored body cannot be parsed for its files.
//! The rows of such a message are written when that body is fetched again. A one-time pass does that: for each account, once its source is
//! registered, the messages that have a stored body, no file row and a size (RFC822.SIZE) well over their stored text (`ATTACHMENT_BACKFILL_GAP_BYTES`)
//! are fetched again, newest first, as the crawl fetches (quiet, rows only). `<data dir>/attachment-backfill-<account>.done` says it ran.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::constants::{BODY_FETCH_RETRY_S, BODY_PREFETCH_BATCH};
use contracts::CoreError;
use store::{AttachmentIn, AttachmentSource, Store};

use super::{lock, Bodies};

/// The largest file a fetched body keeps at once (a body the reader asked for): a file over it is fetched when it is wanted. PROPOSED for
/// docs/CONSTANTS.md: depends on disk use per opened message and the message-size limit of the smallest server (ATTACHMENT_FILE_MAX_BYTES, 10 MiB).
pub const ATTACHMENT_KEEP_MAX_BYTES: u64 = 25 * 1024 * 1024;
/// How much a message's size must exceed its stored text and html for the backfill to fetch it again. PROPOSED for docs/CONSTANTS.md.
pub const ATTACHMENT_BACKFILL_GAP_BYTES: u64 = 30_000;
/// The most messages one backfill pass reads.
const BACKFILL_MAX: usize = 5000;

fn unexpected(why: String) -> CoreError { CoreError::Unexpected { reason_words: why } }

/// The row a parsed attachment becomes. A part with no name is called after its place (`attachment-2`), `.eml` for a nested message.
pub(crate) fn rows_of(files: &[parse::Attachment]) -> Vec<AttachmentIn> {
    files.iter().enumerate().map(|(i, a)| AttachmentIn {
        part: a.part.clone(),
        name: a.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| format!("attachment-{}{}", i + 1, if a.content_type == "message/rfc822" { ".eml" } else { "" })),
        mime_type: a.content_type.clone(), size_bytes: a.size as u64, content_id: a.content_id.clone(), inline: a.inline, encoding: a.encoding.clone(),
    }).collect()
}

/// FNV-1a of the message id, in hex: the folder of its files (a message id holds characters a path should not).
fn folder_key(message_id: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in message_id.bytes() { h ^= u64::from(b); h = h.wrapping_mul(0x0100_0000_01b3); }
    format!("{h:016x}")
}

/// A file name safe as one path component: no separator, no control character, at most 120 bytes.
fn safe_name(name: &str) -> String {
    let mut s: String = name.chars().map(|c| if c == '/' || c == '\\' || c == ':' || c.is_control() { '_' } else { c }).collect();
    while s.len() > 120 { s.pop(); }
    let s = s.trim().trim_start_matches('.').to_string();
    if s.is_empty() { "file".to_string() } else { s }
}

/// Writes one file under `<data dir>/attachments/<folder key>/<ordinal>-<name>`, whole or not at all (a temporary file, then a rename).
pub(crate) fn write_file(data_dir: &Path, message_id: &str, ordinal: usize, name: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let dir = data_dir.join("attachments").join(folder_key(message_id));
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(format!("{ordinal:03}-{}", safe_name(name)));
    let tmp = dir.join(format!(".{ordinal:03}.part"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, &target)?;
    Ok(target)
}

fn data_dir(store: &Store) -> PathBuf { store.path().parent().unwrap_or_else(|| Path::new(".")).to_path_buf() }

/// Removes the folders of files whose message is gone (a row goes with its message, its file does not): every folder under
/// `<data dir>/attachments` that no row's `local_path` is in. Runs at core start. Answers how many folders it removed.
pub fn sweep_orphan_files(store: &Store) -> usize {
    let root = data_dir(store).join("attachments");
    let Ok(paths) = store.attachment_paths() else { return 0 };
    let used: HashSet<PathBuf> = paths.iter().filter_map(|p| Path::new(p).parent().map(Path::to_path_buf)).collect();
    let Ok(entries) = std::fs::read_dir(&root) else { return 0 };
    let mut removed = 0;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && !used.contains(&p) && std::fs::remove_dir_all(&p).is_ok() { removed += 1; }
    }
    removed
}

impl Bodies {
    /// The file rows of one fetched message and, for a message the reader asked for, its files to keep now (ordinal, name, bytes: the ones
    /// within `ATTACHMENT_KEEP_MAX_BYTES`). A message whose files cannot be read has no row and no file: that is no failure of its body.
    pub(crate) fn files_of(&self, message_id: &str, raw: &[u8]) -> (Vec<AttachmentIn>, Vec<(usize, String, Vec<u8>)>) {
        if !lock(&self.reader_asked).remove(message_id) {
            return (parse::attachments_of(raw).map(|f| rows_of(&f)).unwrap_or_default(), Vec::new());
        }
        let Ok(with_bytes) = parse::attachments_with_bytes(raw) else { return (Vec::new(), Vec::new()) };
        let rows = rows_of(&with_bytes.iter().map(|(a, _)| a.clone()).collect::<Vec<_>>());
        let keep = with_bytes.into_iter().enumerate().filter(|(_, (_, b))| b.len() as u64 <= ATTACHMENT_KEEP_MAX_BYTES)
            .map(|(i, (_, b))| (i, rows[i].name.clone(), b)).collect();
        (rows, keep)
    }

    /// Writes the files `files_of` chose and records each path. Called after the rows exist.
    pub(crate) fn keep_files(&self, message_id: &str, files: Vec<(usize, String, Vec<u8>)>) {
        let dir = data_dir(&self.store);
        for (ordinal, name, bytes) in files {
            match write_file(&dir, message_id, ordinal, &name, &bytes) {
                Ok(path) => { if let Err(e) = self.store.set_attachment_local_path(&store::attachment_row_id(message_id, ordinal), Some(&path.to_string_lossy())) {
                    crate::log::log_line(&self.log, &format!("attachment {message_id}: the file's place was not recorded: {e}")); } }
                Err(e) => crate::log::log_line(&self.log, &format!("attachment {message_id}: the file {name} was not kept: {e}")),
            }
        }
    }

    /// ONE function for "the bytes of attachment X" (the forward path and the gallery call it; `core.background().attachment_bytes(id)`): the file
    /// on this Mac when it is there, else that one part from the server (`BODY.PEEK[<part>]`, no \Seen), decoded, kept at `local_path`. The same
    /// file asked twice at once is fetched once. NOT for a view's read: it may wait for the network (use `request_attachment` there).
    pub async fn attachment_bytes(self: &Arc<Self>, attachment_id: &str) -> Result<Vec<u8>, CoreError> {
        let gate = lock(&self.att_locks).entry(attachment_id.to_string()).or_default().clone();
        let _one = gate.lock().await;
        let src = self.store.attachment_source(attachment_id).map_err(|e| unexpected(format!("the attachment could not be read from the store: {e}")))?
            .ok_or_else(|| unexpected(format!("no such attachment: {attachment_id}")))?;
        if let Some(p) = src.local_path.clone() {
            if let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || std::fs::read(p)).await { return Ok(bytes); }
            // the file is gone from this Mac (the owner deleted it): the row says so, and it is fetched again
            let _ = self.store.set_attachment_local_path(&src.id, None);
        }
        self.fetch_and_keep(&src).await
    }

    /// Fetches one attachment's part, decodes it, keeps the file and records its path.
    async fn fetch_and_keep(self: &Arc<Self>, src: &AttachmentSource) -> Result<Vec<u8>, CoreError> {
        // where the message sits now: the place a move recorded, else the folder and UID it was synced under
        let place = self.store.message_place(&src.message_id).ok().flatten();
        let (folder, uid) = match (place, src.uid) {
            (Some(p), _) => (p.folder, p.uid),
            (None, Some(u)) => (src.folder.clone(), u),
            (None, None) => return Err(unexpected(format!("the message of {} has no place on a server", src.file_name))),
        };
        if src.part.is_empty() { return Err(unexpected(format!("{} has no part number: its message must be read again first", src.file_name))); }
        let source = self.source(&src.account_id).ok_or_else(|| unexpected("this account is not connected to its mail server yet".to_string()))?;
        let raw = source.fetch_part(&folder, uid, &src.part).await?;
        let bytes = parse::decode_transfer(&src.encoding, &raw);
        if bytes.len() as u64 != src.size_bytes {
            crate::log::log_line(&self.log, &format!("attachment {}: the part came back as {} bytes, the message said {}", src.id, bytes.len(), src.size_bytes));
        }
        let (store, dir, id, message_id, name, ordinal, data) = (self.store.clone(), data_dir(&self.store), src.id.clone(), src.message_id.clone(), src.file_name.clone(),
            src.id.rsplit(':').next().and_then(|n| n.parse::<usize>().ok()).unwrap_or(0), bytes.clone());
        let kept = tokio::task::spawn_blocking(move || -> Result<(), String> {
            let path = write_file(&dir, &message_id, ordinal, &name, &data).map_err(|e| format!("the file could not be kept: {e}"))?;
            store.set_attachment_local_path(&id, Some(&path.to_string_lossy())).map_err(|e| format!("the file's place could not be recorded: {e}"))?;
            Ok(())
        }).await;
        match kept {
            Ok(Ok(())) => {}
            Ok(Err(why)) => crate::log::log_line(&self.log, &format!("attachment {}: {why}", src.id)),
            Err(e) => crate::log::log_line(&self.log, &format!("attachment {}: {e}", src.id)),
        }
        Ok(bytes)
    }

    /// The background twin of `attachment_bytes` for a view: returns at once, fetches on the core's runtime, once at a time per file, not again
    /// within `BODY_FETCH_RETRY_S` of a failure (core.log has the reason). The row's `local_path` appears when the file lands (the Inbox stream
    /// is told by the store's own change feed).
    pub fn request_attachment(self: &Arc<Self>, attachment_id: &str) {
        {
            let mut flying = lock(&self.att_flying);
            if flying.contains(attachment_id) { return; }
            if lock(&self.att_failed).get(attachment_id).is_some_and(|at| at.elapsed() < Duration::from_secs(BODY_FETCH_RETRY_S)) { return; }
            flying.insert(attachment_id.to_string());
        }
        let (me, id) = (self.clone(), attachment_id.to_string());
        self.runtime.spawn(async move {
            if let Err(e) = me.attachment_bytes(&id).await {
                crate::log::log_line(&me.log, &format!("attachment {id}: {}", super::words(&e)));
                lock(&me.att_failed).insert(id.clone(), Instant::now());
            }
            lock(&me.att_flying).remove(&id);
        });
    }

    /// The one-time pass for mail stored before this build (see the header): once per account, at the first registration of its source.
    pub(crate) fn backfill_attachment_rows(self: &Arc<Self>, account: &str) {
        let marker = data_dir(&self.store).join(format!("attachment-backfill-{}.done", folder_key(account)));
        if marker.exists() || !lock(&self.backfilled).insert(account.to_string()) { return; }
        let (me, account) = (self.clone(), account.to_string());
        self.runtime.spawn(async move {
            let store = me.store.clone();
            let acc = account.clone();
            let candidates = tokio::task::spawn_blocking(move || store.attachment_backfill_candidates(&acc, ATTACHMENT_BACKFILL_GAP_BYTES, BACKFILL_MAX)).await;
            let Ok(Ok(candidates)) = candidates else { return };
            let mut folders: Vec<String> = Vec::new();
            for (_, _, f) in &candidates { if !folders.contains(f) { folders.push(f.clone()); } }
            let mut complete = true;
            for folder in folders {
                let of_folder: Vec<(u32, String)> = candidates.iter().filter(|(_, _, f)| *f == folder).map(|(u, id, _)| (*u, id.clone())).collect();
                for chunk in of_folder.chunks(BODY_PREFETCH_BATCH as usize) {
                    let claimed = me.claim(chunk.to_vec());
                    if claimed.is_empty() { continue; }
                    if !me.download_with(&account, &folder, claimed, true).await { complete = false; }
                }
            }
            if complete {
                if let Err(e) = std::fs::write(&marker, format!("attachment rows read again for {} messages\n", candidates.len())) {
                    crate::log::log_line(&me.log, &format!("attachment backfill: the marker was not written: {e}"));
                }
            }
        });
    }
}
