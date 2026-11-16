//! The one place a message leaves. Every other use case selects entries and calls this.
//!
//! Exactly-once rules, in order:
//! 1. `may_deliver` is the only guard on the entry itself: Sending, Sent and Cancelled never deliver (a retry
//!    of a sent entry is a no-op).
//! 1b. The idempotency key is the guard across entries: when another entry with this key is already recorded
//!    as sent, this one is closed as sent and nothing is delivered (a retried Send that made a second row, a
//!    record pulled from another device).
//! 2. The ledger comes first: the entry is stored as Sending BEFORE the sender is called.
//!    If the app dies in between, restart finds Sending and asks the owner; it never sends on its own.
//! 3. After a delivered receipt the entry is stored as Sent, and `deliver` is never called for it again.
use crate::ctx::Ctx;
use crate::domain::Record;
use crate::domain::CopyPending;
use crate::ports::{DraftEffect, SentCopy};
use contracts::{ActionFailed, CoreError, CoreEvent, OutboxState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Delivered,
    /// No network: the entry waits and leaves on reconnect.
    Waiting,
    /// The server (or the compose step) refused; the words are in the entry.
    Failed,
    /// Nothing to do, with the reason in words.
    Skipped(&'static str),
}

fn skip_reason(r: &Record) -> &'static str {
    match r.entry.state {
        OutboxState::Sent { .. } => "this message was already sent",
        OutboxState::Sending => "this message is being sent now",
        OutboxState::Cancelled => "this message was cancelled",
        _ => "this message cannot be sent",
    }
}

/// When another entry with this entry's key is already Sent: the time it left.
fn sent_with_same_key(ctx: &Ctx, record: &Record) -> Result<Option<i64>, CoreError> {
    Ok(ctx.ports.find_by_key.execute(&record.idempotency_key)?.iter().filter(|o| o.id() != record.id()).find_map(|o| match o.entry.state {
        OutboxState::Sent { at_ms } => Some(at_ms),
        _ => None,
    }))
}

fn failure_of(ctx: &Ctx, record: &Record, error: CoreError) -> ActionFailed {
    match error {
        CoreError::Action { failure } => failure,
        CoreError::Invalid { reason_words, .. } => ActionFailed { account_name: ctx.ports.account_names.execute(&record.entry.draft.from_account_id), reason_words, retryable: false, code: None },
        other => ActionFailed { account_name: ctx.ports.account_names.execute(&record.entry.draft.from_account_id), reason_words: other.to_string(), retryable: true, code: None },
    }
}

/// A connection that broke before the end of the message body: the code `network_<stage>` that send/src/smtp.rs writes, except after DATA.
pub(crate) fn transient_break(f: &ActionFailed) -> bool {
    f.code.as_deref().is_some_and(|c| c.starts_with(send::smtp::NETWORK_CODE_PREFIX) && c != AFTER_DATA_CODE)
}

/// A "try later" ANSWER of the server: a reply code 4xx (send/src/smtp.rs writes it as "451 4.7.0"). The server did not accept the message,
/// so another try cannot make a second copy. Lead, 13 Nov (owner picture 208, "mail cannot be sent"): Microsoft 365 answered
/// `451 4.7.0 Temporary server error. Please try again later.` at AUTH and the entry stayed failed, waiting for the owner's Retry.
pub(crate) fn transient_answer(f: &ActionFailed) -> bool {
    f.retryable && f.code.as_deref().is_some_and(|c| c.len() >= 3 && c.as_bytes()[0] == b'4' && c.as_bytes()[..3].iter().all(u8::is_ascii_digit))
}

/// The code of a break after the whole body was written (send/src/smtp.rs `Stage::AfterData`).
pub(crate) const AFTER_DATA_CODE: &str = "network_after_data";

pub(crate) fn execute(ctx: &mut Ctx, id: &str) -> Result<Outcome, CoreError> {
    let mut record = ctx.get(id)?;
    if !record.may_deliver() {
        return Ok(Outcome::Skipped(skip_reason(&record)));
    }
    let now = ctx.now();
    if let Some(first) = sent_with_same_key(ctx, &record)? {
        // The message already left under this key: close this entry with the first one's time and send nothing.
        record.mark_sent(first, now);
        ctx.commit(&record, DraftEffect::Keep, true)?;
        return Ok(Outcome::Skipped("a message with this key was already sent"));
    }
    if !ctx.ports.online.execute() {
        if !record.waiting {
            let account_name = ctx.ports.account_names.execute(&record.entry.draft.from_account_id);
            let failure = ActionFailed { account_name, reason_words: "there is no network; it will be sent when you are back online".into(), retryable: true, code: None };
            record.mark_failed(failure, true, now);
            ctx.commit(&record, DraftEffect::Keep, true)?;
        }
        return Ok(Outcome::Waiting);
    }

    record.begin_sending(now);
    ctx.commit(&record, DraftEffect::Keep, true)?; // the ledger, before the effect

    match ctx.ports.sender.execute(&record.entry.draft, record.threading.as_ref()) {
        Ok(delivered) => {
            record.mark_sent(delivered.sent_at_ms, ctx.now());
            ctx.commit(&record, DraftEffect::Keep, true)?;
            ctx.events.push(CoreEvent::SendCompleted { outbox_id: record.entry.id.clone(), sent_at_ms: delivered.sent_at_ms });
            if let SentCopy::Pending(pending) = delivered.copy {
                // Delivered stays delivered. The copy is kept in the row and retried on later ticks.
                record.copy_not_saved(CopyPending { words: pending.failure.reason_words, rfc822: pending.rfc822 }, ctx.now());
                ctx.commit(&record, DraftEffect::Keep, false)?;
            }
            Ok(Outcome::Delivered)
        }
        Err(error) => {
            let failure = failure_of(ctx, &record, error);
            let now = ctx.now();
            // fix-realcore round 15 (owner 13 Nov: one reset, never tried again): a connection that broke before the end of the body
            // (`network_<stage>`, send/src/smtp.rs) is tried again by itself, SEND_RETRY_TRIES_MAX tries in all. A break AFTER the body
            // (`network_after_data`) is never tried again: the server may already hold the message, and the next try would mint a new
            // Message-ID, so a second copy could reach the recipient; it stays failed with words that say so, for the owner to decide.
            match (transient_break(&failure) || transient_answer(&failure), Record::retry_pause_ms(record.attempts)) {
                (true, Some(pause)) => record.mark_retrying(failure, now + pause, now),
                _ => record.mark_failed(failure, false, now),
            }
            ctx.commit(&record, DraftEffect::Keep, true)?;
            Ok(Outcome::Failed)
        }
    }
}
