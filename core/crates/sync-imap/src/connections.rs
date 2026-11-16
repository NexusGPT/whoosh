//! The per-provider connection table. The sync engine reads one `ConnectionTable` per account:
//! how many connections the account may open, how many of them may sit in IDLE, and how long to
//! wait before a reconnect. Every number comes in through `ImapLimits`, which the engine fills
//! from docs/CONSTANTS.md; this crate types none of them.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider { Gmail, Other }

impl Provider {
    /// Gmail is recognised by its IMAP host name (the table is needed before any connection).
    pub fn from_host(host: &str) -> Provider {
        let h = host.to_ascii_lowercase();
        if h == "imap.gmail.com" || h == "imap.googlemail.com" { Provider::Gmail } else { Provider::Other }
    }
}

/// The ledger numbers this crate needs, handed in by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImapLimits {
    /// Connections per account for a host with no special rule (IMAP_MAX_CONNECTIONS_DEFAULT). This is the PROVIDER cap, the ceiling
    /// of the adapter's pool; how many calls Whoosh keeps in flight is the sync engine's worker cap (IMAP_CONN_PER_ACCOUNT, 2) for
    /// every account reached over IMAP, a Microsoft 365 account with XOAUTH2 included (sync-engine `worker_cap`).
    pub max_default: usize,
    /// Connections per account on Gmail (IMAP_MAX_CONNECTIONS_GMAIL).
    pub max_gmail: usize,
    /// Connections per account that may sit in IDLE (IMAP_IDLE_CONNECTIONS_PER_ACCOUNT).
    pub idle_per_account: usize,
    /// First wait before a reconnect (SYNC_BACKOFF_START_MS, the engine's existing backoff).
    pub backoff_start_ms: u64,
    /// The longest wait between reconnects (SYNC_BACKOFF_MAX_MS).
    pub backoff_max_ms: u64,
    /// Lines the protocol transcript keeps (IMAP_TRANSCRIPT_LINES).
    pub transcript_lines: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionTable {
    pub provider: Provider,
    pub max_connections: usize,
    pub idle_connections: usize,
    pub backoff_start_ms: u64,
    pub backoff_max_ms: u64,
}

impl ImapLimits {
    pub fn table_for(&self, host: &str) -> ConnectionTable {
        let provider = Provider::from_host(host);
        let max = match provider { Provider::Gmail => self.max_gmail, Provider::Other => self.max_default };
        ConnectionTable {
            provider,
            // A zero cap would make every call wait for ever: one connection is the floor.
            max_connections: max.max(1),
            idle_connections: self.idle_per_account.min(max.max(1)),
            backoff_start_ms: self.backoff_start_ms,
            backoff_max_ms: self.backoff_max_ms,
        }
    }
}

impl ConnectionTable {
    /// The wait before reconnect number `attempt` (0 is the first): the start value, doubled each
    /// time, never above the maximum.
    pub fn reconnect_delay(&self, attempt: u32) -> Duration {
        let ms = self.backoff_start_ms.saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX)).min(self.backoff_max_ms);
        Duration::from_millis(ms)
    }
}
