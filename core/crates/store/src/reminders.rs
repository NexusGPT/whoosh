//! Contracts delta 18, remind if no reply: the store's facts about a reply reminder (the contract's `reply_reminder_state` turns them
//! into waiting, fired or answered), the rows of the Reminders view, the dismissal, and the one write at the firing.
//!
//! A reminder is a `reminder` row with `if_no_reply = 1` riding the message the owner sent (`reply_reminder_on_sent`), or, while the
//! message is still a draft, the draft's `remind_if_no_reply_at_ms`. A reply is a message of the same thread, from an address that is
//! none of the owner's accounts, stored after the owner's message went out. The state is DERIVED at read; only `fired_at_ms` is written,
//! once, so the notification fires once. One reminder per thread: the newest set (`updated_at_ms`, then id) is the thread's.
use crate::error::StoreError;
use crate::Store;
use rusqlite::{params, Connection, OptionalExtension};

/// The reply of a reminder's thread that decides it: the earliest message of someone else after the owner's message went out.
const FIRST_REPLY: &str = "(SELECT min(n.received_at_ms) FROM message n WHERE n.thread_id = s.thread_id AND n.id <> s.id \
     AND n.received_at_ms > s.received_at_ms AND lower(n.from_address) NOT IN (SELECT lower(address) FROM account))";

/// The reminder that counts for each thread: no newer one rides the same thread.
const NEWEST_OF_THREAD: &str = "NOT EXISTS (SELECT 1 FROM reminder r2 JOIN message s2 ON s2.id = r2.message_id WHERE r2.if_no_reply = 1 \
     AND s2.thread_id = s.thread_id AND (r2.updated_at_ms > r.updated_at_ms OR (r2.updated_at_ms = r.updated_at_ms AND r2.id > r.id)))";

/// What the store knows about one thread's reminder, for `reply_reminder_state(at_ms, first_reply_ms, now_ms)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReminderFacts {
    /// The id `ClearReplyReminder` takes: the draft's id while it is a draft, the sent message's id once it went out.
    pub key: String,
    pub at_ms: i64,
    pub first_reply_ms: Option<i64>,
    /// The reminder is still on a draft: it has no thread history yet and cannot have fired.
    pub on_draft: bool,
}

fn facts_of_id(c: &Connection, id: &str) -> Result<Option<ReminderFacts>, StoreError> {
    // 1. a reminder row on the thread (the id is the thread, a message of it, the sent message, or the row's own id)
    let on_sent = c
        .query_row(
            &format!(
                "SELECT s.id, r.when_ms, {FIRST_REPLY} FROM reminder r JOIN message s ON s.id = r.message_id \
                 WHERE r.if_no_reply = 1 AND {NEWEST_OF_THREAD} \
                   AND (s.thread_id = ?1 OR s.id = ?1 OR r.id = ?1 OR s.thread_id = (SELECT thread_id FROM message WHERE id = ?1)) \
                 ORDER BY r.updated_at_ms DESC, r.id DESC LIMIT 1"
            ),
            params![id],
            |r| Ok(ReminderFacts { key: r.get(0)?, at_ms: r.get(1)?, first_reply_ms: r.get(2)?, on_draft: false }),
        )
        .optional()?;
    if on_sent.is_some() {
        return Ok(on_sent);
    }
    // 2. a draft still waiting to be sent (the id is the draft, or the thread it answers)
    Ok(c.query_row(
        "SELECT d.id, d.remind_if_no_reply_at_ms FROM draft d WHERE d.remind_if_no_reply_at_ms IS NOT NULL \
           AND (d.id = ?1 OR d.reply_to_thread_id = ?1 OR d.reply_to_thread_id = (SELECT thread_id FROM message WHERE id = ?1)) \
         ORDER BY d.updated_at_ms DESC, d.id DESC LIMIT 1",
        params![id],
        |r| Ok(ReminderFacts { key: r.get(0)?, at_ms: r.get(1)?, first_reply_ms: None, on_draft: true }),
    )
    .optional()?)
}

