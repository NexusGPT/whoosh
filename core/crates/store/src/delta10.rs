//! contracts delta 10, hook (migration 0005): the store side of RemoveLabel, RemoveAllLabels, DeleteMessages, star and
//! mute as stored state, the List-Unsubscribe header, remind-if-no-reply, the attachments read, and the attachment index's
//! `AttachmentRows` door (its rows go through the store's writer, so the change bus sees them).
use crate::writer::WriteCtx;
use crate::{Store, StoreError};
use attachment_index::{AttachmentIndexEntry, AttachmentKind, AttachmentRows, CatalogRow, IndexState};
use contracts::{AttachmentContent, AttachmentFile, CoreError, LabelAssignment};
use rusqlite::{params, Connection, OptionalExtension};

fn json_strings(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|s| {
            let mut q = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => q.push_str("\\\""),
                    '\\' => q.push_str("\\\\"),
                    c if (c as u32) < 0x20 => q.push(' '),
                    c => q.push(c),
                }
            }
            q.push('"');
            q
        })
        .collect();
    format!("[{}]", quoted.join(","))
}

fn core_err(e: StoreError) -> CoreError { CoreError::Unexpected { reason_words: format!("the attachment index could not reach the store: {e}") } }

impl WriteCtx<'_> {
    /// Whether the owner took this label off the message's thread (0006).
    pub fn label_removed_by_user(&self, label_id: &str, message_id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM message m JOIN thread t ON t.id = m.thread_id, json_each(t.labels_removed_json) j WHERE m.id = ?1 AND j.value = ?2)",
            params![message_id, label_id],
            |r| r.get::<_, bool>(0),
        )?)
    }

    /// Records that the owner took this label off the message's thread (RemoveLabel, RemoveAllLabels; 0006).
    pub fn label_removal_record(&mut self, label_id: &str, message_id: &str) -> Result<(), StoreError> {
        self.tx.execute(
            "UPDATE thread SET labels_removed_json = json_insert(labels_removed_json, '$[#]', ?2)
             WHERE id = (SELECT thread_id FROM message WHERE id = ?1)
               AND NOT EXISTS (SELECT 1 FROM json_each(labels_removed_json) WHERE value = ?2)",
            params![message_id, label_id],
        )?;
        Ok(())
    }

    /// The owner labelled the thread again: their earlier removal of that label no longer holds.
    pub fn label_removal_lift(&mut self, label_id: &str, message_id: &str) -> Result<(), StoreError> {
        self.tx.execute(
            "UPDATE thread SET labels_removed_json = (SELECT json_group_array(value) FROM json_each(labels_removed_json) WHERE value <> ?2)
             WHERE id = (SELECT thread_id FROM message WHERE id = ?1)",
            params![message_id, label_id],
        )?;
        Ok(())
    }

    /// RemoveLabel in the real core: the label leaves these messages and the removal is recorded on their threads.
    pub fn label_remove_by_user(&mut self, label_id: &str, message_ids: &[String]) -> Result<Vec<String>, StoreError> {
        self.will_change(message_ids)?;
        let mut taken = Vec::new();
        for id in message_ids {
            if self.tx.execute("DELETE FROM label_assignment WHERE label_id = ?1 AND message_id = ?2", params![label_id, id])? == 1 {
                taken.push(id.clone());
                self.builtin_tab_drop(label_id, id)?;
            }
            self.label_removal_record(label_id, id)?;
        }
        Ok(taken)
    }

    /// Takes every label off these messages; returns what was removed (for the one undo entry).
    pub fn labels_remove_all(&mut self, message_ids: &[String]) -> Result<Vec<LabelAssignment>, StoreError> {
        self.will_change(message_ids)?;
        let mut removed = Vec::new();
        {
            // fix-labelstreams: the page entry that decided each filing is kept (it was always None here)
            let mut find = self.tx.prepare_cached("SELECT label_id, decided_entry_id FROM label_assignment WHERE message_id = ?1 ORDER BY label_id")?;
            for id in message_ids {
                for l in find.query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))? {
                    let (label_id, decided_entry_id) = l?;
                    removed.push(LabelAssignment { label_id, message_id: id.clone(), decided_entry_id });
                }
            }
        }
        {
            let mut del = self.tx.prepare_cached("DELETE FROM label_assignment WHERE message_id = ?1")?;
            for id in message_ids {
                del.execute(params![id])?;
            }
        }
        for a in &removed {
            self.builtin_tab_drop(&a.label_id, &a.message_id)?;
            self.label_removal_record(&a.label_id, &a.message_id)?;   // delta 11: each removal is the owner's
        }
        Ok(removed)
    }

    /// fix-labelstreams: who decided each filing these messages hold, as a JSON array (label, message, by, rule, prob, at, card, chain, entry),
    /// read BEFORE `labels_remove_all` deletes the rows. The undo entry keeps it beside the contract's list (which holds the page entry only), so
    /// Z can put the whole decider back. One indexed walk of label_assignment_by_message per message.
    pub fn labels_deciders(&self, message_ids: &[String]) -> Result<String, StoreError> {
        let mut st = self.tx.prepare_cached(
            "SELECT COALESCE(json_group_array(json_object('label', label_id, 'message', message_id, 'by', decided_by, 'rule', rule_id, 'prob', probability, \
                    'at', decided_at_ms, 'card', flow_card, 'chain', flow_chain_id, 'entry', decided_entry_id)), '[]') \
             FROM label_assignment WHERE message_id = ?1",
        )?;
        let mut all = Vec::new();
        for id in message_ids {
            let one: String = st.query_row(params![id], |r| r.get(0))?;
            if one != "[]" {
                all.push(one[1..one.len() - 1].to_string());
            }
        }
        Ok(format!("[{}]", all.join(",")))
    }

    /// Puts removed assignments back (Z after RemoveAllLabels); they come back filed by the owner.
    pub fn labels_restore(&mut self, assignments: &[LabelAssignment], now_ms: i64) -> Result<usize, StoreError> {
        let ids: Vec<String> = assignments.iter().map(|a| a.message_id.clone()).collect();
        self.will_change(&ids)?;
        let mut st = self.tx.prepare_cached("INSERT OR IGNORE INTO label_assignment (label_id, message_id, updated_at_ms) VALUES (?1, ?2, ?3)")?;
        let mut n = 0;
        for a in assignments {
            n += st.execute(params![a.label_id, a.message_id, now_ms])?;
        }
        Ok(n)
    }

    pub fn set_starred(&mut self, message_ids: &[String], on: bool) -> Result<usize, StoreError> { self.set_flag("starred", message_ids, on) }
    pub fn set_muted(&mut self, message_ids: &[String], on: bool) -> Result<usize, StoreError> { self.set_flag("muted", message_ids, on) }

    fn set_flag(&mut self, column: &'static str, message_ids: &[String], on: bool) -> Result<usize, StoreError> {
        self.will_change(message_ids)?;
        let mut st = self.tx.prepare_cached(&format!("UPDATE message SET {column} = ?2 WHERE id = ?1"))?;
        let mut n = 0;
        for id in message_ids {
            n += st.execute(params![id, on as i64])?;
        }
        Ok(n)
    }

    /// Records a message's List-Unsubscribe URLs (header order) and whether one-click was offered. Empty clears it.
    pub fn set_list_unsubscribe(&mut self, message_id: &str, urls: &[String], one_click: bool) -> Result<(), StoreError> {
        let json = (!urls.is_empty()).then(|| json_strings(urls));
        self.tx.execute("UPDATE message SET list_unsubscribe_json = ?2, list_unsubscribe_one_click = ?3 WHERE id = ?1", params![message_id, json, one_click as i64])?;
        Ok(())
    }

    pub fn mark_unsubscribed(&mut self, message_id: &str, at_ms: i64) -> Result<bool, StoreError> {
        Ok(self.tx.execute("UPDATE message SET unsubscribed_at_ms = ?2 WHERE id = ?1 AND list_unsubscribe_json IS NOT NULL", params![message_id, at_ms])? == 1)
    }

    /// RemindIfNoReply on a draft: refused in words for a past time or an unknown draft.
    pub fn draft_remind_if_no_reply(&mut self, draft_id: &str, at_ms: i64, now_ms: i64) -> Result<(), StoreError> {
        if at_ms <= now_ms {
            return Err(StoreError::Invalid("that time has already passed".into()));
        }
        match self.tx.execute("UPDATE draft SET remind_if_no_reply_at_ms = ?2 WHERE id = ?1", params![draft_id, at_ms])? {
            1 => Ok(()),
            _ => Err(StoreError::Invalid(format!("no draft {draft_id}"))),
        }
    }

    /// At send: the draft's reminder moves onto the sent message (a `reminder` row with if_no_reply). Returns its id.
    pub fn reply_reminder_on_sent(&mut self, draft_id: &str, sent_message_id: &str, now_ms: i64) -> Result<Option<String>, StoreError> {
        let at: Option<i64> = self.tx.query_row("SELECT remind_if_no_reply_at_ms FROM draft WHERE id = ?1", params![draft_id], |r| r.get(0)).optional()?.flatten();
        // the draft left the draft list when it was queued: the time then rides the outbox entry's envelope
        let at = match at { Some(a) => Some(a), None => self.outbox_reply_reminder(draft_id)? };
        let Some(at) = at else { return Ok(None) };
        let id = format!("rr-{sent_message_id}");
        self.tx.execute(
            "INSERT OR REPLACE INTO reminder (id, message_id, when_ms, note, updated_at_ms, if_no_reply) VALUES (?1, ?2, ?3, '', ?4, 1)",
            params![id, sent_message_id, at, now_ms],
        )?;
        Ok(Some(id))
    }

    /// Fires every due reply reminder whose thread received no reply after the sent message; returns the sent message ids.
    /// A reminder whose thread got a reply is closed without firing.
    pub fn reply_reminders_fire(&mut self, now_ms: i64) -> Result<Vec<String>, StoreError> {
        let due: Vec<(String, String, i64, String)> = {
            let mut st = self.tx.prepare_cached(
                "SELECT r.id, r.message_id, m.received_at_ms, m.thread_id FROM reminder r JOIN message m ON m.id = r.message_id
                 WHERE r.if_no_reply = 1 AND r.fired_at_ms IS NULL AND r.when_ms <= ?1 ORDER BY r.when_ms, r.id",
            )?;
            let rows = st.query_map(params![now_ms], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut fired = Vec::new();
        for (rid, mid, sent_at, thread) in due {
            let replied: bool = self.tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM message WHERE thread_id = ?1 AND id <> ?2 AND received_at_ms > ?3 AND folder <> 'Sent')",
                params![thread, mid, sent_at],
                |r| r.get(0),
            )?;
            self.tx.execute("UPDATE reminder SET fired_at_ms = ?2 WHERE id = ?1", params![rid, now_ms])?;
            if !replied {
                fired.push(mid);
            }
        }
        Ok(fired)
    }

    /// Engine-actions: Mute and Unmute on the THREADS of these messages (a muted thread leaves every inbox list and count and stays out;
    /// the next message that arrives in it arrives read and muted, `apply_muted_arrival`). Returns every message of those threads.
    pub fn mute_threads(&mut self, message_ids: &[String], on: bool) -> Result<Vec<String>, StoreError> {
        let members = self.thread_member_ids(message_ids)?;
        self.set_muted(&members, on)?;
        Ok(members)
    }

    /// Engine-actions: Mark not done (Unarchive). A message the owner archived by the flag comes back at once (`archived` = 0). A message
    /// that sits in one of the account's Archive folders (`archive_folders`: the pairs the server's roles name) goes back to its INBOX:
    /// its `folder` is INBOX and its `remote_id` is cleared, as `move_to_account` does, because the UID belongs to the Archive folder and
    /// would collide with an INBOX UID. Returns the ids that changed. The server's copy is NOT moved here: the real core has no write-back
    /// to a server yet (docs/lanes/engine-actions.md).
    pub fn unarchive(&mut self, message_ids: &[String], archive_folders: &[(String, String)]) -> Result<Vec<String>, StoreError> {
        self.will_change(message_ids)?;
        let mut changed = Vec::new();
        for id in message_ids {
            let row: Option<(String, String, i64)> = self.tx
                .query_row("SELECT account_id, folder, archived FROM message WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()?;
            let Some((account, folder, archived)) = row else { continue };
            let in_archive_folder = archive_folders.iter().any(|(a, f)| *a == account && *f == folder);
            if archived == 0 && !in_archive_folder { continue; }
            if in_archive_folder {
                self.tx.execute("UPDATE message SET archived = 0, folder = 'INBOX', remote_id = NULL WHERE id = ?1", params![id])?;
            } else {
                self.tx.execute("UPDATE message SET archived = 0 WHERE id = ?1", params![id])?;
            }
            changed.push(id.clone());
        }
        Ok(changed)
    }

    /// Engine-actions: RemindIfNoReply on a draft or an outbox entry; refused (Invalid) for a past time. A thread holds one reminder, so
    /// setting a draft's replaces the one another draft of the same reply thread held. A draft is the `draft` row's column; an outbox entry
    /// (the draft left the draft list when it was queued) keeps the time in its envelope as `remind_if_no_reply_at_ms`. Returns false
    /// when neither exists.
    pub fn remind_if_no_reply(&mut self, id: &str, at_ms: i64, now_ms: i64) -> Result<bool, StoreError> {
        if at_ms <= now_ms { return Err(StoreError::Invalid("that time has already passed".into())); }
        if self.tx.execute("UPDATE draft SET remind_if_no_reply_at_ms = ?2 WHERE id = ?1", params![id, at_ms])? == 1 {
            self.tx.execute(
                "UPDATE draft SET remind_if_no_reply_at_ms = NULL WHERE id <> ?1 AND remind_if_no_reply_at_ms IS NOT NULL
                   AND reply_to_thread_id IS NOT NULL AND reply_to_thread_id = (SELECT reply_to_thread_id FROM draft WHERE id = ?1)",
                params![id],
            )?;
            return Ok(true);
        }
        Ok(self.tx.execute(
            "UPDATE outbox_entry SET draft_json = json_set(draft_json, '$.remind_if_no_reply_at_ms', ?2)
             WHERE state <> 'cancelled' AND (id = ?1 OR json_extract(draft_json, '$.draft.id') = ?1)",
            params![id, at_ms],
        )? > 0)
    }

    /// The time an outbox entry holds for its reply reminder (set while the message waits to leave), None when it holds none.
    pub fn outbox_reply_reminder(&self, id: &str) -> Result<Option<i64>, StoreError> {
        Ok(self.tx.query_row(
            "SELECT json_extract(draft_json, '$.remind_if_no_reply_at_ms') FROM outbox_entry
             WHERE state <> 'cancelled' AND (id = ?1 OR json_extract(draft_json, '$.draft.id') = ?1)
               AND json_extract(draft_json, '$.remind_if_no_reply_at_ms') IS NOT NULL LIMIT 1",
            params![id], |r| r.get::<_, Option<i64>>(0),
        ).optional()?.flatten())
    }

    /// Takes the reminder off an outbox entry (ClearReplyReminder before the message left). Returns how many entries held one.
    pub fn outbox_reply_reminder_clear(&mut self, id: &str) -> Result<usize, StoreError> {
        Ok(self.tx.execute(
            "UPDATE outbox_entry SET draft_json = json_remove(draft_json, '$.remind_if_no_reply_at_ms')
             WHERE json_extract(draft_json, '$.remind_if_no_reply_at_ms') IS NOT NULL AND (id = ?1 OR json_extract(draft_json, '$.draft.id') = ?1)",
            params![id],
        )?)
    }

    /// Files one attachment index entry (attachment_content + attachment.extracted_text) through the writer.
    pub fn attachment_index_save(&mut self, e: &AttachmentIndexEntry) -> Result<(), StoreError> {
        let message: Option<String> = self.tx.query_row("SELECT message_id FROM attachment WHERE id = ?1", params![e.attachment_id], |r| r.get(0)).optional()?;
        if let Some(m) = &message {
            self.will_change(std::slice::from_ref(m))?;
        }
        if e.state != IndexState::Indexed {
            self.tx.execute("DELETE FROM attachment_content WHERE attachment_id = ?1", params![e.attachment_id])?;
            self.tx.execute("UPDATE attachment SET extracted_text = NULL WHERE id = ?1", params![e.attachment_id])?;
            return Ok(());
        }
        let c = e.to_content();
        self.tx.execute(
            "INSERT INTO attachment_content (attachment_id, text, ocr_text, labels_json, transcript, embedded, updated_at_ms, vector_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(attachment_id) DO UPDATE SET text = ?2, ocr_text = ?3, labels_json = ?4, transcript = ?5, embedded = ?6, updated_at_ms = ?7, vector_key = ?8",
            params![c.attachment_id, c.text, c.ocr_text, json_strings(&c.labels), c.transcript, c.embedded as i64, c.updated_at_ms, c.vector_key],
        )?;
        let words = e.word_text();
        self.tx.execute("UPDATE attachment SET extracted_text = ?2 WHERE id = ?1", params![e.attachment_id, if words.is_empty() { None } else { Some(words) }])?;
        Ok(())
    }
}

