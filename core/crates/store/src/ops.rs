//! The typed writes the other lanes call. Each one declares the messages it changes, so the counts move by
//! a difference and the events name the right tabs. A write that changes nothing (the flag already had that
//! value) touches no row and publishes nothing.
use crate::error::StoreError;
use crate::Store;
use contracts::Message;
use rusqlite::params;

/// A message as sync hands it over: the contract's `Message` plus the columns the schema keeps beyond it.
#[derive(Debug, Clone)]
pub struct NewMessage {
    pub message: Message,
    pub remote_id: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub has_list_header: bool,
    pub size_bytes: Option<i64>,
    /// The tab the placement decided; None leaves the message uncategorised (it sits in the first tab).
    pub tab_id: Option<String>,
}

/// A JSON array of strings, escaped. The schema checks `json_valid` on these columns.
pub fn json_strings(items: &[String]) -> String {
    format!("[{}]", items.iter().map(|s| json_string(s)).collect::<Vec<_>>().join(","))
}

/// One JSON string literal, escaped.
pub fn json_string(item: &str) -> String {
    let mut out = String::from("\"");
    for c in item.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl Store {
    pub fn set_read(&self, ids: Vec<String>, read: bool) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            ctx.will_change(&ids)?;
            let mut st = ctx.tx.prepare_cached("UPDATE message SET read = ?1 WHERE id = ?2 AND read <> ?1")?;
            let mut changed = 0;
            for id in &ids {
                changed += st.execute(params![read as i64, id])?;
            }
            Ok(changed)
        })
    }

    pub fn set_archived(&self, ids: Vec<String>, archived: bool) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            ctx.will_change(&ids)?;
            let mut st = ctx.tx.prepare_cached("UPDATE message SET archived = ?1 WHERE id = ?2 AND archived <> ?1")?;
            let mut changed = 0;
            for id in &ids {
                changed += st.execute(params![archived as i64, id])?;
            }
            Ok(changed)
        })
    }

    pub fn set_pinned(&self, message_id: String, pinned: bool, now_ms: i64) -> Result<(), StoreError> {
        self.write(move |ctx| {
            ctx.will_change(std::slice::from_ref(&message_id))?;
            if pinned {
                ctx.tx.execute("INSERT OR IGNORE INTO pin (message_id, pinned_at_ms) VALUES (?1, ?2)", params![message_id, now_ms])?;
            } else {
                ctx.tx.execute("DELETE FROM pin WHERE message_id = ?1", params![message_id])?;
            }
            Ok(())
        })
    }

    /// A snooze always has a return time (the schema refuses one without).
    pub fn snooze(&self, ids: Vec<String>, return_at_ms: i64, now_ms: i64) -> Result<(), StoreError> {
        self.write(move |ctx| {
            ctx.will_change(&ids)?;
            let mut st = ctx.tx.prepare_cached(
                "INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(message_id) DO UPDATE SET return_at_ms = excluded.return_at_ms, updated_at_ms = excluded.updated_at_ms \
                 WHERE snooze.return_at_ms <> excluded.return_at_ms",
            )?;
            for id in &ids {
                st.execute(params![id, return_at_ms, now_ms])?;
            }
            Ok(())
        })
    }

    pub fn unsnooze(&self, ids: Vec<String>) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            ctx.will_change(&ids)?;
            let mut st = ctx.tx.prepare_cached("DELETE FROM snooze WHERE message_id = ?1")?;
            let mut removed = 0;
            for id in &ids {
                removed += st.execute(params![id])?;
            }
            Ok(removed)
        })
    }

    /// Sync's insert: creates the thread when it is new, keeps its last-message time, and places the message.
    /// A message with the same id is left as it is. Messages that were the newest of their thread are declared too,
    /// because a newer message ends their "waiting".
    pub fn insert_messages(&self, batch: Vec<NewMessage>) -> Result<usize, StoreError> {
        self.write(move |ctx| ctx.insert_batch(&batch))
    }
}

