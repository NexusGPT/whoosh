//! Microsoft Graph mail sync for Whoosh: per-folder delta rounds, change-notification
//! subscriptions for the Mac, and the per-mailbox limits (read from docs/CONSTANTS.md).
//!
//! Open-source start: `reqwest` over the Graph REST shapes with `serde`; see
//! docs/lanes/core-sync-graph.md for the Graph client crates that were checked.
//! No request leaves the machine in tests: they run against recorded fixtures served from 127.0.0.1.
pub mod client;
pub mod delta;
pub mod folders;
pub mod limits;
pub mod model;
pub mod notify;
pub mod throttle;
pub mod time;

pub use client::{slowed_words, GraphConfig, GraphMailbox, SlowedSink, TokenSource};
pub use delta::DeltaRound;
pub use folders::GraphFolder;
pub use model::DeltaChange;
pub use notify::{folders_to_sync, parse_inbound, ChangeType, GraphSubscription, Inbound, LifecycleEvent, LifecycleNotice, Notice};
pub use throttle::{Limits, Throttle};
pub use time::{BoxFut, Sleeper, TokioSleeper};
