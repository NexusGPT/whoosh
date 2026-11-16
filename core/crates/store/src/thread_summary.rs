//! lane fix-aicore (migration 0030): the reader's thread summary, kept on this Mac in two columns of `thread`: `summary_text` (what the
//! owner's writing model wrote) and `summary_of` (the newest message of the thread when it was made; a newer message makes it stale).
//! The engine (core/crates/engine/src/thread_summary.rs) reads it on the reader's path (one indexed read, never a model call) and writes it
//! from its background worker.
use crate::error::StoreError;
use crate::Store;
use rusqlite::{params, OptionalExtension};

/// What the store holds for one thread's summary, beside the newest message the thread has now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSummaryRow {
    /// The kept summary, None when none was made.
    pub text: Option<String>,
    /// The newest message the kept summary was made from.
    pub made_from: Option<String>,
    /// The newest message of the thread now (by `received_at_ms`, then id), None for a thread with no message.
    pub newest: Option<String>,
}

impl ThreadSummaryRow {
    /// True when a summary must be made: none is kept, or a newer message joined the thread since it was made.
    pub fn is_stale(&self) -> bool { self.newest.is_some() && (self.text.is_none() || self.made_from != self.newest) }
}

/// One message of a thread as the summary's prompt reads it: who, when, the subject and the text (the stored body, else the preview).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryMessage {
    pub id: String,
    pub sender: String,
    pub received_at_ms: i64,
    pub subject: String,
    pub text: String,
}

impl Store {
    /// The kept summary of `thread_id` and the thread's newest message, in one read (index `message_by_thread`). None for an unknown thread.
    pub fn thread_summary(&self, thread_id: &str) -> Result<Option<ThreadSummaryRow>, StoreError> {
        self.read(|c| {
            Ok(c.prepare_cached(
                "SELECT t.summary_text, t.summary_of, \
                 (SELECT m.id FROM message m WHERE m.thread_id = t.id ORDER BY m.received_at_ms DESC, m.id DESC LIMIT 1) \
                 FROM thread t WHERE t.id = ?1",
            )?
            .query_row(params![thread_id], |r| Ok(ThreadSummaryRow { text: r.get(0)?, made_from: r.get(1)?, newest: r.get(2)? }))
            .optional()?)
        })
    }

    /// The newest `limit` messages of `thread_id`, oldest first (what a summary is made from).
    pub fn thread_summary_messages(&self, thread_id: &str, limit: usize) -> Result<Vec<SummaryMessage>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached(
                "SELECT id, CASE WHEN sender_name <> '' THEN sender_name ELSE from_address END, received_at_ms, subject, COALESCE(NULLIF(body_text, ''), preview) \
                 FROM message WHERE thread_id = ?1 ORDER BY received_at_ms DESC, id DESC LIMIT ?2",
            )?;
            let rows = st.query_map(params![thread_id, limit as i64], |r| {
                Ok(SummaryMessage { id: r.get(0)?, sender: r.get(1)?, received_at_ms: r.get(2)?, subject: r.get(3)?, text: r.get(4)? })
            })?;
            let mut out = rows.collect::<Result<Vec<_>, _>>()?;
            out.reverse();
            Ok(out)
        })
    }

    /// Keeps `text` as the summary of `thread_id`, made from the newest message `made_from`. Quiet: no list, count or thread record shows
    /// these columns, so nothing is published (the update hook's notes are dropped, as `set_reply_to_quiet` does). False for an unknown thread.
    pub fn thread_summary_put(&self, thread_id: &str, text: &str, made_from: &str) -> Result<bool, StoreError> {
        let (id, text, of) = (thread_id.to_string(), text.to_string(), made_from.to_string());
        self.submit(move |st| {
            let out = (|| -> Result<bool, StoreError> {
                let n = st.conn.prepare_cached("UPDATE thread SET summary_text = ?2, summary_of = ?3 WHERE id = ?1")?.execute(params![id, text, of])?;
                Ok(n > 0)
            })();
            st.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out
        })?
    }
}
