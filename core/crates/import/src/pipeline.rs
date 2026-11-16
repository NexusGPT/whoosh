//! The import: a reader walks the source, each message is converted and batched, a batch is written through the sink, and the
//! progress callback reports counts per mailbox. Memory holds one message plus one batch.
use std::collections::HashMap;

use crate::apple::{count_apple_mail, walk_apple_mail};
use crate::consts::{IMPORT_BATCH_BYTES, IMPORT_BATCH_MESSAGES};
use crate::convert::convert;
use crate::error::ImportError;
use crate::mbox::walk_mbox;
use crate::model::{AccountAddress, ImportProgress, ImportReport, MailVisitor, MailboxCount, RawMail, Source, SkippedFile};
use crate::olm::{count_olm, walk_olm};
use crate::sink::MessageSink;
use store::NewMessage;

/// Who the messages can belong to.
pub struct ImportContext<'a> {
    /// The accounts on this Mac, by receiving address.
    pub accounts: &'a [AccountAddress],
    /// The account that takes a message no address matches ("Imported").
    pub imported_account_id: &'a str,
}

/// How many messages the source holds, when that is cheap: Apple Mail and Outlook count entries without reading messages;
/// an mbox has no index, so it answers None.
pub fn count_messages(source: &Source) -> Result<Option<u64>, ImportError> {
    match source {
        Source::AppleMail { mail_dir } => Ok(Some(count_apple_mail(mail_dir)?.iter().map(|(_, n)| n).sum())),
        Source::Olm { file } => count_olm(file).map(Some),
        Source::Mbox { .. } => Ok(None),
    }
}

/// Read the whole source into the sink. `progress` is called after every batch written, at the end of every mailbox and once at
/// the end (`finished: true`). A message that cannot be read is skipped by name; a failure of the whole source or of the
/// database stops the run and is returned in words.
pub fn import(source: &Source, ctx: &ImportContext<'_>, sink: &mut dyn MessageSink, progress: &mut dyn FnMut(&ImportProgress)) -> Result<ImportReport, ImportError> {
    let total_known = count_messages(source)?;
    let mut run = Run {
        ctx,
        sink,
        progress,
        kind: source.kind(),
        total_known,
        current: String::new(),
        batch: Vec::new(),
        batch_bytes: 0,
        counts: Vec::new(),
        skipped: Vec::new(),
        per_account: HashMap::new(),
        deferred: None,
        flagged_ids: Vec::new(),
    };
    match source {
        Source::AppleMail { mail_dir } => walk_apple_mail(mail_dir, &mut run)?,
        Source::Olm { file } => walk_olm(file, &mut run)?,
        Source::Mbox { file } => walk_mbox(file, &mut run)?,
    }
    if let Some(e) = run.deferred.take() {
        return Err(e);
    }
    run.flush()?;
    run.report(true);
    Ok(run.finish())
}

struct Run<'a> {
    ctx: &'a ImportContext<'a>,
    sink: &'a mut dyn MessageSink,
    progress: &'a mut dyn FnMut(&ImportProgress),
    kind: crate::model::SourceKind,
    total_known: Option<u64>,
    current: String,
    batch: Vec<NewMessage>,
    batch_bytes: usize,
    counts: Vec<MailboxCount>,
    skipped: Vec<SkippedFile>,
    per_account: HashMap<String, u64>,
    /// A write that failed while a skip was being recorded (a skip cannot return an error); it surfaces on the next call.
    deferred: Option<ImportError>,
    flagged_ids: Vec<String>,
}

impl Run<'_> {
    fn counts_for(&mut self, mailbox: &str) -> &mut MailboxCount {
        let at = match self.counts.iter().position(|c| c.mailbox == mailbox) {
            Some(i) => i,
            None => {
                self.counts.push(MailboxCount { mailbox: mailbox.to_string(), seen: 0, stored: 0, skipped: 0 });
                self.counts.len() - 1
            }
        };
        &mut self.counts[at]
    }

    /// Write the batch. A batch only ever holds one mailbox, so the stored count goes to that mailbox.
    fn flush(&mut self) -> Result<(), ImportError> {
        if self.batch.is_empty() {
            return Ok(());
        }
        let batch = std::mem::take(&mut self.batch);
        self.batch_bytes = 0;
        let stored = self.sink.insert(batch)? as u64;
        let current = self.current.clone();
        self.counts_for(&current).stored += stored;
        self.report(false);
        Ok(())
    }

    fn report(&mut self, finished: bool) {
        let seen: u64 = self.counts.iter().map(|c| c.seen).sum();
        let stored: u64 = self.counts.iter().map(|c| c.stored).sum();
        let skipped: u64 = self.counts.iter().map(|c| c.skipped).sum();
        let p = ImportProgress { kind: self.kind, mailbox: self.current.clone(), total_known: self.total_known, seen, stored, skipped, mailboxes: self.counts.clone(), finished };
        (self.progress)(&p);
    }

    fn finish(self) -> ImportReport {
        let seen: u64 = self.counts.iter().map(|c| c.seen).sum();
        let stored: u64 = self.counts.iter().map(|c| c.stored).sum();
        let skipped_n: u64 = self.counts.iter().map(|c| c.skipped).sum();
        let mut per_account: Vec<(String, u64)> = self.per_account.into_iter().collect();
        per_account.sort();
        ImportReport { kind: self.kind, seen, stored, already_there: seen.saturating_sub(skipped_n).saturating_sub(stored), skipped: self.skipped, mailboxes: self.counts, per_account, flagged_ids: self.flagged_ids }
    }

    /// A change of mailbox closes the batch of the one before and tells the progress callback.
    fn enter(&mut self, mailbox: &str) -> Result<(), ImportError> {
        if self.current != mailbox {
            self.flush()?;
            if !self.current.is_empty() {
                self.report(false);
            }
            self.current = mailbox.to_string();
            self.counts_for(mailbox);
        }
        Ok(())
    }
}

impl MailVisitor for Run<'_> {
    fn mail(&mut self, mail: RawMail) -> Result<(), ImportError> {
        if let Some(e) = self.deferred.take() {
            return Err(e);
        }
        self.enter(&mail.mailbox)?;
        self.counts_for(&mail.mailbox).seen += 1;
        match convert(&mail, self.ctx.accounts, self.ctx.imported_account_id) {
            Err(words) => {
                self.counts_for(&mail.mailbox).skipped += 1;
                self.skipped.push(SkippedFile { origin: mail.origin, words });
            }
            Ok(done) => {
                if done.flagged {
                    self.flagged_ids.push(done.new.message.id.clone());
                }
                *self.per_account.entry(done.account_id).or_insert(0) += 1;
                self.batch_bytes += mail.bytes.len();
                self.batch.push(done.new);
                if self.batch.len() >= IMPORT_BATCH_MESSAGES || self.batch_bytes >= IMPORT_BATCH_BYTES {
                    self.flush()?;
                }
            }
        }
        Ok(())
    }

    fn skip(&mut self, mailbox: &str, origin: String, words: String) {
        if let Err(e) = self.enter(mailbox) {
            self.deferred.get_or_insert(e);
        }
        let c = self.counts_for(mailbox);
        c.seen += 1;
        c.skipped += 1;
        self.skipped.push(SkippedFile { origin, words });
    }
}
