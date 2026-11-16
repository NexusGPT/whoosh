//! The use case: compose, deliver once, then put the Sent copy in the account's Sent folder.
//! Orchestration only; both effects go through ports.
use crate::account::SendAccount;
use crate::attachments::OutgoingFile;
use crate::compose::compose;
use crate::ports::{AppendSentPort, AppendSentRequest, DeliverPort, DeliverRequest};
use crate::threading::ReplyContext;
use contracts::{ActionFailed, Clock, CoreError, Draft, Signature};
use std::sync::Arc;

pub struct SendInput {
    pub account: SendAccount,
    pub draft: Draft,
    pub signature: Option<Signature>,
    /// The message this one answers or forwards; its headers become In-Reply-To and References.
    pub reply: Option<ReplyContext>,
    /// delta 22: the draft's files, read when the message is built, in the draft's order (one per `draft.attachment_ids`).
    pub files: Vec<OutgoingFile>,
}

/// Where the Sent copy stands after a delivery.
#[derive(Debug, Clone, PartialEq)]
pub enum SentCopy {
    Stored,
    /// The message WAS delivered but the Sent folder did not take its copy. The caller must not
    /// send again; it retries only the append with these bytes.
    Pending { failure: ActionFailed, rfc822: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SendReceipt {
    pub message_id: String,
    pub sent_at_ms: i64,
    pub sent_copy: SentCopy,
}

pub struct SendMessage {
    deliver: Arc<dyn DeliverPort>,
    append: Arc<dyn AppendSentPort>,
    clock: Arc<dyn Clock>,
}

impl SendMessage {
    pub fn new(deliver: Arc<dyn DeliverPort>, append: Arc<dyn AppendSentPort>, clock: Arc<dyn Clock>) -> Self {
        Self { deliver, append, clock }
    }

    /// Delivery failure: Err, nothing was sent and nothing was appended.
    /// Delivery success: Ok, with the Sent copy outcome in the receipt.
    pub fn execute(&self, input: &SendInput) -> Result<SendReceipt, CoreError> {
        let now_ms = self.clock.now_ms();
        let composed = compose(&input.account, &input.draft, input.signature.as_ref(), now_ms, input.reply.as_ref(), &input.files)?;

        self.deliver.execute(&DeliverRequest {
            account_name: input.account.name.clone(),
            smtp: input.account.smtp.clone(),
            envelope: composed.envelope.clone(),
            rfc822: composed.rfc822.clone(),
        })?;

        let appended = self.append.execute(&AppendSentRequest { account_id: input.account.id.clone(), rfc822: composed.rfc822.clone() });
        let sent_copy = match appended {
            Ok(()) => SentCopy::Stored,
            Err(e) => SentCopy::Pending {
                failure: ActionFailed {
                    account_name: input.account.name.clone(),
                    reason_words: format!("Sent, copy not saved: the message left, but the Sent folder did not take its copy ({e})"),
                    retryable: true,
                    code: None,
                },
                rfc822: composed.rfc822,
            },
        };
        Ok(SendReceipt { message_id: composed.message_id, sent_at_ms: now_ms, sent_copy })
    }
}
