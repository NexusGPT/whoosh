//! junk-r2: the store side of the local spam filter (migration 0012). The learned table (`junk_model`, one per profile), the accounts whose
//! first-run training ran (`junk_bootstrap`), the owner's lesson per message (`junk_learned`), and each live arrival's score and state
//! (`message.junk_score`, `message.junk_state`). Every statement is parameterised; nothing here decides anything.
use rusqlite::{params, OptionalExtension};

use crate::{Store, StoreError, WriteCtx};

/// The owner's lesson on one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunkLesson {
    Junk,
    NotJunk,
}

impl JunkLesson {
    fn word(self) -> &'static str {
        match self { JunkLesson::Junk => "junk", JunkLesson::NotJunk => "not_junk" }
    }
    fn from_word(w: &str) -> Option<JunkLesson> {
        match w { "junk" => Some(JunkLesson::Junk), "not_junk" => Some(JunkLesson::NotJunk), _ => None }
    }
}

/// What the filter did with a live arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunkState {
    /// Kept in place, marked for the owner to review.
    Suspect,
    /// Moved to the account's Junk folder.
    Moved,
}

impl JunkState {
    fn word(self) -> &'static str {
        match self { JunkState::Suspect => "suspect", JunkState::Moved => "moved" }
    }
    fn from_word(w: &str) -> Option<JunkState> {
        match w { "suspect" => Some(JunkState::Suspect), "moved" => Some(JunkState::Moved), _ => None }
    }
}

/// The stored learned table and the message counts it was learned from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredJunkModel {
    pub bytes: Vec<u8>,
    pub trained_junk: i64,
    pub trained_good: i64,
}

/// What the filter reads of a stored message (the bootstrap and a lesson): no header beyond the ones the row keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JunkText {
    pub message_id: String,
    pub sender_name: String,
    pub from_address: String,
    pub subject: String,
    /// The stored body text when the message was opened, else its preview.
    pub text: String,
    pub html: Option<String>,
}

const TEXT_COLS: &str = "id, sender_name, from_address, subject, coalesce(body_text, preview), body_html";

fn text_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<JunkText> {
    Ok(JunkText { message_id: r.get(0)?, sender_name: r.get(1)?, from_address: r.get(2)?, subject: r.get(3)?, text: r.get(4)?, html: r.get(5)? })
}

impl WriteCtx<'_> {
    /// Records the owner's lesson and answers the one it replaced (None the first time). Written BEFORE the model trains.
    pub fn junk_lesson_put(&mut self, message_id: &str, lesson: JunkLesson) -> Result<Option<JunkLesson>, StoreError> {
        let before: Option<String> = self.tx.prepare_cached("SELECT label FROM junk_learned WHERE message_id = ?1")?.query_row(params![message_id], |r| r.get(0)).optional()?;
        self.tx.execute(
            "INSERT INTO junk_learned (message_id, label) VALUES (?1, ?2) ON CONFLICT(message_id) DO UPDATE SET label = excluded.label",
            params![message_id, lesson.word()],
        )?;
        Ok(before.as_deref().and_then(JunkLesson::from_word))
    }

    /// The filter's score of a live arrival, and what it did (None = kept with its score only).
    pub fn junk_mark(&mut self, message_id: &str, score: f64, state: Option<JunkState>) -> Result<usize, StoreError> {
        self.will_change(&[message_id.to_string()])?;
        Ok(self.tx.execute("UPDATE message SET junk_score = ?2, junk_state = ?3 WHERE id = ?1", params![message_id, score.clamp(0.0, 1.0), state.map(JunkState::word)])?)
    }

    /// The owner said "not junk": the mark goes (the score stays, it is what the filter thought).
    pub fn junk_clear_state(&mut self, message_id: &str) -> Result<usize, StoreError> {
        self.will_change(&[message_id.to_string()])?;
        Ok(self.tx.execute("UPDATE message SET junk_state = NULL WHERE id = ?1", params![message_id])?)
    }
}

impl Store {
    /// The learned table, None before the first save.
    pub fn junk_model(&self) -> Result<Option<StoredJunkModel>, StoreError> {
        self.read(|c| {
            Ok(c.query_row("SELECT model_bytes, trained_junk, trained_good FROM junk_model WHERE id = 1", [], |r| {
                Ok(StoredJunkModel { bytes: r.get(0)?, trained_junk: r.get(1)?, trained_good: r.get(2)? })
            }).optional()?)
        })
    }