impl crate::WriteCtx<'_> {
    /// The body of `Store::insert_messages`, usable inside a larger transaction (a page and its cursor, `page.rs`).
    pub fn insert_batch(&mut self, batch: &[NewMessage]) -> Result<usize, StoreError> {
        let ctx = self;
        let mut declare: Vec<String> = batch.iter().map(|n| n.message.id.clone()).collect();
        {
            let mut latest = ctx.tx.prepare_cached("SELECT id FROM message WHERE thread_id = ?1 ORDER BY received_at_ms DESC, id DESC LIMIT 1")?;
            for n in batch {
                if let Ok(id) = latest.query_row([&n.message.thread_id], |r| r.get::<_, String>(0)) {
                    declare.push(id);
                }
            }
        }
        ctx.will_change(&declare)?;
        let mut inserted = 0;
        for n in batch {
            let m = &n.message;
            ctx.tx.execute(
                "INSERT OR IGNORE INTO thread (id, account_id, subject, participants_json, last_message_at_ms) VALUES (?1, ?2, ?3, '[]', ?4)",
                params![m.thread_id, m.account_id, m.subject, m.received_at_ms],
            )?;
            ctx.tx.execute(
                "UPDATE thread SET last_message_at_ms = ?2 WHERE id = ?1 AND (last_message_at_ms IS NULL OR last_message_at_ms < ?2)",
                params![m.thread_id, m.received_at_ms],
            )?;
            let added = ctx.tx.execute(
                "INSERT OR IGNORE INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, to_json, cc_json, subject, preview, \
                    body_text, body_html, received_at_ms, read, archived, folder, has_list_header, size_bytes, login_code, trackers_blocked, bcc_json) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                params![
                    m.id, m.thread_id, m.account_id, n.remote_id, m.sender_name, m.from_address, json_strings(&m.to), json_strings(&m.cc),
                    m.subject, m.preview, n.body_text, n.body_html, m.received_at_ms, m.read as i64, m.archived as i64, m.folder,
                    n.has_list_header as i64, n.size_bytes, m.login_code, m.trackers_blocked as i64, json_strings(&m.bcc)
                ],
            )?;
            inserted += added;
            if added == 1 {
                ctx.apply_muted_arrival(&m.id, &m.thread_id)?;
            }
            if !m.list_unsubscribe.is_empty() {
                // contracts delta 10: the List-Unsubscribe header the sync adapter read (0005's columns)
                ctx.set_list_unsubscribe(&m.id, &m.list_unsubscribe, m.list_unsubscribe_one_click)?;
            }
            if let Some(tab) = &n.tab_id {
                ctx.tx.execute(
                    "INSERT OR IGNORE INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms) \
                     VALUES (?1, ?2, 'default', NULL, NULL, ?3, ?3)",
                    params![m.id, tab, m.received_at_ms],
                )?;
            }
            ctx.thread_changed(&m.thread_id);
        }
        Ok(inserted)
    }
}

impl crate::WriteCtx<'_> {
    /// Contracts delta 18, the muted-arrival rule: a message that ARRIVES (a new row) in a thread that holds another muted message is
    /// read at once and muted itself, so it is out of every inbox list and count and sits in the Muted view. The contract's
    /// `arrival_in_thread` is the one rule; this applies it inside the insert's own transaction, so no reader sees the message unmuted.
    pub(crate) fn apply_muted_arrival(&mut self, message_id: &str, thread_id: &str) -> Result<(), StoreError> {
        let thread_muted: bool = self.tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM message o WHERE o.thread_id = ?1 AND o.id <> ?2 AND o.muted = 1)",
            params![thread_id, message_id],
            |r| r.get(0),
        )?;
        let treatment = contracts::arrival_in_thread(thread_muted);
        if !treatment.in_inbox {
            self.tx.execute("UPDATE message SET muted = 1 WHERE id = ?1", params![message_id])?;
        }
        if treatment.mark_read {
            self.tx.execute("UPDATE message SET read = 1 WHERE id = ?1", params![message_id])?;
        }
        Ok(())
    }
}

