//! The Whoosh store: SQLite behind one writer thread.
//!
//! - Single writer: every write is a job on one thread that owns the only write connection (`writer.rs`).
//! - WAL: readers (`Store::inbox`, `counts`, `thread`, ...) use their own read-only connection and never wait for a write.
//! - Change feed: one `TableChange` per touched table per commit, and the contract's Inbox, Counts and Thread
//!   streams fed from the same commit, one event per stream (`changes.rs`, `writer.rs`).
//! - Schema: the migrations crate's runner, included by path. See docs/lanes/core-store.md "## Contract deltas"
//!   for the Cargo dependency line that replaces the include.
//!
//! Third-party code: rusqlite (MIT) with the bundled SQLite (public domain). Nothing is copied.

#[path = "../migrations/src/lib.rs"]
pub mod migrations;

// ai-settings-core: providers, agents and the speech engine over migration 0001 (docs/lanes/ai-settings-core.md)
pub mod ai_settings;
pub mod app_settings;
mod changes;
mod contacts;
// lane real-notes: private notes, MCP grants/server/approvals (docs/lanes/real-notes.md)
mod notes_mcp;
mod delta10;
mod delta12;
mod realcore;
mod delta15;
mod import_rows;
mod junk;
mod reminders;
pub mod counts;
mod error;
mod label_meta;
mod labels;
mod moves;
mod ops;
mod open;
mod page;
mod place;
mod pending;
pub mod read;
// delta 20: AI replies settings over migration 0013 (docs/lanes/replies-settings.md)
mod replies;
mod reply_to;
mod seed;
mod setup;
mod thread_headers;
// fix-aicore: the reader's thread summary over migration 0030 (thread_summary.rs)
pub mod thread_summary;
mod voice;
mod writer;

pub use changes::{streams_for_table, TableChange, TableSink};
pub use error::StoreError;
pub use pending::{NewPending, PendingRow};
pub use junk::{JunkLesson, JunkState, JunkText, StoredJunkModel};
pub use contacts::{ContactRecord, MergeResult};
pub use notes_mcp::{McpApprovalRow, NoteFull, Refusal};
pub use label_meta::{decided_by_columns, decided_by_from, decode_conditions, encode_conditions, SET_ASIDE_MODEL, WAITING_MODEL};
pub use labels::{all_labels, find_label, get_label, new_id, EnsuredLabel, LabelRecord, NewLabel};
pub use moves::AccountMove;
pub use place::ServerPlace;
pub use ops::NewMessage;
pub use page::FolderProgress;
pub use read::{SnoozedRow, INBOX_FOLDER, INBOX_PAGE_ROWS};
pub use realcore::{attachment_row_id, AttachmentIn, AttachmentSource, BodyParts, StoredView, SyncAccount, SyncStateRow};
pub use reminders::ReminderFacts;
pub use seed::{default_tab_names, tab_id};
pub use setup::{CachedDetection, MessageLocation};
pub use reply_to::ReplyTo;
pub use thread_headers::{bracketed, ThreadHeaders};
pub use voice::{voice_refresh_days, VoiceProfile};
pub use writer::WriteCtx;

use changes::Bus;
use contracts::{Counts, EventSink, InboxQuery, InboxRow, MailThread, StreamKind};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use writer::{Job, SharedTally, Touched, WriterState};

pub struct Store {
    path: PathBuf,
    jobs: Mutex<Option<Sender<Job>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
    reader: Mutex<Connection>,
    bus: Arc<Bus>,
    tally: SharedTally,
}