    /// Writes the learned table back (after a batch of lessons, never after each).
    pub fn junk_model_save(&self, bytes: Vec<u8>, trained_junk: i64, trained_good: i64, now_ms: i64) -> Result<(), StoreError> {
        self.write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO junk_model (id, model_bytes, trained_junk, trained_good, updated_at_ms) VALUES (1, ?1, ?2, ?3, ?4) \
                 ON CONFLICT(id) DO UPDATE SET model_bytes = excluded.model_bytes, trained_junk = excluded.trained_junk, trained_good = excluded.trained_good, updated_at_ms = excluded.updated_at_ms",
                params![bytes, trained_junk, trained_good, now_ms],
            )?;
            Ok(())
        })
    }

    /// True once the account's first-run training ran.
    pub fn junk_bootstrapped(&self, account_id: &str) -> Result<bool, StoreError> {
        let a = account_id.to_string();
        self.read(move |c| Ok(c.prepare_cached("SELECT 1 FROM junk_bootstrap WHERE account_id = ?1")?.exists(params![a])?))
    }

    /// Records the first-run training of one account; a second record changes nothing (the first stays). True when this one counted.
    pub fn junk_bootstrap_record(&self, account_id: &str, junk_learned: i64, good_learned: i64, now_ms: i64) -> Result<bool, StoreError> {
        let a = account_id.to_string();
        self.write(move |ctx| Ok(ctx.tx.execute("INSERT OR IGNORE INTO junk_bootstrap (account_id, done_at_ms, junk_learned, good_learned) VALUES (?1, ?2, ?3, ?4)", params![a, now_ms, junk_learned, good_learned])? == 1))
    }

    /// The (junk, not junk) counts the account's first-run training learned, None before it ran.
    pub fn junk_bootstrap_report(&self, account_id: &str) -> Result<Option<(i64, i64)>, StoreError> {
        let a = account_id.to_string();
        self.read(move |c| Ok(c.query_row("SELECT junk_learned, good_learned FROM junk_bootstrap WHERE account_id = ?1", params![a], |r| Ok((r.get(0)?, r.get(1)?))).optional()?))
    }

    /// The owner's lesson on one message, None when there is none.
    pub fn junk_lesson(&self, message_id: &str) -> Result<Option<JunkLesson>, StoreError> {
        let m = message_id.to_string();
        self.read(move |c| {
            let w: Option<String> = c.prepare_cached("SELECT label FROM junk_learned WHERE message_id = ?1")?.query_row(params![m], |r| r.get(0)).optional()?;
            Ok(w.as_deref().and_then(JunkLesson::from_word))
        })
    }

    /// One message's score and state, as the filter left them.
    pub fn junk_of(&self, message_id: &str) -> Result<Option<(Option<f64>, Option<JunkState>)>, StoreError> {
        let m = message_id.to_string();
        self.read(move |c| {
            let row: Option<(Option<f64>, Option<String>)> = c.prepare_cached("SELECT junk_score, junk_state FROM message WHERE id = ?1")?.query_row(params![m], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
            Ok(row.map(|(s, w)| (s, w.as_deref().and_then(JunkState::from_word))))
        })
    }

    /// The newest `limit` messages of the account's folders, newest first: what a first-run training reads.
    pub fn junk_corpus(&self, account_id: &str, folders: &[String], limit: usize) -> Result<Vec<JunkText>, StoreError> {
        if folders.is_empty() || limit == 0 { return Ok(vec![]); }
        let a = account_id.to_string();
        let marks = (0..folders.len()).map(|i| format!("?{}", i + 3)).collect::<Vec<_>>().join(",");
        let sql = format!("SELECT {TEXT_COLS} FROM message WHERE account_id = ?1 AND folder IN ({marks}) ORDER BY received_at_ms DESC, id DESC LIMIT ?2");
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        self.read(move |c| {
            let mut st = c.prepare(&sql)?;
            let mut args: Vec<&dyn rusqlite::ToSql> = vec![&a, &limit];
            for f in folders { args.push(f); }
            let rows = st.query_map(args.as_slice(), text_row)?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// The server id (IMAP UID) a stored message keeps, None when it has none (imported, or never synced).
    pub fn junk_remote_id(&self, message_id: &str) -> Result<Option<String>, StoreError> {
        let m = message_id.to_string();
        self.read(move |c| Ok(c.prepare_cached("SELECT remote_id FROM message WHERE id = ?1")?.query_row(params![m], |r| r.get::<_, Option<String>>(0)).optional()?.flatten()))
    }

    /// The text of the messages a lesson is about (an unknown id is left out).
    pub fn junk_texts(&self, message_ids: &[String]) -> Result<Vec<JunkText>, StoreError> {
        self.read(move |c| {
            let mut st = c.prepare_cached(&format!("SELECT {TEXT_COLS} FROM message WHERE id = ?1"))?;
            let mut out = Vec::new();
            for id in message_ids {
                if let Some(t) = st.query_row(params![id], text_row).optional()? { out.push(t); }
            }
            Ok(out)
        })
    }
}
