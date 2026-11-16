//! attach-engine (contracts delta 23): `DownloadAttachment { attachment_id }` in the real core, its state path `attachment/<id>/transfer` and its event
//! `AttachmentTransferChanged`. The command ANSWERS AT ONCE (a store read, the contract's plan, a task queued on the core's runtime): the fetch runs in
//! the background and ends in the event. No view waits for the network: the state path is a store read plus this module's two small maps.
//!
//! THE FETCH IS NOT NEW: it is `Bodies::attachment_bytes` (body-r3, the one function the Forward path calls too): one `BODY.PEEK[<part>]`, decoded, the
//! file written under the data folder (a temporary file, then a rename) and its path recorded in the row. This module adds only what the contract asks
//! around it: the rule (`plan_download_attachment`), the "coming now" and "failed, in words" memory (`attachment_transfer_of`), the event.
//!
//! STATE LIVES HERE, NOT IN `RustCore`: `rust_core.rs` is not this lane's, so the two maps (`running`, `failed`) sit in a registry keyed by the core's
//! store (a `Weak<Store>` proves the entry is of a core still alive, so a core opened again on the same folder starts clean). A restart forgets a failure
//! (the tile reads "not on this Mac" and the owner asks again) and a fetch cut short by the restart leaves no file (`write_file` renames whole files only).
//!
//! THE TEXT OF THE FILE (second round): when a file has landed, its text is read from the disk copy (no second fetch) with what exists: plain text read by
//! the core, a PDF through `attachment_index::extract_pdf_text` (the reader `AttachmentIndexer` and reply-sources use), at most ATTACHMENT_TEXT_MAX_CHARS
//! characters. It is filed through the store's existing writer (`attachment_index_save`: `attachment.extracted_text`, which the word index reads, and
//! `attachment_content`) and cut by the indexer's own rule (`chunk_text`, CHUNK_TOKENS, CHUNK_OVERLAP) into `chunk` rows `attachment:<id>` under the message
//! (`Store::replace_attachment_chunks`). A file with no text (a picture, a scan, a kind only the OS can read: docx, rtf, odt) stores nothing and leaves ONE
//! core.log line, never an error. NOT DONE: embedding those chunks (indexer.rs, not this lane's), and the OS readers (Swift ports the engine does not hold).
//! The reader's own body fetch keeps files but nothing in the engine reads their text either: `AttachmentIndexer` has no caller in the engine.
//!
//! BOUNDS: a file over `ATTACHMENT_KEEP_MAX_BYTES` is refused in words before any fetch (`plan_download_attachment`, code `attachment_too_large_to_keep`);
//! a part that arrives bigger than that anyway (the row's size was the sender's claim) is not kept and fails in words. LIMIT, named: the bytes of ONE part
//! are held in memory while they are decoded (`ImapMailAdapter::fetch_part` and `Bodies::fetch_and_keep` return a `Vec`; both are outside this lane), so
//! "to disk as they arrive" needs a streaming fetch in sync-imap: at most about 1.4 x 25 MiB of base64, plus the decoded copy, at a time per file.
//! A server that goes silent ends in the adapter's own time limit (`IMAP_COMMAND_TIMEOUT_MS`), which becomes the failure words.
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use contracts::{
    attachment_transfer_json, attachment_transfer_of, plan_download_attachment, AttachmentFile, AttachmentTransfer, CommandOutcome, CoreError, CoreEvent, DownloadPlan,
    StreamKind, ATTACHMENT_KEEP_MAX_BYTES,
};
use attachment_index::{extract_pdf_text, AttachmentIndexEntry, AttachmentKind, IndexState};
use search_vector::constants::{CHUNK_OVERLAP, CHUNK_TOKENS};
use search_vector::{chunk_text, WhitespaceTokenizer};
use store::Store;

use crate::bodies::Bodies;
use crate::sync::{emit, Sinks};

// attach-fetch (SPEC A152): the text attachments of stored mail are fetched in the background with this file's own fetch and second round (attachment_auto.rs).
#[path = "attachment_auto.rs"]
pub mod auto;

