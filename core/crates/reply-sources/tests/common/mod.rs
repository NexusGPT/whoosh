//! Fake ports for the tests: a disk in memory (links resolve, as a real disk's would, so a walk that followed one would be seen), a
//! document reader, an index and a progress log. Each records what it was asked.
#![allow(dead_code)]
use attachment_index::AttachmentKind;
use contracts::{reply_source_key, CoreError, ReplySourceRef, ReplySyncProgress};
use reply_sources::{CalendarEvent, CalendarInfo, CalendarStore, ExcerptSink, FileTree, ItemStamp, ProgressSink, SourceExcerpt, TextReader, TreeEntry, TreeEntryKind};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Clone)]
pub enum Node {
    File { bytes: Vec<u8>, size: u64, modified_ms: i64 },
    Dir,
    Link { target: String },
}

#[derive(Default)]
pub struct FakeTree {
    pub nodes: Mutex<BTreeMap<String, Node>>,
    pub reads: Mutex<Vec<String>>,
}

impl FakeTree {
    pub fn dir(&self, path: &str) {
        self.nodes.lock().unwrap().insert(path.to_string(), Node::Dir);
    }
    pub fn file(&self, path: &str, text: &str, modified_ms: i64) {
        self.nodes.lock().unwrap().insert(path.to_string(), Node::File { bytes: text.as_bytes().to_vec(), size: text.len() as u64, modified_ms });
    }
    /// a file whose reported size is `size` whatever its bytes (a big file without the memory)
    pub fn sized(&self, path: &str, text: &str, size: u64, modified_ms: i64) {
        self.nodes.lock().unwrap().insert(path.to_string(), Node::File { bytes: text.as_bytes().to_vec(), size, modified_ms });
    }
    pub fn link(&self, path: &str, target: &str) {
        self.nodes.lock().unwrap().insert(path.to_string(), Node::Link { target: target.to_string() });
    }
    pub fn remove(&self, path: &str) {
        self.nodes.lock().unwrap().remove(path);
    }
    pub fn take_reads(&self) -> Vec<String> {
        std::mem::take(&mut *self.reads.lock().unwrap())
    }
    /// a path with every link along it replaced by its target
    fn real(&self, path: &str) -> String {
        let nodes = self.nodes.lock().unwrap();
        let mut out = String::new();
        for part in path.trim_start_matches('/').split('/') {
            out = format!("{out}/{part}");
            if let Some(Node::Link { target }) = nodes.get(&out) {
                out = target.clone();
            }
        }
        out
    }
}

impl FileTree for FakeTree {
    fn read_dir(&self, dir: &Path) -> Result<Vec<TreeEntry>, CoreError> {
        let dir = self.real(&dir.to_string_lossy());
        let nodes = self.nodes.lock().unwrap();
        if !matches!(nodes.get(&dir), Some(Node::Dir)) {
            // the words a disk gives, which do not name the folder: the sync's own words must
            return Err(CoreError::Unexpected { reason_words: "No such file or directory".into() });
        }
        let prefix = format!("{dir}/");
        Ok(nodes
            .iter()
            .filter_map(|(p, n)| {
                let name = p.strip_prefix(&prefix)?;
                if name.contains('/') {
                    return None;
                }
                let kind = match n {
                    Node::File { size, modified_ms, .. } => TreeEntryKind::File { size: *size, modified_ms: *modified_ms },
                    Node::Dir => TreeEntryKind::Dir,
                    Node::Link { .. } => TreeEntryKind::Link,
                };
                Some(TreeEntry { name: name.to_string(), kind })
            })
            .collect())
    }

    fn read(&self, file: &Path, max_bytes: u64) -> Result<Vec<u8>, CoreError> {
        let path = self.real(&file.to_string_lossy());
        self.reads.lock().unwrap().push(path.clone());
        match self.nodes.lock().unwrap().get(&path) {
            Some(Node::File { bytes, .. }) => Ok(bytes.iter().take(max_bytes as usize).copied().collect()),
            _ => Err(CoreError::NotFound { kind: "file".into(), id: path }),
        }
    }
}

/// The app's document readers: a text per path, or an error.
#[derive(Default)]
pub struct FakeReader {
    pub texts: Mutex<HashMap<String, Result<Option<String>, String>>>,
    pub calls: Mutex<Vec<(String, AttachmentKind)>>,
}

impl FakeReader {
    pub fn answer(&self, path: &str, text: &str) {
        self.texts.lock().unwrap().insert(path.to_string(), Ok(Some(text.to_string())));
    }
    pub fn refuse(&self, path: &str, words: &str) {
        self.texts.lock().unwrap().insert(path.to_string(), Err(words.to_string()));
    }
}