fn list_unsubscribe_of(conn: &Connection, message_id: &str) -> Result<(Vec<String>, bool), StoreError> {
    let one_click: Option<i64> = conn.query_row("SELECT list_unsubscribe_one_click FROM message WHERE id = ?1", params![message_id], |r| r.get(0)).optional()?;
    let mut st = conn.prepare_cached("SELECT j.value FROM message m, json_each(m.list_unsubscribe_json) j WHERE m.id = ?1 ORDER BY j.key")?;
    let urls = st.query_map(params![message_id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok((urls, one_click.unwrap_or(0) != 0))
}

fn attachments_of(conn: &Connection, message_id: &str) -> Result<Vec<AttachmentFile>, StoreError> {
    let mut st = conn.prepare_cached("SELECT id, file_name, mime_type, size_bytes, local_path, inline OR coalesce(json_extract(CASE WHEN json_valid(remote_id) THEN remote_id END, '$.inline'), 0), coalesce(content_id, json_extract(CASE WHEN json_valid(remote_id) THEN remote_id END, '$.cid')) FROM attachment WHERE message_id = ?1 ORDER BY id")?;
    let rows = st.query_map(params![message_id], |r| {
        Ok(AttachmentFile { id: r.get(0)?, message_id: message_id.to_string(), file_name: r.get(1)?, mime_type: r.get(2)?, size_bytes: r.get::<_, i64>(3)? as u64, local_path: r.get(4)?, inline: r.get::<_, i64>(5)? == 1, content_id: r.get::<_, Option<String>>(6)?.filter(|s| !s.is_empty()) })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn catalog_of(conn: &Connection, message_ids: &[String]) -> Result<Vec<CatalogRow>, StoreError> {
    let mut st = conn.prepare_cached(
        "SELECT a.message_id, a.id, a.file_name, a.mime_type, a.local_path, c.attachment_id IS NOT NULL, coalesce(a.extracted_text, '')
         FROM attachment a LEFT JOIN attachment_content c ON c.attachment_id = a.id WHERE a.message_id = ?1 ORDER BY a.id",
    )?;
    let mut out = Vec::new();
    for id in message_ids {
        let rows = st.query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, bool>(5)?, r.get::<_, String>(6)?)))?;
        for row in rows {
            let (message_id, attachment_id, file_name, mime, local, indexed, text) = row?;
            let state = if AttachmentKind::detect(&mime, &file_name) == AttachmentKind::Unreadable {
                IndexState::Unreadable
            } else if local.is_none() {
                IndexState::NotDownloaded
            } else if indexed {
                IndexState::Indexed
            } else {
                IndexState::Failed { reason_words: "not indexed yet, it will be searchable shortly".to_string() }
            };
            out.push(CatalogRow { message_id, attachment_id, file_name, state, text });
        }
    }
    Ok(out)
}

fn content_of(conn: &Connection, attachment_id: &str) -> Result<Option<AttachmentContent>, StoreError> {
    let row = conn
        .query_row(
            "SELECT text, ocr_text, transcript, embedded, updated_at_ms, vector_key FROM attachment_content WHERE attachment_id = ?1",
            params![attachment_id],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, Option<String>>(5)?)),
        )
        .optional()?;
    let Some((text, ocr_text, transcript, embedded, updated_at_ms, vector_key)) = row else { return Ok(None) };
    let mut st = conn.prepare_cached("SELECT j.value FROM attachment_content c, json_each(c.labels_json) j WHERE c.attachment_id = ?1 ORDER BY j.key")?;
    let labels = st.query_map(params![attachment_id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(Some(AttachmentContent { attachment_id: attachment_id.to_string(), text, ocr_text, labels, transcript, embedded: embedded != 0, updated_at_ms, vector_key }))
}

/// Messages whose attachments' names or read-out text hold every word (case-blind), newest first: the candidates the search
/// reads the attachment catalog for (contracts delta 11). Words are plain; a '%' or '_' in them is matched as itself.
fn attachment_candidates_of(conn: &Connection, words: &str, limit: usize) -> Result<Vec<String>, StoreError> {
    let terms: Vec<String> = words.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 2).map(|w| w.to_lowercase()).collect();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let cond: Vec<String> = (0..terms.len())
        .map(|i| format!("instr(lower(a.file_name || ' ' || coalesce(a.extracted_text, '')), ?{}) > 0", i + 1))
        .collect();
    let sql = format!(
        "SELECT a.message_id FROM attachment a JOIN message m ON m.id = a.message_id WHERE {} GROUP BY a.message_id ORDER BY max(m.received_at_ms) DESC LIMIT {}",
        cond.join(" AND "),
        limit
    );
    let mut st = conn.prepare(&sql)?;
    let rows = st.query_map(rusqlite::params_from_iter(terms.iter()), |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

impl Store {
    pub fn attachment_candidates(&self, words: &str, limit: usize) -> Result<Vec<String>, StoreError> { let w = words.to_string(); self.read(move |c| attachment_candidates_of(c, &w, limit)) }
    pub fn label_remove_by_user(&self, label_id: String, message_ids: Vec<String>) -> Result<Vec<String>, StoreError> { self.write(move |ctx| ctx.label_remove_by_user(&label_id, &message_ids)) }
    pub fn label_removed_by_user(&self, label_id: String, message_id: String) -> Result<bool, StoreError> { self.write(move |ctx| ctx.label_removed_by_user(&label_id, &message_id)) }
    pub fn labels_remove_all(&self, message_ids: Vec<String>) -> Result<Vec<LabelAssignment>, StoreError> { self.write(move |ctx| ctx.labels_remove_all(&message_ids)) }
    pub fn labels_restore(&self, assignments: Vec<LabelAssignment>, now_ms: i64) -> Result<usize, StoreError> { self.write(move |ctx| ctx.labels_restore(&assignments, now_ms)) }
    pub fn set_starred(&self, message_ids: Vec<String>, on: bool) -> Result<usize, StoreError> { self.write(move |ctx| ctx.set_starred(&message_ids, on)) }
    pub fn set_muted(&self, message_ids: Vec<String>, on: bool) -> Result<usize, StoreError> { self.write(move |ctx| ctx.set_muted(&message_ids, on)) }
    pub fn set_list_unsubscribe(&self, message_id: String, urls: Vec<String>, one_click: bool) -> Result<(), StoreError> { self.write(move |ctx| ctx.set_list_unsubscribe(&message_id, &urls, one_click)) }
    pub fn mark_unsubscribed(&self, message_id: String, at_ms: i64) -> Result<bool, StoreError> { self.write(move |ctx| ctx.mark_unsubscribed(&message_id, at_ms)) }
    pub fn mute_threads(&self, message_ids: Vec<String>, on: bool) -> Result<Vec<String>, StoreError> { self.write(move |ctx| ctx.mute_threads(&message_ids, on)) }
    pub fn unarchive(&self, message_ids: Vec<String>, archive_folders: Vec<(String, String)>) -> Result<Vec<String>, StoreError> { self.write(move |ctx| ctx.unarchive(&message_ids, &archive_folders)) }
    pub fn remind_if_no_reply(&self, id: String, at_ms: i64, now_ms: i64) -> Result<bool, StoreError> { self.write(move |ctx| ctx.remind_if_no_reply(&id, at_ms, now_ms)) }
    pub fn outbox_reply_reminder(&self, id: &str) -> Result<Option<i64>, StoreError> {
        let id = id.to_string();
        self.read(move |c| Ok(c.query_row(
            "SELECT json_extract(draft_json, '$.remind_if_no_reply_at_ms') FROM outbox_entry
             WHERE state <> 'cancelled' AND (id = ?1 OR json_extract(draft_json, '$.draft.id') = ?1)
               AND json_extract(draft_json, '$.remind_if_no_reply_at_ms') IS NOT NULL LIMIT 1",
            params![id], |r| r.get::<_, Option<i64>>(0)).optional()?.flatten()))
    }
    pub fn outbox_reply_reminder_clear(&self, id: String) -> Result<usize, StoreError> { self.write(move |ctx| ctx.outbox_reply_reminder_clear(&id)) }
    /// Whether a message is muted (its own flag; a muted thread has every message muted). None = no such message.
    pub fn is_muted(&self, message_id: &str) -> Result<Option<bool>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| Ok(c.query_row("SELECT muted FROM message WHERE id = ?1", params![id], |r| r.get::<_, i64>(0)).optional()?.map(|m| m != 0)))
    }
    /// Whether `RemindIfNoReply` has something to put the reminder on: a draft, or an outbox entry (by its id or by the id of its draft).
    pub fn reply_reminder_target_exists(&self, id: &str) -> Result<bool, StoreError> {
        let id = id.to_string();
        self.read(move |c| Ok(c.query_row(
            "SELECT EXISTS (SELECT 1 FROM draft WHERE id = ?1) OR EXISTS (SELECT 1 FROM outbox_entry WHERE state <> 'cancelled' AND (id = ?1 OR json_extract(draft_json, '$.draft.id') = ?1))",
            params![id], |r| r.get::<_, bool>(0))?))
    }
    /// When the list of this message was left (Unsubscribe), None when it was not. A message no row names is also None.
    pub fn unsubscribed_at(&self, message_id: &str) -> Result<Option<i64>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| Ok(c.query_row("SELECT unsubscribed_at_ms FROM message WHERE id = ?1", params![id], |r| r.get::<_, Option<i64>>(0)).optional()?.flatten()))
    }
    /// The account a message belongs to: the one an unsubscribe mail is sent from.
    pub fn message_account(&self, message_id: &str) -> Result<Option<String>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| Ok(c.query_row("SELECT account_id FROM message WHERE id = ?1", params![id], |r| r.get::<_, String>(0)).optional()?))
    }
    pub fn draft_remind_if_no_reply(&self, draft_id: String, at_ms: i64, now_ms: i64) -> Result<(), StoreError> { self.write(move |ctx| ctx.draft_remind_if_no_reply(&draft_id, at_ms, now_ms)) }
    pub fn reply_reminder_on_sent(&self, draft_id: String, sent_message_id: String, now_ms: i64) -> Result<Option<String>, StoreError> { self.write(move |ctx| ctx.reply_reminder_on_sent(&draft_id, &sent_message_id, now_ms)) }
    pub fn reply_reminders_fire(&self, now_ms: i64) -> Result<Vec<String>, StoreError> { self.write(move |ctx| ctx.reply_reminders_fire(now_ms)) }
    /// The List-Unsubscribe URLs in header order, and whether one-click was offered (`contracts::unsubscribe_method_from` picks).
    pub fn list_unsubscribe(&self, message_id: &str) -> Result<(Vec<String>, bool), StoreError> { let id = message_id.to_string(); self.read(move |c| list_unsubscribe_of(c, &id)) }
    /// The message's attachments with name, MIME type, size and the local path once downloaded.
    pub fn attachments(&self, message_id: &str) -> Result<Vec<AttachmentFile>, StoreError> { let id = message_id.to_string(); self.read(move |c| attachments_of(c, &id)) }
}

/// The attachment index's door: rows written through the store's writer, read through its reader.
impl AttachmentRows for Store {
    fn save(&self, entry: &AttachmentIndexEntry) -> Result<(), CoreError> {
        let e = entry.clone();
        self.write(move |ctx| ctx.attachment_index_save(&e)).map_err(core_err)
    }
    fn load(&self, attachment_id: &str) -> Result<Option<AttachmentContent>, CoreError> {
        let id = attachment_id.to_string();
        self.read(move |c| content_of(c, &id)).map_err(core_err)
    }
    fn catalog(&self, message_ids: &[String]) -> Result<Vec<CatalogRow>, CoreError> {
        let ids = message_ids.to_vec();
        self.read(move |c| catalog_of(c, &ids)).map_err(core_err)
    }
}