/// What one core knows about its fetches: the files coming now, and the words of the last failure of each file that has not been asked again since.
#[derive(Default)]
pub struct Transfers {
    running: Mutex<HashSet<String>>,
    failed: Mutex<HashMap<String, String>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> { m.lock().unwrap_or_else(|e| e.into_inner()) }

/// One entry per live core: the store's address, a `Weak` of it (it keeps the allocation, so the address cannot be handed to a newer store while the
/// entry stands) and the maps.
fn registry() -> &'static Mutex<HashMap<usize, (Weak<Store>, Arc<Transfers>)>> {
    static R: OnceLock<Mutex<HashMap<usize, (Weak<Store>, Arc<Transfers>)>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn transfers_of(store: &Arc<Store>) -> Arc<Transfers> {
    let mut reg = lock(registry());
    reg.retain(|_, (w, _)| w.strong_count() > 0);
    reg.entry(Arc::as_ptr(store) as usize).or_insert_with(|| (Arc::downgrade(store), Arc::new(Transfers::default()))).1.clone()
}

/// The maps of the core that owns `store`; None when it never started a download (nothing is running, nothing failed).
fn existing(store: &Store) -> Option<Arc<Transfers>> {
    let reg = lock(registry());
    reg.get(&(store as *const Store as usize)).filter(|(w, _)| w.strong_count() > 0).map(|(_, t)| t.clone())
}

/// The file as the contract reads it: `local_path` only when the file is really there (a path whose file the owner deleted reads as not here).
fn file_of(store: &Store, attachment_id: &str) -> Result<Option<AttachmentFile>, CoreError> {
    let unexpected = |e: store::StoreError| CoreError::Unexpected { reason_words: format!("the attachment could not be read: {e}") };
    let Some(src) = store.attachment_source(attachment_id).map_err(unexpected)? else { return Ok(None) };
    let Some((mut file, ..)) = store.stored_attachment(&src.message_id, attachment_id).map_err(unexpected)? else { return Ok(None) };
    if file.local_path.as_deref().is_some_and(|p| !Path::new(p).is_file()) { file.local_path = None; }
    Ok(Some(file))
}

/// `DownloadAttachment`. Idempotent: a file already here, or already coming, answers Ok and starts nothing. `runtime` is the core's own.
pub fn run(store: &Arc<Store>, bodies: &Arc<Bodies>, sinks: &Sinks, runtime: Option<tokio::runtime::Handle>, attachment_id: &str) -> Result<CommandOutcome, CoreError> {
    let done = || Ok(CommandOutcome { undo_entry_id: None, created_id: None });
    let file = file_of(store, attachment_id)?;
    let t = transfers_of(store);
    {
        // the check and the claim are one step under the lock: two commands at once start ONE fetch
        let mut running = lock(&t.running);
        match plan_download_attachment(attachment_id.to_string(), file.clone(), running.contains(attachment_id))? {
            DownloadPlan::AlreadyOnThisMac | DownloadPlan::AlreadyDownloading => return done(),
            DownloadPlan::Start => {}
        }
        let Some(rt) = runtime else {
            return Err(CoreError::Unexpected { reason_words: "the core has nothing to fetch the file with right now".into() });
        };
        running.insert(attachment_id.to_string());
        lock(&t.failed).remove(attachment_id);
        let (store, bodies, sinks, t, id, message_id) = (store.clone(), bodies.clone(), sinks.clone(), t.clone(), attachment_id.to_string(), file.map(|f| f.message_id).unwrap_or_default());
        rt.spawn(async move {
            let outcome = fetch(&store, &bodies, &id).await;
            // the maps are final BEFORE the event, so a tile that re-reads on it sees the end of the story
            match &outcome { Ok(()) => { lock(&t.failed).remove(&id); } Err(words) => { lock(&t.failed).insert(id.clone(), words.clone()); } }
            lock(&t.running).remove(&id);
            let transfer = match outcome { Ok(()) => AttachmentTransfer::OnThisMac, Err(reason_words) => AttachmentTransfer::Failed { reason_words } };
            let landed = matches!(transfer, AttachmentTransfer::OnThisMac);
            emit(&sinks, StreamKind::Thread, CoreEvent::AttachmentTransferChanged { attachment_id: id.clone(), message_id, transfer });
            // the tile is already told; the text of the file joins the indexes after (a file with no text stores nothing and says so in core.log)
            if landed {
                let log = crate::log::path_beside(store.path());
                let _ = tokio::task::spawn_blocking(move || index_text(&store, &id, &log)).await;
            }
        });
    }
    done()
}

/// The background fetch: `Bodies::attachment_bytes`, then the proof that the file is on this Mac and within the cap. Err = the failure in words.
async fn fetch(store: &Arc<Store>, bodies: &Arc<Bodies>, id: &str) -> Result<(), String> {
    let bytes = bodies.attachment_bytes(id).await.map_err(|e| format!("The file could not be fetched from the mail server: {}", crate::bodies::words(&e)))?;
    let size = bytes.len() as u64;
    drop(bytes);
    let kept = store.attachment_source(id).ok().flatten().and_then(|s| s.local_path);
    if size > ATTACHMENT_KEEP_MAX_BYTES {
        // the sender's size was a claim: what came is over the cap, so it is not kept
        if let Some(p) = &kept { let _ = std::fs::remove_file(p); }
        let _ = store.set_attachment_local_path(id, None);
        return Err(format!("the file is bigger than the {} MB kept on this Mac: open it on the server", ATTACHMENT_KEEP_MAX_BYTES / (1024 * 1024)));
    }
    match kept {
        Some(p) if Path::new(&p).is_file() => Ok(()),
        _ => Err("the file came from the server but could not be kept on this Mac".into()),
    }
}

/// `attachment/<id>/transfer` (the contract's JSON). None for a path that is not one, or a file this core does not know.
pub fn state(store: &Store, path: &str) -> Option<String> {
    let parts = path.split('/').collect::<Vec<_>>();
    let ["attachment", id, "transfer"] = parts.as_slice() else { return None };
    // attach-fetch: `attachment/auto-fetch/transfer` is the automatic fetch's counts, not a file (attachment_auto.rs)
    if path == auto::STATE_PATH { return Some(auto::state_json(store)); }
    let file = file_of(store, id).ok().flatten()?;
    let t = existing(store);
    let in_flight = t.as_ref().is_some_and(|t| lock(&t.running).contains(*id));
    let failure = t.as_ref().and_then(|t| lock(&t.failed).get(*id).cloned());
    Some(attachment_transfer_json(attachment_transfer_of(file, in_flight, failure)))
}

/// The text of the file on disk, or why there is none (in words, for the log).
fn text_of_file(path: &Path, kind: AttachmentKind) -> Result<String, String> {
    let max = attachment_index::constants::ATTACHMENT_TEXT_MAX_CHARS;
    let text = match kind {
        AttachmentKind::PlainText => {
            use std::io::Read;
            // at most 4 bytes a character: a file bigger than the cap is never read whole
            let limit = max.map(|m| m as u64 * 4).unwrap_or(ATTACHMENT_KEEP_MAX_BYTES);
            let mut bytes = Vec::new();
            std::fs::File::open(path).and_then(|f| f.take(limit).read_to_end(&mut bytes)).map_err(|e| format!("the file could not be read: {e}"))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        AttachmentKind::Pdf => {
            let bytes = std::fs::read(path).map_err(|e| format!("the file could not be read: {e}"))?;
            extract_pdf_text(&bytes).ok_or_else(|| "a PDF with no text layer the core can read (a scan, an encrypted or unusual file)".to_string())?
        }
        AttachmentKind::Document => return Err("a document only the OS readers can open (not available to the core)".into()),
        AttachmentKind::Image | AttachmentKind::Audio | AttachmentKind::Video => return Err("a picture, sound or video: its text needs the OS readers".into()),
        AttachmentKind::Unreadable => return Err("a kind of file that holds no text".into()),
    };
    let text: String = match max { Some(m) => text.chars().take(m).collect(), None => text };
    if text.trim().is_empty() { Err("the file holds no text".into()) } else { Ok(text) }
}

/// Files the text of one landed file (see the header). Never an error to the caller: what it could not do is one line in core.log.
fn index_text(store: &Arc<Store>, id: &str, log: &Path) {
    let say = |why: &str| crate::log::log_line(log, &format!("attachment {id}: no text stored: {why}"));
    let file = match file_of(store, id) { Ok(Some(f)) => f, _ => return say("the file is not listed any more") };
    let Some(path) = file.local_path.clone() else { return say("the file is not on this Mac") };
    let kind = AttachmentKind::detect(&file.mime_type, &file.file_name);
    let text = match text_of_file(Path::new(&path), kind) { Ok(t) => t, Err(why) => return say(&why) };
    let chunks: Vec<String> = match Ok::<_, String>(search_vector::pieces::pieces_of(&search_vector::pieces::ByteBound, None, &search_vector::pieces::clean_for_meaning(&text))) {
        Ok(c) => c.into_iter().filter(|t| !t.trim().is_empty()).collect(),
        Err(e) => return say(&format!("the text could not be cut: {e}")),
    };
    let entry = AttachmentIndexEntry {
        message_id: file.message_id.clone(), attachment_id: id.to_string(), name: file.file_name.clone(), kind, state: IndexState::Indexed, text: Some(text),
        ocr_text: None, labels: Vec::new(), transcript: None, vectors: Vec::new(), notes: Vec::new(), updated_at_ms: crate::rust_core::now_ms(),
    };
    if let Err(e) = store.write(move |ctx| ctx.attachment_index_save(&entry)) { return say(&format!("the text was not filed: {e}")); }
    match store.replace_attachment_chunks(id, chunks) {
        // the mail's indexer embeds the new pieces now, not at the mail's next re-index
        Ok(_) => crate::bodies::crawl::enqueue_for_store(store, &[file.message_id.clone()]),
        Err(e) => say(&format!("its chunks were not filed: {e}")),
    }
}