/// Ids of the sent messages whose reminder is FIRED by the contract's rule: its time came and no reply arrived by then (a reply after
/// the time does not unfire it). `AND ?` scopes it to an account set; newest firing time first.
pub(crate) fn fired_message_ids(c: &Connection, now_ms: i64, scope_sql: &str, limit: usize) -> Result<Vec<String>, StoreError> {
    let sql = format!(
        "SELECT s.id FROM reminder r JOIN message s ON s.id = r.message_id \
         WHERE r.if_no_reply = 1 AND r.when_ms <= ?1 AND {NEWEST_OF_THREAD} \
           AND NOT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = s.thread_id AND n.id <> s.id AND n.received_at_ms > s.received_at_ms \
                           AND n.received_at_ms <= r.when_ms AND lower(n.from_address) NOT IN (SELECT lower(address) FROM account)){scope_sql} \
         ORDER BY r.when_ms DESC, r.id LIMIT {}",
        limit as i64
    );
    let mut st = c.prepare(&sql)?;
    let rows = st.query_map(params![now_ms], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

impl Store {
    /// The reminder of a thread, read from any id of it (see `ReminderFacts`). None = no reminder is set.
    pub fn reply_reminder_facts(&self, id: &str) -> Result<Option<ReminderFacts>, StoreError> {
        let id = id.to_string();
        self.read(move |c| facts_of_id(c, &id))
    }

    /// Dismisses a thread's reminder (a waiting, a fired or an answered one): every reply reminder riding the thread's messages goes, and
    /// the reminder a draft of the thread still holds is cleared. Returns how many reminders were removed or cleared.
    pub fn reply_reminder_clear(&self, id: &str) -> Result<usize, StoreError> {
        let id = id.to_string();
        self.write(move |ctx| {
            let thread: Option<String> = ctx.tx.query_row("SELECT thread_id FROM message WHERE id = ?1", params![id], |r| r.get(0)).optional()?;
            let by_row: Option<String> = ctx
                .tx
                .query_row("SELECT s.thread_id FROM reminder r JOIN message s ON s.id = r.message_id WHERE r.id = ?1", params![id], |r| r.get(0))
                .optional()?;
            let thread = thread.or(by_row).unwrap_or_else(|| id.clone());
            let removed = ctx.tx.execute(
                "DELETE FROM reminder WHERE if_no_reply = 1 AND message_id IN (SELECT id FROM message WHERE thread_id = ?1)",
                params![thread],
            )?;
            let cleared = ctx.tx.execute(
                "UPDATE draft SET remind_if_no_reply_at_ms = NULL WHERE remind_if_no_reply_at_ms IS NOT NULL AND (id = ?1 OR reply_to_thread_id = ?2)",
                params![id, thread],
            )?;
            Ok(removed + cleared)
        })
    }

    /// The write at the firing: every reminder whose time came, with no reply by then, and not yet marked, gets `fired_at_ms` now. Returns
    /// the sent message ids it marked, so the notification goes out once. A reminder a reply answered is left unmarked (it never fires).
    pub fn reply_reminders_mark_fired(&self, now_ms: i64) -> Result<Vec<String>, StoreError> {
        self.write(move |ctx| {
            let due: Vec<String> = {
                let mut st = ctx.tx.prepare(&format!(
                    "SELECT s.id FROM reminder r JOIN message s ON s.id = r.message_id \
                     WHERE r.if_no_reply = 1 AND r.fired_at_ms IS NULL AND r.when_ms <= ?1 AND {NEWEST_OF_THREAD} \
                       AND NOT EXISTS (SELECT 1 FROM message n WHERE n.thread_id = s.thread_id AND n.id <> s.id AND n.received_at_ms > s.received_at_ms \
                                       AND n.received_at_ms <= r.when_ms AND lower(n.from_address) NOT IN (SELECT lower(address) FROM account)) \
                     ORDER BY r.when_ms, r.id"
                ))?;
                let rows = st.query_map(params![now_ms], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            let mut mark = ctx.tx.prepare_cached("UPDATE reminder SET fired_at_ms = ?2 WHERE message_id = ?1 AND if_no_reply = 1 AND fired_at_ms IS NULL")?;
            for id in &due {
                mark.execute(params![id, now_ms])?;
            }
            Ok(due)
        })
    }
}
