//! Whoosh import crate: read a local mail store (Apple Mail message files, an Outlook for Mac `.olm` export, a Thunderbird or Unix
//! mbox) and hand each message to the same local database the sync writes to. Imported mail is local-only rows: nothing here
//! talks to a server, and nothing is ever pushed to one.
//!
//! A message goes: reader (one file, entry or mbox message at a time) -> `parse::analyse` -> an account by receiving address,
//! else the Imported account -> a batch -> `MessageSink::insert`. The row id is `imp:<account id>:<Message-ID>` (a content hash
//! when there is no Message-ID), so the same message imported twice is one row. The numbers (batch size, size limit) come from
//! docs/CONSTANTS.md through build.rs.
mod apple;
mod consts;
mod convert;
mod emlx;
mod error;
mod matching;
mod mbox;
mod model;
mod olm;
mod olm_build;
mod pipeline;
mod rawhdr;
mod sink;
mod timeutil;

pub use consts::{IMPORT_BATCH_BYTES, IMPORT_BATCH_MESSAGES, IMPORT_MAX_MESSAGE_BYTES};
pub use apple::{count_apple_mail, find_apple_mail, walk_apple_mail};
pub use convert::{content_key, convert, folder_for, message_key, Converted};
pub use emlx::{parse_emlx, Emlx};
pub use error::{classify_io_error, is_permission_refusal, ImportError, FULL_DISK_ACCESS_WORDS};
pub use matching::{account_for, candidate_addresses, is_sent_mailbox};
pub use mbox::{flags_from_headers, walk_mbox, MboxMessage, MboxMessages};
pub use model::{
    AccountAddress, AttachmentRef, ImportProgress, ImportReport, MailFlags, MailVisitor, MailboxCount, RawMail, SkippedFile, Source, SourceKind,
};
pub use olm::{count_olm, is_message_entry, mailbox_of, walk_olm};
pub use pipeline::{count_messages, import, ImportContext};
pub use sink::{MessageSink, StoreSink};
