//! lane reply-headers (SPEC A144): each stored mail's Message-ID and References, so a reply is threaded from the store and never waits
//! on the mail server (outbox `StoreReplyContext`). Three ways in, none on a UI path:
//!   1. an arrival the sync stores through `junk_filter::store_arrivals` gets its headers in the insert's own transaction
//!      (`imap_source::arrival_headers`);
//!   2. a page the sync engine stores (`ImapSource::page`) cannot carry them (the engine's `Page` holds `store::NewMessage`, which keeps its
//!      shape), so the page's headers are held here (`hold`) and written once the engine has stored the page and announced it (`kick`);
//!   3. the mail stored before (migration 0023 left it unknown) or whose fetch read no Message-ID: after each folder sync, a background pass
//!      per account and folder reads up to a page of rows with no `rfc_message_id`, asks the server (`thread_headers_many`, one call) and
//!      writes what came back. A UID the server did not answer is marked '' so it is not asked on every pass.
//! The pacing is the body download's (bodies.rs): `folder_synced` names the folder (every folder the sync fetches, and at registration every
//! folder that already holds unknown rows: `adopt_stored`), `kick` (after each stored page, after each applied change set) runs the account's
//! folders once at a time, INBOX first, a page per call until the folder has no unknown row; a kick during a run runs it again after. The run is on the runtime's blocking
//! pool: the server call blocks its thread (`AccountAdapters` is the blocking form) and the store's write waits for the writer.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::CoreError;
use send::ParentHeaders;
use store::{bracketed, ReplyTo, Store, ThreadHeaders};

use super::{lock, words};

/// Rows asked per folder per pass, in one `thread_headers_many` call: one history page (docs/CONSTANTS.md SYNC_HISTORY_PAGE, the size the
/// servers already answer in one FETCH for the sync itself). No new number.
pub const FILL_PAGE: usize = contracts::constants::SYNC_HISTORY_PAGE as usize;

/// Arrival headers held between a page's fetch and its store, at most this many (two pages); past it the oldest are dropped and the pass asks
/// the server for them. A kick follows every stored page, so the list holds one page in practice. New number, for the lead's ledger.
pub const HELD_MAX: usize = contracts::constants::HELD_PAGES_MAX as usize * FILL_PAGE;

/// Where the headers of a folder's messages come from: the account's IMAP adapters (sync-imap's blocking `AccountAdapters`), or a fake a
/// test registers. A UID the server does not answer is absent from the result.
pub trait ThreadHeadersSource: Send + Sync {
    fn thread_headers_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError>;

    /// real-replyto (0027): the same answer with each mail's Reply-To (name, address) when the source read it in the same fetch. The default
    /// has no Reply-To to give (`None`: the stored column is left as it is), so a source that only knows `thread_headers_many` is unchanged.
    fn thread_headers_reply_to_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders, Option<Vec<(String, String)>>)>, CoreError> {
        Ok(self.thread_headers_many(account_id, folder, uids)?.into_iter().map(|(u, h)| (u, h, None)).collect())
    }
}

impl ThreadHeadersSource for sync_imap::AccountAdapters {
    fn thread_headers_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders)>, CoreError> {
        sync_imap::AccountAdapters::thread_headers_many(self, account_id, folder, uids)
    }
    fn thread_headers_reply_to_many(&self, account_id: &str, folder: &str, uids: &[u32]) -> Result<Vec<(u32, ParentHeaders, Option<Vec<(String, String)>>)>, CoreError> {
        Ok(sync_imap::AccountAdapters::thread_headers_reply_to_many(self, account_id, folder, uids)?.into_iter().map(|(u, h, r)| (u, h, Some(r))).collect())
    }
}

/// The server's answer for one UID as the store keeps it. An answer with an empty Message-ID counts as no answer (the '' mark).
fn from_server(h: ParentHeaders) -> Option<ThreadHeaders> {
    let message_id = bracketed(&h.message_id)?;
    Some(ThreadHeaders { message_id, references: h.references.iter().filter_map(|r| bracketed(r)).collect() })
}

/// What one pass over one folder did: rows asked, rows given headers, rows marked as not answered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Filled {
    pub asked: usize,
    pub found: usize,
    pub marked: usize,
}

/// One account's passes: the folders the sync named (INBOX first), and whether a run is going.
#[derive(Default)]
struct Runs {
    folders: Vec<String>,
    running: bool,
    again: bool,
}

