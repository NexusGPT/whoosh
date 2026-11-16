//! "Reminder if no reply" (GotEmail 0.9.7): a message the owner sent that is still the newest of its thread after the
//! wait has passed, and has no reminder yet. The wait is the caller's (a ledger row is needed, see the lane notes).
use crate::error::SmartInboxError;
use crate::load::open_reader;
use rusqlite::params;
use store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueReminder {
    pub message_id: String,
    pub thread_id: String,
    pub account_id: String,
    pub subject: String,
    pub sent_at_ms: i64,
}

pub struct RemindIfNoReply<'a> {
    pub store: &'a Store,
}

impl RemindIfNoReply<'_> {
    pub fn execute(&self, now_ms: i64, wait_ms: i64) -> Result<Vec<DueReminder>, SmartInboxError> {
        let conn = open_reader(self.store.path())?;
        let mut st = conn.prepare(
            "SELECT m.id, m.thread_id, m.account_id, m.subject, m.received_at_ms FROM message m \
             WHERE lower(m.from_address) IN (SELECT lower(address) FROM account) AND m.folder <> 'INBOX' AND m.received_at_ms <= ?1 - ?2 \
               AND NOT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = m.thread_id AND (n.received_at_ms > m.received_at_ms OR (n.received_at_ms = m.received_at_ms AND n.id > m.id))) \
               AND NOT EXISTS (SELECT 1 FROM reminder r WHERE r.message_id = m.id) \
             ORDER BY m.received_at_ms, m.id",
        )?;
        let rows = st.query_map(params![now_ms, wait_ms], |r| {
            Ok(DueReminder { message_id: r.get(0)?, thread_id: r.get(1)?, account_id: r.get(2)?, subject: r.get(3)?, sent_at_ms: r.get(4)? })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}