impl crate::WriteCtx<'_> {
    /// When the message was pinned, if it is.
    pub fn pinned_at(&self, message_id: &str) -> Result<Option<i64>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self.tx.prepare_cached("SELECT pinned_at_ms FROM pin WHERE message_id = ?1")?.query_row(params![message_id], |r| r.get(0)).optional()?)
    }

    /// Pin at a time (replacing an earlier pin time) or unpin (`None`). Declares the message first.
    pub fn set_pinned_at(&mut self, message_id: &str, pinned_at_ms: Option<i64>) -> Result<(), StoreError> {
        self.will_change(std::slice::from_ref(&message_id.to_string()))?;
        match pinned_at_ms {
            Some(t) => self.tx.execute("INSERT OR REPLACE INTO pin (message_id, pinned_at_ms) VALUES (?1, ?2)", params![message_id, t])?,
            None => self.tx.execute("DELETE FROM pin WHERE message_id = ?1", params![message_id])?,
        };
        Ok(())
    }

    /// When the message returns from its snooze, if it is snoozed.
    pub fn snoozed_until(&self, message_id: &str) -> Result<Option<i64>, StoreError> {
        use rusqlite::OptionalExtension;
        Ok(self.tx.prepare_cached("SELECT return_at_ms FROM snooze WHERE message_id = ?1")?.query_row(params![message_id], |r| r.get(0)).optional()?)
    }

    /// Snooze until a time (replacing an earlier one) or lift the snooze (`None`). Declares the message first.
    pub fn set_snoozed_until(&mut self, message_id: &str, return_at_ms: Option<i64>, now_ms: i64) -> Result<(), StoreError> {
        self.will_change(std::slice::from_ref(&message_id.to_string()))?;
        match return_at_ms {
            Some(t) => self.tx.execute(
                "INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(message_id) DO UPDATE SET return_at_ms = excluded.return_at_ms, updated_at_ms = excluded.updated_at_ms",
                params![message_id, t, now_ms],
            )?,
            None => self.tx.execute("DELETE FROM snooze WHERE message_id = ?1", params![message_id])?,
        };
        Ok(())
    }

    /// Whether the message carried a List-Unsubscribe style header (a mailing-list message). False for an unknown message.
    pub fn has_list_header(&self, message_id: &str) -> Result<bool, StoreError> {
        use rusqlite::OptionalExtension;
        let flag: Option<i64> = self.tx.prepare_cached("SELECT has_list_header FROM message WHERE id = ?1")?.query_row(params![message_id], |r| r.get(0)).optional()?;
        Ok(flag.unwrap_or(0) != 0)
    }

    /// A new draft that repeats a message the owner sent: same recipients, subject and text, from the same account, same
    /// attachments. Only a message from one of the owner's own addresses can be sent again.
    pub fn draft_from_sent(&mut self, message_id: &str, draft_id: &str, now_ms: i64) -> Result<String, StoreError> {
        use rusqlite::OptionalExtension;
        let row = self
            .tx
            .prepare_cached(
                "SELECT m.account_id, m.to_json, m.cc_json, m.subject, coalesce(m.body_text, m.preview), m.from_address, a.address \
                 FROM message m JOIN account a ON a.id = m.account_id WHERE m.id = ?1",
            )?
            .query_row(params![message_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?)))
            .optional()?;
        let Some((account, to_json, cc_json, subject, body, from, own)) = row else {
            return Err(StoreError::Invalid(format!("the message {message_id} has no account to send from")));
        };
        if !from.eq_ignore_ascii_case(&own) {
            return Err(StoreError::Invalid("only a message you sent can be sent again".into()));
        }
        let attachments: Vec<String> = {
            let mut st = self.tx.prepare_cached("SELECT id FROM attachment WHERE message_id = ?1 ORDER BY id")?;
            let rows = st.query_map(params![message_id], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        self.tx.execute(
            "INSERT INTO draft (id, from_account_id, to_json, cc_json, subject, body, attachment_ids_json, ai_marks_json, reply_to_thread_id, updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '[]', NULL, ?8)",
            params![draft_id, account, to_json, cc_json, subject, body, json_strings(&attachments), now_ms],
        )?;
        Ok(draft_id.to_string())
    }
}
