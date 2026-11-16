//! One folder of this Mac brought up to date in the index: walk it, read a file only when its size or modified time changed, remove
//! the files that are gone (or became too big, or hidden), and report progress. Runs in the background; every port call is the disk or
//! the local index, never the network.
use crate::constants::{REPLY_EXCERPT_MAX_CHARS, REPLY_EXCERPT_MIN_CHARS};
use crate::excerpt::{excerpt_id, ItemStamp, SourceExcerpt};
use crate::ports::{ExcerptSink, FileTree, ProgressSink};
use crate::progress::ProgressTracker;
use crate::split::split_excerpts;
use crate::text::read_file_text;
use crate::usecases::SyncOutcome;
use crate::walk::{walk_folder, FoundFile, WalkSkips};
use attachment_index::{Limits, TextReader};
use contracts::{ReplySourceRef, ReplySyncProgress};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct FolderPorts<'a> {
    pub tree: &'a dyn FileTree,
    pub reader: &'a dyn TextReader,
    pub sink: &'a dyn ExcerptSink,
    pub progress: &'a dyn ProgressSink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderSyncReport {
    pub outcome: SyncOutcome,
    /// the files that hold text, now in the index (the tile's "214 fichiers")
    pub item_count: u64,
    /// files read this time (new or changed)
    pub read: u32,
    /// files left alone because their size and modified time did not change
    pub unchanged: u32,
    /// files removed from the index (gone from the folder, or now skipped)
    pub removed: u32,
    /// files of a text kind whose reader failed: kept with no excerpt, read again when they change
    pub unreadable: u32,
    pub skips: WalkSkips,
    pub last: ReplySyncProgress,
}

fn version(f: &FoundFile) -> String {
    format!("{}:{}", f.size, f.modified_ms)
}

fn excerpts_of(source: &ReplySourceRef, root: &str, f: &FoundFile, title: &str, text: &str) -> Vec<SourceExcerpt> {
    let folder = format!("{}/{}", root.trim_end_matches('/'), f.rel);
    split_excerpts(text, REPLY_EXCERPT_MIN_CHARS, REPLY_EXCERPT_MAX_CHARS)
        .into_iter()
        .enumerate()
        .map(|(i, t)| SourceExcerpt {
            id: excerpt_id(source, &f.rel, i as u32),
            source: source.clone(),
            item_key: f.rel.clone(),
            ordinal: i as u32,
            title: title.to_string(),
            folder: folder.clone(),
            date_ms: Some(f.modified_ms),
            text: t,
        })
        .collect()
}

/// Syncs the folder `path` (as the Sources pane stores it: "~/Documents/Contrats" or "/Volumes/..."). `keep_going` is asked before each
/// file; false stops the sync where it is.
pub fn sync_local_folder(ports: &FolderPorts, path: &str, keep_going: &dyn Fn() -> bool) -> FolderSyncReport {
    let source = ReplySourceRef::LocalFolder { path: path.to_string() };
    let mut tracker = ProgressTracker::new(source.clone(), ports.progress);
    let mut r = FolderSyncReport { outcome: SyncOutcome::UpToDate, item_count: 0, read: 0, unchanged: 0, removed: 0, unreadable: 0, skips: WalkSkips::default(), last: ReplySyncProgress::NotStarted };
    let fail = |tracker: &mut ProgressTracker, r: &mut FolderSyncReport, words: String| {
        tracker.failed(r.item_count, words.clone());
        r.outcome = SyncOutcome::Failed { reason_words: words };
        r.last = tracker.state().clone();
    };
    let known: HashMap<String, ItemStamp> = match ports.sink.stamps(&source) {
        Ok(s) => s.into_iter().map(|s| (s.item_key.clone(), s)).collect(),
        Err(e) => {
            fail(&mut tracker, &mut r, format!("the index of {path} could not be read: {e}"));
            return r;
        }
    };
    let walk = match walk_folder(ports.tree, Path::new(path)) {
        Ok(w) => w,
        Err(e) => {
            fail(&mut tracker, &mut r, format!("the folder {path} could not be opened: {e}"));
            return r;
        }
    };
    r.skips = walk.skips.clone();
    let total = walk.files.len() as u64;
    tracker.advance(0, total, 0);
    let max_chars = Limits::from_constants().text_max_chars;
    for (i, f) in walk.files.iter().enumerate() {
        if !keep_going() {
            r.outcome = SyncOutcome::Stopped;
            r.last = tracker.state().clone();
            return r;
        }
        let v = version(f);
        match known.get(&f.rel) {
            Some(stamp) if stamp.version == v => {
                r.unchanged += 1;
                r.item_count += (stamp.excerpt_count > 0) as u64;
            }
            _ => {
                let excerpts = match read_file_text(ports.tree, ports.reader, f, max_chars) {
                    Ok(Some(t)) => excerpts_of(&source, path, f, &t.title, &t.text),
                    Ok(None) => Vec::new(),
                    Err(_) => {
                        r.unreadable += 1;
                        Vec::new()
                    }
                };
                let stamp = ItemStamp { item_key: f.rel.clone(), version: v, excerpt_count: excerpts.len() as u32 };
                if let Err(e) = ports.sink.put_item(&source, &stamp, &excerpts) {
                    fail(&mut tracker, &mut r, format!("{} could not be added to the index: {e}", f.rel));
                    return r;
                }
                r.read += 1;
                r.item_count += (!excerpts.is_empty()) as u64;
            }
        }
        tracker.advance(i as u64 + 1, total, r.item_count);
    }
    let found: HashSet<&str> = walk.files.iter().map(|f| f.rel.as_str()).collect();
    let mut gone: Vec<&String> = known.keys().filter(|k| !found.contains(k.as_str())).collect();
    gone.sort();
    for key in gone {
        if let Err(e) = ports.sink.remove_item(&source, key) {
            fail(&mut tracker, &mut r, format!("{key} could not be removed from the index: {e}"));
            return r;
        }
        r.removed += 1;
    }
    tracker.up_to_date(r.item_count);
    r.last = tracker.state().clone();
    r
}
