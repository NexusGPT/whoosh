//! IMAP adapter behind the ImapMailAdapter port (fn-sync-imap).
//!
//! Open-source start: `async-imap` 0.12 (MIT OR Apache-2.0) is the protocol client, `imap-proto`
//! its parser, `rustls` the TLS, `mail-parser` the header reader. This crate adds what none of
//! them has: the first-screen-before-history order, the IDLE re-issue loop on an injected clock,
//! the QRESYNC/CONDSTORE resync with a fallback, folder roles, the per-provider connection table,
//! Gmail's message id and labels, a redacted protocol transcript, `validate` for the add-account
//! screen, and the cross-account transfer that never loses a message.
mod adapter;
mod blocking;
mod checked;
mod command_limit;
mod connections;
mod consts;
mod envelope;
mod idle;
mod port;
mod roles;
mod transcript;
mod transfer;
mod transport;
mod validate;
mod xoauth2;

pub use blocking::{parent_headers_from, AccountAdapters, ImapParentHeaders, ImapSentAppender};
pub use adapter::{Auth, ImapAdapter, ImapConfig, Security, TokenSource};
pub use xoauth2::OAuthTokenSource;
pub use consts::{IMAP_CONN_PER_ACCOUNT, IMAP_IDLE_REISSUE_MIN};
pub use idle::{IdleEvent, Sleeper, TokioSleeper};
pub use port::*;
pub use connections::{ConnectionTable, ImapLimits, Provider};
pub use roles::{role_and_source, role_for, FolderRole, RoleBy};
pub use transcript::Transcript;
pub use validate::{validate, Credential, ImapEndpoint, RoleMapping, SmtpEndpoint, Step, StepResult, ValidationConfig, ValidationReport};
pub use transfer::{transfer, TransferMode};
pub use xoauth2::xoauth2_initial_response;