impl Store {
    /// Open (and migrate, and seed the default tabs of) the database at `path`, and start the writer.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let mut conn = open::open_writer(path)?;
        seed::seed_split_tabs(&conn)?;
        let touched: Touched = Arc::new(Mutex::new(Default::default()));
        let hook_touched = touched.clone();
        conn.update_hook(Some(move |action: rusqlite::hooks::Action, _db: &str, table: &str, _row: i64| {
            let mut map = hook_touched.lock().unwrap_or_else(|e| e.into_inner());
            let entry = map.entry(table.to_string()).or_insert_with(|| TableChange { table: table.to_string(), ..Default::default() });
            match action {
                rusqlite::hooks::Action::SQLITE_INSERT => entry.inserts += 1,
                rusqlite::hooks::Action::SQLITE_DELETE => entry.deletes += 1,
                _ => entry.updates += 1,
            }
        }))?;
        let reader = open::open_reader(path)?;
        let bus = Arc::new(Bus::default());
        let tally: SharedTally = Arc::new(Mutex::new(None));
        let (tx, rx) = mpsc::channel::<Job>();
        let mut state = WriterState { conn, touched, bus: bus.clone(), tally: tally.clone() };
        let handle = std::thread::Builder::new()
            .name("whoosh-store-writer".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    job(&mut state);
                }
            })
            .map_err(|e| StoreError::Invalid(format!("could not start the writer thread: {e}")))?;
        Ok(Store { path: path.to_path_buf(), jobs: Mutex::new(Some(tx)), handle: Mutex::new(Some(handle)), reader: Mutex::new(reader), bus, tally })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn submit<R: Send + 'static>(&self, f: impl FnOnce(&mut WriterState) -> R + Send + 'static) -> Result<R, StoreError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let job: Job = Box::new(move |state| {
            let _ = reply_tx.send(f(state));
        });
        {
            let guard = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            guard.as_ref().ok_or(StoreError::WriterGone)?.send(job).map_err(|_| StoreError::WriterGone)?;
        }
        reply_rx.recv().map_err(|_| StoreError::WriterGone)
    }

    /// Run `f` as one transaction on the writer thread and wait for the commit. An `Err` (or a panic) undoes
    /// everything `f` did and publishes nothing.
    pub fn write<R: Send + 'static>(&self, f: impl FnOnce(&mut WriteCtx<'_>) -> Result<R, StoreError> + Send + 'static) -> Result<R, StoreError> {
        self.submit(move |state| state.run(f))?
    }

    fn read<R>(&self, f: impl FnOnce(&Connection) -> Result<R, StoreError>) -> Result<R, StoreError> {
        let guard = self.reader.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// The first page of one tab (INBOX_PAGE_ROWS rows): pinned first, then newest first.
    pub fn inbox(&self, query: &InboxQuery) -> Result<Vec<InboxRow>, StoreError> {
        self.read(|c| read::inbox(c, query, INBOX_PAGE_ROWS))
    }

    /// Like `inbox` with a row limit of the caller's choosing.
    pub fn inbox_limit(&self, query: &InboxQuery, limit: usize) -> Result<Vec<InboxRow>, StoreError> {
        self.read(|c| read::inbox(c, query, limit))
    }

    /// The list row of each message id, in the order given, whatever folder holds it (SPEC A141, `read::rows_for_messages`).
    pub fn rows_for_messages(&self, ids: &[String]) -> Result<Vec<InboxRow>, StoreError> {
        self.read(|c| read::rows_for_messages(c, ids))
    }

    pub fn thread(&self, thread_id: &str) -> Result<Option<MailThread>, StoreError> {
        self.read(|c| read::thread(c, thread_id))
    }

    pub fn snoozed(&self) -> Result<Vec<SnoozedRow>, StoreError> {
        self.read(read::snoozed)
    }

    pub fn next_snooze_return_ms(&self) -> Result<Option<i64>, StoreError> {
        self.read(read::next_snooze_return_ms)
    }

    pub fn pinned(&self) -> Result<Vec<InboxRow>, StoreError> {
        self.read(read::pinned)
    }

    /// The counts the app shows. Served from the writer's tally; the first call after open (or after a write that
    /// could not say what it changed) recomputes the tally on the writer thread.
    pub fn counts(&self) -> Result<Counts, StoreError> {
        if self.tally.lock().unwrap_or_else(|e| e.into_inner()).is_none() {
            self.submit(|st| {
                let primary = read::primary_tab(&st.conn)?;
                st.counts_value(&primary).map(|_| ())
            })??;
        }
        let tally = self.tally.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default();
        self.read(|c| counts::build(c, &tally))
    }

    /// The counts computed from scratch, bypassing the tally. The tally must always equal this.
    pub fn recount(&self) -> Result<Counts, StoreError> {
        self.read(|c| {
            let primary = read::primary_tab(c)?;
            counts::build(c, &counts::recount(c, &primary)?)
        })
    }

    pub fn subscribe(&self, stream: StreamKind, sink: Arc<dyn EventSink>) -> u64 {
        self.bus.subscribe(stream, sink)
    }

    /// The raw feed: one call per touched table per commit.
    pub fn subscribe_tables(&self, sink: TableSink) -> u64 {
        self.bus.subscribe_tables(sink)
    }

    pub fn unsubscribe(&self, subscription: u64) {
        self.bus.unsubscribe(subscription)
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = self.handle.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = h.join();
        }
    }
}
