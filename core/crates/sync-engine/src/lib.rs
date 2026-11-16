//! Whoosh sync engine: the scheduler for 30+ accounts.
//!
//! Order: the first screen of every account first, then full-history backfill newest to oldest per folder.
//! Limits: per-account worker caps from the ledger, at most `backfill_accounts` accounts backfilling at once,
//! provider limits shown as "slowed by <provider>" on that account only. One failing account records its
//! error in the server's words and never stalls the others. Every row goes through the store's writer;
//! a restart resumes each folder from its stored anchor.
pub mod account;
pub mod backfill;
pub mod config;
pub mod consts;
pub mod engine;
pub mod first_screen;
pub mod gate;
pub mod limits;
pub mod port;
pub mod retry;
pub mod state;
pub mod sync_account;
pub mod words;

pub use config::EngineConfig;
pub use engine::{Engine, Resume};
pub use limits::{provider_name, slowed_status, slowed_words, worker_cap};
pub use port::{AccountSource, Page, SourceError, Transport};
pub use state::{parse_failure, FolderRow, Progress, StatePatch};
pub use sync_account::Outcome;
