//! One provider call with the account's worker cap, provider-limit backoff and retries.
//! A throttle marks the account "slowed by <provider>" (and releases the first-screen gate, so a slowed
//! account never holds the others); the first success after it clears the words. A retryable failure with retries left
//! says `sync.paused` with the seconds to the next attempt, and clears the same way.
use contracts::ActionFailed;
use std::future::Future;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

use crate::account::AccountCtx;
use crate::limits::slowed_status;
use crate::port::SourceError;
use crate::state::StatePatch;
use crate::words;

pub async fn call<T, F, Fut>(ctx: &AccountCtx, mut op: F) -> Result<T, ActionFailed>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, SourceError>>,
{
    let mut attempt = 0u32;
    loop {
        let result = {
            let _permit = ctx.workers.acquire().await.map_err(|e| ctx.failure(format!("the account's workers stopped: {e}"), false))?;
            op().await
        };
        match result {
            Ok(v) => {
                if ctx.slowed.swap(false, SeqCst) {
                    ctx.record(StatePatch { provider_status_words: Some(None), ..Default::default() }).await?;
                }
                return Ok(v);
            }
            Err(SourceError::Throttled { retry_after_ms, reason_words }) => {
                eprintln!("sync {}: provider limit: {reason_words}", ctx.account.id);
                ctx.gate.arrive(ctx.index);
                if !ctx.slowed.swap(true, SeqCst) {
                    ctx.record(StatePatch { provider_status_words: Some(Some(slowed_status(&ctx.account))), ..Default::default() }).await?;
                }
                let wait = retry_after_ms.unwrap_or_else(|| ctx.cfg.backoff_ms(attempt)).min(ctx.cfg.backoff_max_ms);
                attempt = attempt.saturating_add(1);
                tokio::time::sleep(Duration::from_millis(wait)).await;
            }
            Err(SourceError::Failed { reason_words, retryable, code }) => {
                eprintln!("sync {}: {reason_words} (retryable: {retryable}, attempt {attempt})", ctx.account.id);
                if retryable && attempt < ctx.cfg.retry_max {
                    let wait = ctx.cfg.backoff_ms(attempt);
                    let retry_s = wait.div_ceil(1000);
                    ctx.slowed.store(true, SeqCst); // cleared by the first success, like a provider limit
                    let status = words::paused_status(&format!("Paused: {reason_words}. Trying again in {retry_s} s."), retry_s, &reason_words);
                    ctx.record(StatePatch { provider_status_words: Some(Some(status)), ..Default::default() }).await?;
                    tokio::time::sleep(Duration::from_millis(wait)).await;
                    attempt += 1;
                    continue;
                }
                return Err(ctx.failure_coded(reason_words, retryable, code));
            }
        }
    }
}
