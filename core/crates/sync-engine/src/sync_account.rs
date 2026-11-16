//! Use case: sync one account end to end. Its failure is recorded on its own row and ends only its own task.
use contracts::ActionFailed;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::account::AccountCtx;
use crate::backfill::backfill_folder;
use crate::first_screen::first_screen;
use crate::gate::Arrival;
use crate::retry;
use crate::state::{Progress, StatePatch};
use crate::words;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Complete,
    Failed(ActionFailed),
    Paused,
}

pub async fn sync_account(ctx: Arc<AccountCtx>, backfill_slots: Arc<Semaphore>) -> Outcome {
    let _arrival = Arrival { gate: &ctx.gate, index: ctx.index };
    match run(&ctx, &backfill_slots).await {
        Ok(()) => Outcome::Complete,
        Err(failure) => {
            eprintln!("sync {}: stopped: {}: {}", ctx.account.id, failure.account_name, failure.reason_words);
            // A retryable failure (its attempts spent) is tried again after the ledger's longest wait, and says so; any
            // other waits for the owner's next action on the account and says nothing more than its failure.
            let status = failure.retryable.then(|| {
                let retry_s = ctx.cfg.backoff_max_ms.div_ceil(1000);
                words::paused_status(&format!("Paused: {}. Trying again in {retry_s} s.", failure.reason_words), retry_s, &failure.reason_words)
            });
            if let Err(e) = ctx.record(StatePatch { last_error: Some(Some(failure.clone())), provider_status_words: Some(status), ..Default::default() }).await {
                eprintln!("sync {}: could not record the failure: {}", ctx.account.id, e.reason_words);
            }
            Outcome::Failed(failure)
        }
    }
}

async fn run(ctx: &Arc<AccountCtx>, backfill_slots: &Semaphore) -> Result<(), ActionFailed> {
    // a run that starts again after a failure clears it: the account is working, and says it is connecting
    ctx.record(StatePatch { last_error: Some(None), progress: Some(Progress::Connecting), ..Default::default() }).await?;
    let folders = retry::call(ctx, || ctx.source.list_folders()).await?;
    let stored = ctx.load_folders().await?;
    let first = first_screen(ctx, &folders, &stored).await;
    ctx.gate.arrive(ctx.index);
    first?;
    ctx.gate.wait_all().await;

    let _slot = backfill_slots.acquire().await.map_err(|e| ctx.failure(format!("the backfill queue stopped: {e}"), false))?;
    let stored = ctx.load_folders().await?;
    let mut folders_set = JoinSet::new();
    for folder in folders {
        let (ctx, start) = (ctx.clone(), stored.get(&folder).copied().unwrap_or_default());
        folders_set.spawn(async move { backfill_folder(&ctx, &folder, start).await });
    }
    while let Some(joined) = folders_set.join_next().await {
        joined.map_err(|e| ctx.failure(format!("a folder sync stopped: {e}"), false))??;
    }
    let now = ctx.clock.now_ms();
    ctx.record(StatePatch { history_fetched: Some(true), last_error: Some(None), last_synced_ms: Some(now), progress: Some(Progress::Messages), ..Default::default() }).await
}
