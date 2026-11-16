//! One account as the engine runs it: its source, its worker cap, its slowed flag, and the two ways it
//! writes (a page, a state change). Each write goes to the store on a blocking thread and is announced
//! as `SyncStateChanged` with the row as stored.
use contracts::{Account, ActionFailed, Clock, CoreEvent, EventSink, SyncState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;
use store::{Store, StoreError};
use tokio::sync::Semaphore;

use crate::config::EngineConfig;
use crate::gate::Gate;
use crate::limits::worker_cap;
use crate::words;
use crate::port::{AccountSource, Page};
use crate::state::{self, FolderRow, StatePatch};

pub struct AccountCtx {
    pub account: Account,
    pub source: Arc<dyn AccountSource>,
    pub store: Arc<Store>,
    pub sink: Arc<dyn EventSink>,
    pub clock: Arc<dyn Clock>,
    pub cfg: EngineConfig,
    /// Calls in flight for this account, at most `worker_cap(provider)`.
    pub workers: Semaphore,
    pub slowed: AtomicBool,
    pub gate: Arc<Gate>,
    pub index: usize,
}

/// What every account of one run shares.
#[derive(Clone)]
pub struct Shared {
    pub store: Arc<Store>,
    pub sink: Arc<dyn EventSink>,
    pub clock: Arc<dyn Clock>,
    pub cfg: EngineConfig,
    pub gate: Arc<Gate>,
}

impl AccountCtx {
    pub fn new(account: Account, source: Arc<dyn AccountSource>, shared: Shared, index: usize) -> Self {
        let cap = worker_cap(account.provider, source.transport());
        let Shared { store, sink, clock, cfg, gate } = shared;
        AccountCtx { account, source, store, sink, clock, cfg, workers: Semaphore::new(cap), slowed: AtomicBool::new(false), gate, index }
    }

    /// A failure as the owner reads it: the account's name and the server's words, coded `sync.failed` at the step "sync"
    /// (a failure with a step of its own comes through `failure_coded`).
    pub fn failure(&self, reason_words: String, retryable: bool) -> ActionFailed {
        let code = words::failed_code("sync", "", &reason_words);
        self.failure_coded(reason_words, retryable, Some(code))
    }

    /// A failure with the code its source gave it; a source with none gets the sync's own.
    pub fn failure_coded(&self, reason_words: String, retryable: bool, code: Option<String>) -> ActionFailed {
        let account_name = if self.account.display_name.is_empty() { self.account.address.clone() } else { self.account.display_name.clone() };
        let code = code.unwrap_or_else(|| words::failed_code("sync", "", &reason_words));
        ActionFailed { account_name, reason_words, retryable, code: Some(code) }
    }

    fn store_failure(&self, e: StoreError) -> ActionFailed {
        eprintln!("sync {}: store write failed: {e}", self.account.id);
        let text = format!("could not save mail on this Mac: {e}");
        self.failure_coded(text, false, Some(words::failed_code("save", "", &e.to_string())))
    }

    async fn blocking<R: Send + 'static>(&self, f: impl FnOnce(&Store) -> Result<R, StoreError> + Send + 'static) -> Result<R, ActionFailed> {
        let store = self.store.clone();
        // spawn_blocking runs to the end even when this task is aborted: a page's two writes are never split by a stop.
        match tokio::task::spawn_blocking(move || f(&store)).await {
            Ok(r) => r.map_err(|e| self.store_failure(e)),
            Err(e) => Err(self.failure(format!("a store write stopped: {e}"), false)),
        }
    }

    fn announce(&self, state: SyncState) {
        self.sink.on_event(CoreEvent::SyncStateChanged { state });
    }

    pub async fn load_folders(&self) -> Result<HashMap<String, FolderRow>, ActionFailed> {
        let id = self.account.id.clone();
        self.blocking(move |s| state::load_folders(s, &id)).await
    }

    pub async fn persist(&self, folder: &str, page: Page) -> Result<(), ActionFailed> {
        let (id, f, now, progress) = (self.account.id.clone(), folder.to_string(), self.clock.now_ms(), !self.slowed.load(SeqCst));
        let st = self.blocking(move |s| state::persist_page(s, &id, &f, page, now, progress)).await?;
        self.announce(st);
        Ok(())
    }

    pub async fn record(&self, patch: StatePatch) -> Result<(), ActionFailed> {
        let id = self.account.id.clone();
        let st = self.blocking(move |s| state::write_state(s, &id, patch)).await?;
        self.announce(st);
        Ok(())
    }
}