impl TextReader for FakeReader {
    fn read_text(&self, path: &Path, kind: AttachmentKind) -> Result<Option<String>, CoreError> {
        let p = path.to_string_lossy().into_owned();
        self.calls.lock().unwrap().push((p.clone(), kind));
        match self.texts.lock().unwrap().get(&p) {
            Some(Ok(t)) => Ok(t.clone()),
            Some(Err(w)) => Err(CoreError::Unexpected { reason_words: w.clone() }),
            None => Ok(None),
        }
    }
}

/// The index: per source key, per item, its stamp and excerpts.
#[derive(Default)]
pub struct FakeSink {
    pub items: Mutex<BTreeMap<String, BTreeMap<String, (ItemStamp, Vec<SourceExcerpt>)>>>,
    pub puts: Mutex<Vec<String>>,
    pub removes: Mutex<Vec<String>>,
    pub fail_stamps: Mutex<bool>,
}

impl FakeSink {
    pub fn excerpts(&self, source: &ReplySourceRef) -> Vec<SourceExcerpt> {
        let items = self.items.lock().unwrap();
        items.get(&reply_source_key(source.clone())).map(|m| m.values().flat_map(|(_, e)| e.clone()).collect()).unwrap_or_default()
    }
    pub fn item_keys(&self, source: &ReplySourceRef) -> Vec<String> {
        let items = self.items.lock().unwrap();
        items.get(&reply_source_key(source.clone())).map(|m| m.keys().cloned().collect()).unwrap_or_default()
    }
    pub fn take_puts(&self) -> Vec<String> {
        std::mem::take(&mut *self.puts.lock().unwrap())
    }
}

impl ExcerptSink for FakeSink {
    fn stamps(&self, source: &ReplySourceRef) -> Result<Vec<ItemStamp>, CoreError> {
        if *self.fail_stamps.lock().unwrap() {
            return Err(CoreError::Unexpected { reason_words: "disk full".into() });
        }
        let items = self.items.lock().unwrap();
        Ok(items.get(&reply_source_key(source.clone())).map(|m| m.values().map(|(s, _)| s.clone()).collect()).unwrap_or_default())
    }
    fn put_item(&self, source: &ReplySourceRef, stamp: &ItemStamp, excerpts: &[SourceExcerpt]) -> Result<(), CoreError> {
        self.puts.lock().unwrap().push(stamp.item_key.clone());
        self.items.lock().unwrap().entry(reply_source_key(source.clone())).or_default().insert(stamp.item_key.clone(), (stamp.clone(), excerpts.to_vec()));
        Ok(())
    }
    fn remove_item(&self, source: &ReplySourceRef, item_key: &str) -> Result<(), CoreError> {
        self.removes.lock().unwrap().push(item_key.to_string());
        if let Some(m) = self.items.lock().unwrap().get_mut(&reply_source_key(source.clone())) {
            m.remove(item_key);
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeProgress {
    pub log: Mutex<Vec<(u64, ReplySyncProgress)>>,
}

impl FakeProgress {
    pub fn states(&self) -> Vec<ReplySyncProgress> {
        self.log.lock().unwrap().iter().map(|(_, p)| p.clone()).collect()
    }
}

impl ProgressSink for FakeProgress {
    fn progress(&self, _source: &ReplySourceRef, item_count: u64, progress: &ReplySyncProgress) {
        self.log.lock().unwrap().push((item_count, progress.clone()));
    }
}

#[derive(Default)]
pub struct FakeCalendars {
    pub calendars: Vec<CalendarInfo>,
    pub events: Mutex<Vec<CalendarEvent>>,
    pub asked: Mutex<Vec<(String, i64, i64)>>,
    pub refuse: bool,
}

impl CalendarStore for FakeCalendars {
    fn calendars(&self) -> Result<Vec<CalendarInfo>, CoreError> {
        Ok(self.calendars.clone())
    }
    fn events(&self, calendar_id: &str, from_ms: i64, to_ms: i64) -> Result<Vec<CalendarEvent>, CoreError> {
        if self.refuse {
            return Err(CoreError::Unexpected { reason_words: "access to calendars is not allowed".into() });
        }
        self.asked.lock().unwrap().push((calendar_id.to_string(), from_ms, to_ms));
        // the store hands back every event of the calendar, in the window or not: the sync must keep the window itself
        Ok(self.events.lock().unwrap().iter().filter(|e| e.calendar_id == calendar_id).cloned().collect())
    }
}

pub fn path(p: &str) -> PathBuf {
    PathBuf::from(p)
}