pub struct ThreadHeadersFill {
    store: Arc<Store>,
    runtime: tokio::runtime::Handle,
    log: PathBuf,
    sources: Mutex<HashMap<String, Arc<dyn ThreadHeadersSource>>>,
    runs: Mutex<HashMap<String, Runs>>,
    held: Mutex<Vec<(String, ThreadHeaders)>>,
    held_reply_to: Mutex<Vec<(String, Vec<ReplyTo>)>>,
}

impl ThreadHeadersFill {
    pub fn new(store: Arc<Store>, runtime: tokio::runtime::Handle) -> ThreadHeadersFill {
        let log = crate::log::path_beside(store.path());
        ThreadHeadersFill { store, runtime, log, sources: Mutex::new(HashMap::new()), runs: Mutex::new(HashMap::new()), held: Mutex::new(Vec::new()), held_reply_to: Mutex::new(Vec::new()) }
    }

    /// The account's source from now on (a test registers a fake).
    pub fn set_source(&self, account_id: &str, source: Arc<dyn ThreadHeadersSource>) {
        lock(&self.sources).insert(account_id.to_string(), source);
    }

    /// The account's live IMAP adapter (the one its sync opened), through sync-imap's blocking `AccountAdapters`; then the folders that
    /// already hold unknown rows are adopted (`adopt_stored`).
    pub fn set_adapter(self: &Arc<Self>, account_id: &str, adapter: Arc<sync_imap::ImapAdapter>) {
        let adapters = sync_imap::AccountAdapters::new(self.runtime.clone()).with(account_id, adapter);
        self.set_source(account_id, Arc::new(adapters));
        self.adopt_stored(account_id);
    }

    /// Every folder of the account that holds rows with unknown headers joins the passes, and a run starts: the mail stored before this
    /// build is covered in every folder, not only in the folders a later sync names. One store read; returns at once.
    pub fn adopt_stored(self: &Arc<Self>, account_id: &str) {
        match self.store.thread_headers_unknown_folders(account_id) {
            Ok(folders) => for f in folders { self.folder_synced(account_id, &f); },
            Err(e) => crate::log::log_line(&self.log, &format!("thread headers {account_id}: the folders to fill could not be read: {e}")),
        }
        self.kick(account_id);
    }

    /// A page's headers, held until the engine has stored the page (the next `kick` writes them).
    pub fn hold(&self, rows: impl IntoIterator<Item = (String, ThreadHeaders)>) {
        let mut held = lock(&self.held);
        held.extend(rows);
        if held.len() > HELD_MAX {
            let extra = held.len() - HELD_MAX;
            held.drain(..extra);
        }
    }

    /// real-replyto: a page's Reply-To entries, held like its headers (the next `kick` writes them once the engine has stored the page).
    pub fn hold_reply_to(&self, rows: impl IntoIterator<Item = (String, Vec<ReplyTo>)>) {
        let mut held = lock(&self.held_reply_to);
        held.extend(rows);
        if held.len() > HELD_MAX {
            let extra = held.len() - HELD_MAX;
            held.drain(..extra);
        }
    }

    /// The sync stored a page of `folder` (or applied its changes): its unknown rows join the account's passes (INBOX first).
    pub fn folder_synced(&self, account: &str, folder: &str) {
        let mut all = lock(&self.runs);
        let r = all.entry(account.to_string()).or_default();
        if !r.folders.iter().any(|f| f == folder) {
            if folder.eq_ignore_ascii_case("INBOX") { r.folders.insert(0, folder.to_string()); } else { r.folders.push(folder.to_string()); }
        }
    }

    /// Runs the account's passes on the blocking pool, once at a time (a kick during a run runs it again after). Returns at once.
    pub fn kick(self: &Arc<Self>, account: &str) {
        {
            let mut all = lock(&self.runs);
            let r = all.entry(account.to_string()).or_default();
            if r.folders.is_empty() && lock(&self.held).is_empty() && lock(&self.held_reply_to).is_empty() { return; }
            if r.running { r.again = true; return; }
            r.running = true;
        }
        let (me, account) = (self.clone(), account.to_string());
        self.runtime.spawn_blocking(move || me.run(&account));
    }

