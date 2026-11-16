//! The scheduler. Every account runs as its own task: first screen, then (once every account has its
//! first screen, or has failed, or is slowed) its backfill, with at most `backfill_accounts` accounts
//! backfilling at once. Dropping `run`'s future aborts every task it started.
use contracts::{Account, ActionFailed, Clock, EventSink};
use std::sync::Arc;
use store::Store;
use std::time::Duration;
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinSet;

use crate::account::{AccountCtx, Shared};
use crate::config::EngineConfig;
use crate::gate::Gate;
use crate::port::AccountSource;
use crate::sync_account::{sync_account, Outcome};

/// What `Engine::run_resuming` tells its caller between two runs of one account.
#[derive(Debug, Clone, PartialEq)]
pub enum Resume {
    /// The run stopped on this failure (recorded on the account's row); the account waits to be tried again.
    Failed(ActionFailed),
    /// The wait ended (the ledger's longest backoff, or the owner's action): the account runs again.
    Again,
}

pub struct Engine {
    store: Arc<Store>,
    sink: Arc<dyn EventSink>,
    clock: Arc<dyn Clock>,
    cfg: EngineConfig,
}

impl Engine {
    pub fn new(store: Arc<Store>, sink: Arc<dyn EventSink>, clock: Arc<dyn Clock>, cfg: EngineConfig) -> Engine {
        Engine { store, sink, clock, cfg }
    }

    /// Sync every account to its full history. Returns each account's outcome by account id.
    pub async fn run(&self, accounts: Vec<(Account, Arc<dyn AccountSource>)>) -> Vec<(String, Outcome)> {
        let mut out = Vec::new();
        let mut active = Vec::new();
        for (account, source) in accounts {
            if account.paused {
                out.push((account.id.clone(), Outcome::Paused));
            } else {
                active.push((account, source));
            }
        }
        let gate = Arc::new(Gate::new(active.len()));
        let shared = Shared { store: self.store.clone(), sink: self.sink.clone(), clock: self.clock.clone(), cfg: self.cfg, gate };
        let slots = Arc::new(Semaphore::new(self.cfg.backfill_accounts.max(1)));
        let mut tasks = JoinSet::new();
        for (index, (account, source)) in active.into_iter().enumerate() {
            let id = account.id.clone();
            let ctx = Arc::new(AccountCtx::new(account, source, shared.clone(), index));
            let slots = slots.clone();
            tasks.spawn(async move { (id, sync_account(ctx, slots).await) });
        }
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(done) => out.push(done),
                Err(e) => eprintln!("sync: an account task ended without an outcome: {e}"),
            }
        }
        out
    }

    /// Syncs one account and keeps it syncing. A run that fails is tried again: after `backoff_max_ms` (the ledger's longest
    /// wait, SYNC_BACKOFF_MAX_MS) when the failure is retryable, and at once when `retry` is notified (the owner's next action on
    /// the account). A failure that is not retryable (a refused login, a refused command) waits for that action only, so a wrong
    /// password is not sent every few minutes. Returns when a run completes or the account is paused; a caller that stops the
    /// account drops this future.
    pub async fn run_resuming(&self, account: Account, source: Arc<dyn AccountSource>, retry: &Notify, mut on: impl FnMut(Resume)) -> Outcome {
        loop {
            let out = self.run(vec![(account.clone(), source.clone())]).await.into_iter().next().map(|(_, o)| o).unwrap_or(Outcome::Paused);
            let Outcome::Failed(failure) = out else { return out };
            let timed = failure.retryable;
            let owner = retry.notified();
            tokio::pin!(owner);
            owner.as_mut().enable();
            on(Resume::Failed(failure));
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(self.cfg.backoff_max_ms)), if timed => {}
                _ = &mut owner => {}
            }
            on(Resume::Again);
        }
    }
}
