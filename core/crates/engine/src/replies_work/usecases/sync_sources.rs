//! The folders of this Mac (J10.2), on the background thread, run at start and after every change of the Sources pane (the store's change feed on
//! `reply_source`): a folder switched on and not walked yet in this launch is walked by reply-sources (`sync_local_folder`: progress events, the
//! excerpts into the in-memory index); a folder removed from the pane leaves the index. The walk stops between two files when its folder is switched
//! off or removed, and waits there while the background is paused.
use std::cell::Cell;
use std::path::Path;
use std::sync::atomic::Ordering;

use attachment_index::AttachmentKind;
use contracts::{CoreError, ReplySource, ReplySourceRef};
use reply_sources::adapters::DiskTree;
use reply_sources::{sync_calendar, sync_local_folder, FolderPorts, ProgressSink, SyncOutcome, TextReader};

use super::words;
use crate::replies_work::adapters::calendars::{utc_offset_s, StoreCalendars};
use crate::replies_work::adapters::progress::Progress;
use crate::replies_work::Shared;

/// The documents only the app can read (word-processor, sheet, slide files): no reader reaches the core yet, so they hold no text here. Plain text,
/// markdown, HTML and PDF are read by reply-sources itself.
struct NoAppReader;
impl TextReader for NoAppReader {
    fn read_text(&self, _path: &Path, _kind: AttachmentKind) -> Result<Option<String>, CoreError> { Ok(None) }
}

fn enabled_folder(s: &Shared, source: &ReplySourceRef) -> bool {
    s.store.reply_sources().map(|all| all.iter().any(|r| &r.source == source && r.enabled)).unwrap_or(false)
}

/// The mail folders on the Sources pane: each folder the crawl knows gets its row (off until the owner ticks it), and its count and progress are the
/// contract's reading of the crawl's figures (`folder_progress_source`: up to date once every message is indexed; off = not started). The figures are the
/// crawl's `Bodies::progress`, read on ITS background connection (the store's `reply_folder_progress` counts on the shared reader the views wait on, a
/// whole-table GROUP BY after every arrival). Written only when they changed, so the change feed settles.
fn mail_folders(s: &Shared, rows: &[ReplySource], progress: &Progress<'_>) {
    let Some(bodies) = s.bodies.upgrade() else { return };
    let folders = match bodies.progress() { Ok(f) => f, Err(e) => return s.log(&format!("the mail folders' progress could not be read: {e}")) };
    // sync-full-r1 round 2: the crawl answers the contract's FolderProgress itself (messages, bodies, indexed = every chunk embedded)
    for figures in folders {
        let source = ReplySourceRef::MailFolder { account_id: figures.account_id.clone(), folder: figures.folder.clone() };
        let held = rows.iter().find(|r| r.source == source);
        if held.is_none() {
            s.wait_unpaused();
            if let Err(e) = s.store.reply_source_ensure(source.clone(), crate::rust_core::now_ms()) { s.log(&format!("a mail folder was not added to the sources: {}", words(&e))); continue; }
        }
        let now = bodies.folder_source(figures, held.is_some_and(|r| r.enabled));   // bg-hooks r2: says so when the meaning model is not on this Mac
        if held.map(|r| (r.item_count, &r.progress)) != Some((now.item_count, &now.progress)) { progress.progress(&source, now.item_count, &now.progress); }
    }
}

/// The calendars the shell pushed: each ENABLED calendar's events become excerpts (reply-sources' `sync_calendar`: one per event, in the owner's time,
/// progress events), a calendar switched off or gone leaves the index. Built again only when the pushed calendars or the switches changed.
fn calendars(s: &Shared, rows: &[ReplySource], progress: &Progress<'_>) {
    let on: Vec<String> = rows.iter().filter(|r| r.enabled).filter_map(|r| match &r.source { ReplySourceRef::Calendar { calendar_id } => Some(calendar_id.clone()), _ => None }).collect();
    let key = (s.calendars_epoch.load(Ordering::SeqCst), on.clone());
    if s.calendars_built.lock().unwrap_or_else(|e| e.into_inner()).as_ref() == Some(&key) { return; }
    for held in s.memory.sources().into_iter().filter(|src| matches!(src, ReplySourceRef::Calendar { calendar_id } if !on.contains(calendar_id))) { s.memory.remove_source(&held); }
    let now = crate::rust_core::now_ms();
    let store = StoreCalendars { store: &s.store };
    for id in &on {
        let report = sync_calendar(&store, &s.memory, progress, id, now, utc_offset_s(now), &|| { s.wait_unpaused(); true });
        if let SyncOutcome::Failed { reason_words } = report.outcome { s.log(&format!("the calendar {id} was not read: {reason_words}")); }
    }
    *s.calendars_built.lock().unwrap_or_else(|e| e.into_inner()) = Some(key);
}

pub(crate) fn run(s: &Shared) {
    let rows = match s.store.reply_sources() { Ok(r) => r, Err(e) => return s.log(&format!("the sources could not be read: {}", words(&e))) };
    mail_folders(s, &rows, &Progress { store: &s.store, sinks: &s.sinks, gate: &|| s.wait_unpaused() });
    calendars(s, &rows, &Progress { store: &s.store, sinks: &s.sinks, gate: &|| s.wait_unpaused() });
    // removed from the pane: its excerpts leave the index
    for gone in s.memory.sources().into_iter().filter(|src| !rows.iter().any(|r| &r.source == src)) {
        s.memory.remove_source(&gone);
        s.synced.lock().unwrap_or_else(|e| e.into_inner()).remove(&gone);
    }
    s.synced.lock().unwrap_or_else(|e| e.into_inner()).retain(|src| rows.iter().any(|r| &r.source == src));
    let todo: Vec<ReplySourceRef> = rows.iter()
        .filter(|r| r.enabled && matches!(r.source, ReplySourceRef::LocalFolder { .. }))
        .map(|r| r.source.clone())
        .filter(|src| !s.synced.lock().unwrap_or_else(|e| e.into_inner()).contains(src))
        .collect();
    let (tree, reader) = (DiskTree::from_env(), NoAppReader);
    let progress = Progress { store: &s.store, sinks: &s.sinks, gate: &|| s.wait_unpaused() };
    for source in todo {
        let ReplySourceRef::LocalFolder { path } = &source else { continue };
        s.synced.lock().unwrap_or_else(|e| e.into_inner()).insert(source.clone());
        let ports = FolderPorts { tree: &tree, reader: &reader, sink: &s.memory, progress: &progress };
        // the switch is read again only when the pane changed (every write to reply_source bumps the epoch)
        let (seen, still) = (Cell::new(s.sources_epoch.load(Ordering::SeqCst)), Cell::new(true));
        let keep_going = || {
            s.wait_unpaused();
            let now = s.sources_epoch.load(Ordering::SeqCst);
            if now != seen.get() { seen.set(now); still.set(enabled_folder(s, &source)); }
            still.get()
        };
        let report = sync_local_folder(&ports, path, &keep_going);
        match report.outcome {
            // switched off or removed mid-walk: walked again when switched on (unchanged files are not read twice)
            SyncOutcome::Stopped => { s.synced.lock().unwrap_or_else(|e| e.into_inner()).remove(&source); }
            SyncOutcome::Failed { reason_words } => s.log(&format!("the folder {path} was not read: {reason_words}")),
            SyncOutcome::UpToDate => {}
        }
        let still_listed = s.store.reply_sources().map(|all| all.iter().any(|r| r.source == source)).unwrap_or(true);
        if !still_listed { s.memory.remove_source(&source); }
    }
}