    fn run(&self, account: &str) {
        loop {
            self.write_held();
            let folders = lock(&self.runs).get(account).map(|r| r.folders.clone()).unwrap_or_default();
            for folder in folders {
                // page after page until the folder has no unknown row left: every asked row is written (headers or the '' mark), so each
                // pass reads new rows and the loop ends; a refused call stops the folder until the next kick
                loop {
                    match self.fill_folder(account, &folder) {
                        Ok(f) if f.asked > 0 => continue,
                        Ok(_) => break,
                        Err(why) => {
                            let line = format!("thread headers {account} {folder}: not filled this time: {why}");
                            eprintln!("{line}");
                            crate::log::log_line(&self.log, &line);
                            break;
                        }
                    }
                }
            }
            let mut all = lock(&self.runs);
            let r = all.entry(account.to_string()).or_default();
            if r.again { r.again = false; continue; }
            r.running = false;
            return;
        }
    }

    /// The held arrival headers, written for the rows the engine has stored (a row not stored is left out by the store's own UPDATE; the
    /// pass asks the server for it once it is stored). Answers how many rows changed.
    pub fn write_held(&self) -> usize {
        let reply_to = std::mem::take(&mut *lock(&self.held_reply_to));
        if !reply_to.is_empty() {
            if let Err(e) = self.store.set_reply_to_quiet(reply_to) { crate::log::log_line(&self.log, &format!("thread headers: the held Reply-To were not kept: {e}")); }
        }
        let rows: Vec<(String, Option<ThreadHeaders>)> = std::mem::take(&mut *lock(&self.held)).into_iter().map(|(id, h)| (id, Some(h))).collect();
        if rows.is_empty() { return 0; }
        match self.store.set_thread_headers_quiet(rows) {
            Ok(n) => n,
            Err(e) => { crate::log::log_line(&self.log, &format!("thread headers: the held arrival headers were not kept: {e}")); 0 }
        }
    }

    /// One pass over one folder: up to `FILL_PAGE` rows with no headers, newest first, asked in one call; each answered row gets its headers,
    /// each row the server did not answer is marked. A failed call changes nothing (the next pass asks again). No source: nothing to do.
    pub fn fill_folder(&self, account: &str, folder: &str) -> Result<Filled, String> {
        let Some(source) = lock(&self.sources).get(account).cloned() else { return Ok(Filled::default()) };
        let unknown = self.store.thread_headers_unknown(account, folder, FILL_PAGE).map_err(|e| e.to_string())?;
        if unknown.is_empty() { return Ok(Filled::default()); }
        let uids: Vec<u32> = unknown.iter().map(|(u, _)| *u).collect();
        // real-replyto: the same fetch also reads each mail's Reply-To (no extra call); a source that cannot give it leaves the column as it is
        let mut by_uid: HashMap<u32, ParentHeaders> = HashMap::new();
        let mut reply_to: Vec<(String, Vec<ReplyTo>)> = Vec::new();
        let ids: HashMap<u32, String> = unknown.iter().map(|(u, id)| (*u, id.clone())).collect();
        for (uid, h, r) in source.thread_headers_reply_to_many(account, folder, &uids).map_err(|e| words(&e))? {
            if let (Some(items), Some(id)) = (r, ids.get(&uid)) {
                if from_server(h.clone()).is_some() { reply_to.push((id.clone(), items.into_iter().map(|(name, address)| ReplyTo { name, address }).collect())); }
            }
            by_uid.insert(uid, h);
        }
        let rows: Vec<(String, Option<ThreadHeaders>)> = unknown.into_iter().map(|(uid, id)| (id, by_uid.remove(&uid).and_then(from_server))).collect();
        let found = rows.iter().filter(|(_, h)| h.is_some()).count();
        let filled = Filled { asked: uids.len(), found, marked: rows.len() - found };
        self.store.set_thread_headers_quiet(rows).map_err(|e| e.to_string())?;
        self.store.set_reply_to_quiet(reply_to).map_err(|e| e.to_string())?;
        Ok(filled)
    }

    /// True when no pass is running. Held arrival headers do not count: they wait for the engine's next stored page, which may never come
    /// (a page the engine could not store), and `Bodies::settle` must not wait for them.
    pub fn idle(&self) -> bool {
        lock(&self.runs).values().all(|r| !r.running)
    }

    /// Waits until `idle` (the tests read after it).
    pub fn settle(&self, within: Duration) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < within {
            if self.idle() { return true; }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }
}
