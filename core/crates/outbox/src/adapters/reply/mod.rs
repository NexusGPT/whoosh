//! Which message does a reply answer, and what are its headers? Lane reply-headers (SPEC A144, migration 0023): the store keeps each
//! mail's Message-ID and References, so the reply reads them there first: the newest message of the thread whose headers are stored,
//! whether or not it still has a place on the server (an archived or moved mail answers offline; no UI path waits on the network).
//! Only when no message of the thread has stored headers (mail stored before 0023 that the fill-in pass has not reached yet) is the
//! mail server asked, as before: the store knows where the newest message of the thread lives (account, folder, UID), and the send
//! crate's `ParentHeadersPort` (implemented by the IMAP adapter) reads its headers.
use crate::ports::ReplyContextPort;
use contracts::CoreError;
use rusqlite::OptionalExtension;
use send::{ParentHeadersPort, ParentRef, ReplyContext, ThreadKind};
use std::sync::Arc;
use store::Store;

/// The hidden test copy's broken copy `WHOOSH_CORE_MUTANT=reply-ignores-store` (engine rust_core.rs sets it, only when `rig`): the reply skips the headers the
/// store holds and asks the server, as before A144. Never set in the owner's copy.
static IGNORE_STORED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn ignore_stored_headers(on: bool) {
    IGNORE_STORED.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub struct StoreReplyContext {
    store: Arc<Store>,
    headers: Arc<dyn ParentHeadersPort>,
}

impl StoreReplyContext {
    pub fn new(store: Arc<Store>, headers: Arc<dyn ParentHeadersPort>) -> Self {
        Self { store, headers }
    }

    /// Where the newest message of a thread that HAS a place on its server sits. A Gmail message under several labels has
    /// rows in `message_location`; any other message sits in its own folder under its remote id (the UID). The owner's own sent
    /// copy of an earlier reply (no location row, no remote id) is skipped: a second reply answers the newest message the server
    /// holds, so its In-Reply-To names that message and its References carry that message's chain.
    fn parent_of(&self, thread_id: &str) -> Result<ParentRef, CoreError> {
        let id = thread_id.to_string();
        let found = self
            .store
            .write(move |ctx| {
                let by_time = "ORDER BY m.received_at_ms DESC, m.id DESC LIMIT 1";
                let placed = "AND ((m.remote_id IS NOT NULL AND m.remote_id <> '') OR EXISTS (SELECT 1 FROM message_location l WHERE l.message_id = m.id))";
                let pick = |ctx: &store::WriteCtx<'_>, filter: &str| -> rusqlite::Result<Option<(String, String, String, Option<String>)>> {
                    ctx.tx
                        .query_row(
                            &format!("SELECT m.id, m.account_id, m.folder, m.remote_id FROM message m WHERE m.thread_id = ?1 {filter} {by_time}"),
                            [&id],
                            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                        )
                        .optional()
                };
                // the newest message with a place on the server; none has one (only the owner's local copies) -> the newest message, which
                // the refusal below names in words (a thread with no message at all stays NotFound)
                let newest = match pick(ctx, placed)? {
                    Some(row) => Some(row),
                    None => pick(ctx, "")?,
                };
                let Some((message_id, account_id, folder, remote_id)) = newest else { return Ok(None) };
                let located: Option<(String, i64)> = ctx
                    .tx
                    .query_row(
                        "SELECT folder, uid FROM message_location WHERE message_id = ?1 AND account_id = ?2 ORDER BY uid_validity DESC, uid DESC LIMIT 1",
                        [&message_id, &account_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;
                Ok(Some(match located {
                    Some((f, uid)) => (account_id, f, Some(uid.to_string())),
                    None => (account_id, folder, remote_id),
                }))
            })
            .map_err(|e| CoreError::Unexpected { reason_words: format!("the outbox could not find the message you reply to: {e}") })?;
        match found {
            None => Err(CoreError::NotFound { kind: "thread".into(), id: thread_id.to_string() }),
            Some((account_id, folder, Some(remote_id))) => Ok(ParentRef { account_id, folder, remote_id }),
            Some(_) => Err(CoreError::Invalid { field: "reply_to_thread_id".into(), reason_words: "the message you reply to has no place on the mail server yet".into() }),
        }
    }
}

impl StoreReplyContext {
    /// The stored headers of the newest message of the thread that has them. `parent_references` is that message's own chain: `ReplyContext`
    /// adds its Message-ID after it (send/src/threading.rs), each id once.
    fn stored(&self, thread_id: &str, kind: ThreadKind) -> Result<Option<ReplyContext>, CoreError> {
        if IGNORE_STORED.load(std::sync::atomic::Ordering::Relaxed) { return Ok(None); }
        let found = self
            .store
            .newest_thread_headers(thread_id)
            .map_err(|e| CoreError::Unexpected { reason_words: format!("the outbox could not read the stored headers of the message you reply to: {e}") })?;
        Ok(found.map(|(_, h)| ReplyContext { kind, parent_message_id: h.message_id, parent_references: h.references }))
    }
}

impl ReplyContextPort for StoreReplyContext {
    fn execute(&self, thread_id: &str, kind: ThreadKind) -> Result<ReplyContext, CoreError> {
        if let Some(found) = self.stored(thread_id, kind)? {
            return Ok(found);
        }
        let parent = self.parent_of(thread_id)?;
        let headers = self.headers.execute(&parent)?;
        Ok(ReplyContext { kind, parent_message_id: headers.message_id, parent_references: headers.references })
    }
}
