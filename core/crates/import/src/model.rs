//! The shapes the readers hand to the pipeline and the pipeline hands to the engine.
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKind {
    AppleMail,
    Olm,
    Mbox,
}

/// What to read. `AppleMail.mail_dir` is the folder that holds the `V*` folders (`~/Library/Mail`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    AppleMail { mail_dir: PathBuf },
    Olm { file: PathBuf },
    Mbox { file: PathBuf },
}

impl Source {
    pub fn kind(&self) -> SourceKind {
        match self {
            Source::AppleMail { .. } => SourceKind::AppleMail,
            Source::Olm { .. } => SourceKind::Olm,
            Source::Mbox { .. } => SourceKind::Mbox,
        }
    }
}

/// The state a source kept for one message. Where a source says nothing, `read` is true: an archive is not new mail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailFlags {
    pub read: bool,
    pub flagged: bool,
    pub answered: bool,
    pub draft: bool,
    /// The source marked the message deleted; the import leaves it out.
    pub deleted: bool,
}

impl Default for MailFlags {
    fn default() -> Self {
        MailFlags { read: true, flagged: false, answered: false, draft: false, deleted: false }
    }
}

/// A file that belongs to a message but is not inside its bytes: a partial Apple Mail message's attachment, an Outlook attachment entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRef {
    pub name: String,
    pub content_type: String,
    pub size: u64,
    pub content_id: Option<String>,
    /// A path on disk, or the entry name inside the archive.
    pub location: String,
}

/// One message as a reader found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMail {
    /// The mailbox it sat in, nested ones joined by "/" ("Clients/Acme").
    pub mailbox: String,
    /// The RFC 822 message.
    pub bytes: Vec<u8>,
    pub flags: MailFlags,
    /// When the source says the message arrived (Apple's date-received, an mbox From_ line, Outlook's received time).
    pub received_ms: Option<i64>,
    pub attachments: Vec<AttachmentRef>,
    /// The file or archive entry it came from, for the words about a skipped message.
    pub origin: String,
}

/// A message or file the import left out, named and with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedFile {
    pub origin: String,
    pub words: String,
}

/// The receiving addresses of the accounts on this Mac, for the match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountAddress {
    pub id: String,
    pub address: String,
}

/// Counts for one mailbox of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailboxCount {
    pub mailbox: String,
    /// Messages and files looked at.
    pub seen: u64,
    /// New rows written to the local database.
    pub stored: u64,
    /// Left out (unreadable, too large, marked deleted).
    pub skipped: u64,
}

/// What the callback shows while the import runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProgress {
    pub kind: SourceKind,
    /// The mailbox being read now.
    pub mailbox: String,
    /// How many messages the source holds, when that is cheap to know (Apple Mail, Outlook); None for an mbox until it is read.
    pub total_known: Option<u64>,
    pub seen: u64,
    pub stored: u64,
    pub skipped: u64,
    pub mailboxes: Vec<MailboxCount>,
    pub finished: bool,
}

/// The end result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub kind: SourceKind,
    pub seen: u64,
    pub stored: u64,
    /// Seen, readable, and already in the database (an earlier run, or a copy under another mailbox).
    pub already_there: u64,
    pub skipped: Vec<SkippedFile>,
    pub mailboxes: Vec<MailboxCount>,
    /// Messages given to each account id (by receiving address, else the Imported account).
    pub per_account: Vec<(String, u64)>,
    /// Ids of the messages the source had flagged; the engine files them under Starred once they are stored (as it does for synced mail).
    pub flagged_ids: Vec<String>,
}

/// What a reader calls while it walks a source. The pipeline implements it; a test can too.
pub trait MailVisitor {
    /// One message. An `Err` stops the walk (the store refused, say).
    fn mail(&mut self, mail: RawMail) -> Result<(), crate::ImportError>;
    /// A file or message that could not be read; the walk goes on.
    fn skip(&mut self, mailbox: &str, origin: String, words: String);
}
