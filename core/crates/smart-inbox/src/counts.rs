//! Counts per tab and per account, and the waiting threads. The definitions are the contract's
//! (contracts::UNREAD_DEFINITION, WAITING_DEFINITION) and the store's; `tests/` holds these sums equal to `Store::recount`.
use crate::load::load_tabs;
use rusqlite::{params, Connection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTabCounts {
    pub account_id: String,
    pub tab_id: String,
    pub unread: u32,
    pub waiting: u32,
}

/// waiting = the thread's newest message (any folder) is inbound, sits in the INBOX, and is neither archived nor snoozed.
const WAITING: &str = "lower(m.from_address) NOT IN (SELECT lower(address) FROM account) \
    AND NOT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = m.thread_id \
        AND (n.received_at_ms > m.received_at_ms OR (n.received_at_ms = m.received_at_ms AND n.id > m.id)))";
const VISIBLE: &str = "m.archived = 0 AND m.folder = 'INBOX' AND NOT EXISTS (SELECT 1 FROM snooze s WHERE s.message_id = m.id)";

/// Unread and waiting per (account, tab). Add the rows of one tab and you get the tab's count in the counts stream;
/// add the rows of one account and you get the account's.
pub fn account_counts(conn: &Connection) -> rusqlite::Result<Vec<AccountTabCounts>> {
    let primary = load_tabs(conn)?.primary();
    let sql = format!(
        "SELECT m.account_id, COALESCE(mc.tab_id, ?1), SUM(CASE WHEN m.read = 0 THEN 1 ELSE 0 END), SUM(CASE WHEN {WAITING} THEN 1 ELSE 0 END) \
         FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id WHERE {VISIBLE} GROUP BY m.account_id, COALESCE(mc.tab_id, ?1) ORDER BY 1, 2"
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(params![primary], |r| {
        Ok(AccountTabCounts { account_id: r.get(0)?, tab_id: r.get(1)?, unread: r.get::<_, i64>(2)?.max(0) as u32, waiting: r.get::<_, i64>(3)?.max(0) as u32 })
    })?;
    rows.collect()
}

/// The threads whose newest message is inbound and unanswered (one id per thread).
pub fn awaiting_reply_threads(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let sql = format!("SELECT m.thread_id FROM message m WHERE {VISIBLE} AND {WAITING} GROUP BY m.thread_id ORDER BY m.thread_id");
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect()
}
