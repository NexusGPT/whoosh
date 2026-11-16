//! The engine's numbers. Ledger numbers come from `consts` and the store; the three with no ledger row
//! yet (history page, backoff base and ceiling, retry count) are inputs with no default value.
use crate::consts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineConfig {
    /// Messages in an account's first screen (the inbox page the stream carries).
    pub first_screen: u32,
    /// Messages per backfill page.
    pub history_page: u32,
    /// Accounts backfilling at once.
    pub backfill_accounts: usize,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
    /// Retries of a retryable failure before the account records it and stops.
    pub retry_max: u32,
}

impl EngineConfig {
    /// The ledger values for this device plus the three numbers that have no ledger row yet.
    pub fn from_ledger(history_page: u32, backoff_base_ms: u64, backoff_max_ms: u64, retry_max: u32) -> Self {
        let backfill_accounts = if cfg!(target_os = "ios") { consts::BACKFILL_CONCURRENT_IPHONE } else { consts::BACKFILL_CONCURRENT_MAC };
        EngineConfig { first_screen: store::INBOX_PAGE_ROWS as u32, history_page, backfill_accounts, backoff_base_ms, backoff_max_ms, retry_max }
    }

    /// Wait before retry `attempt` (0-based): base doubled per attempt, never above the ceiling.
    pub fn backoff_ms(&self, attempt: u32) -> u64 {
        self.backoff_base_ms.saturating_mul(1u64 << attempt.min(20)).min(self.backoff_max_ms)
    }
}
