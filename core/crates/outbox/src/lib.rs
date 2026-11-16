//! The outbox (fn-outbox-undo-send, fn-outbox-schedule, fn-outbox-crossdevice).
//!
//! Layout follows the backend seam ladder: `domain` (the record and its transitions, no I/O),
//! `ports` (one method each), `usecases` (one file per use case, orchestration only),
//! `adapters/<system>` (store, send, cloud sync). `outbox` is the thin facade.
//!
//! Time is driven from outside: nothing here sleeps or starts a thread. The app calls `Outbox::tick`
//! on a timer and on wake; tests call it with a clock they move.
pub mod adapters;
pub mod config;
pub mod domain;
mod ctx;
mod outbox;
pub mod ports;
mod usecases;

pub use config::{undo_bounds, DeviceRole, OutboxConfig, UndoBounds};
pub use ctx::{Ports, SyncPorts};
pub use domain::{CopyPending, Record};
pub use outbox::{EntryView, Outbox, SendOutcome, TickReport};
pub use ports::{
    AccountNamePort, AttachmentFilesPort, Delivered, DraftEffect, FindByKeyPort, GetEntryPort, ListOpenEntriesPort, ListPendingCopiesPort, LoadDraftPort, ReplyContextPort, OnlinePort, PendingCopy, SentCopy,
    PublishChangePort, PullChangesPort, SaveEntryPort, SaveRequest, SendAccountPort, SendEntryPort,
};
