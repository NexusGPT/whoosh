//! The send crate (lettre) behind `SendEntryPort`.
use crate::ports::{AttachmentFilesPort, Delivered, SendAccountPort, SendEntryPort, SentCopy, PendingCopy};
use contracts::{Clock, CoreError, Draft};
use send::{AppendSentPort, DeliverPort, ReplyContext, SendInput, SendMessage};
use std::sync::Arc;

pub struct SendCrateSender {
    accounts: Arc<dyn SendAccountPort>,
    send: SendMessage,
    /// delta 22: where a draft's files are read when its message is built (None: a draft that names a file is refused by the send crate).
    files: Option<Arc<dyn AttachmentFilesPort>>,
}

impl SendCrateSender {
    pub fn new(accounts: Arc<dyn SendAccountPort>, deliver: Arc<dyn DeliverPort>, append: Arc<dyn AppendSentPort>, clock: Arc<dyn Clock>) -> Self {
        Self { accounts, send: SendMessage::new(deliver, append, clock), files: None }
    }

    /// delta 22: the sender reads each draft's files through `files` when it builds the message.
    pub fn with_files(mut self, files: Arc<dyn AttachmentFilesPort>) -> Self {
        self.files = Some(files);
        self
    }
}

impl SendEntryPort for SendCrateSender {
    /// The body goes in as the owner wrote it; the send crate appends the signature (once).
    fn execute(&self, draft: &Draft, reply: Option<&ReplyContext>) -> Result<Delivered, CoreError> {
        let (account, signature) = self.accounts.execute(&draft.from_account_id)?;
        let files = match (&self.files, draft.attachment_ids.is_empty()) {
            (Some(port), false) => port.execute(draft)?,
            _ => Vec::new(),
        };
        let receipt = self.send.execute(&SendInput { account, draft: draft.clone(), signature, reply: reply.cloned(), files })?;
        let copy = match receipt.sent_copy {
            send::SentCopy::Stored => SentCopy::Stored,
            send::SentCopy::Pending { failure, rfc822 } => SentCopy::Pending(PendingCopy { account_id: draft.from_account_id.clone(), rfc822, failure }),
        };
        Ok(Delivered { message_id: receipt.message_id, sent_at_ms: receipt.sent_at_ms, copy })
    }
}
