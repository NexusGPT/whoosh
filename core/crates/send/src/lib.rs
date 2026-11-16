//! The send adapter (fn-send-smtp): compose one message with the address's signature,
//! deliver it once over SMTP through lettre, then hand the Sent copy to the IMAP adapter's port.
//!
//! One file per concern: `account` (what an account sends with), `ports` (one method each),
//! `compose` (the message), `smtp` (the lettre adapter), `send_message` (the use case).
pub mod account;
pub mod attachments;
pub mod compose;
pub mod ports;
pub mod rich;
pub mod send_message;
pub mod smtp;
pub mod threading;

pub use attachments::{check_built, part_bytes, OutgoingFile};
pub use account::{Security, SendAccount, SmtpCredentials, SmtpSettings};
pub use compose::{compose, Composed, SIGNATURE_DELIMITER};
pub use ports::{AppendSentPort, AppendSentRequest, DeliverPort, DeliverRequest, Envelope};
pub use send_message::{SendInput, SendMessage, SendReceipt, SentCopy};
pub use smtp::{LettreDelivery, LoggedDelivery, StepLog};
pub use threading::{parse_message_ids, ParentHeaders, ParentHeadersPort, ParentRef, ReplyContext, ThreadKind};
