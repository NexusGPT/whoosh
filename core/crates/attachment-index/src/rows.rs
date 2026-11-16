//! Where an entry lands. The store crate owns the schema (migration 0003, vector_key from 0004); `AttachmentRows` is the door it
//! implements, and `SqliteRows` is the same statements over a connection, so the lane's checks run the real table.
use crate::entry::{AttachmentIndexEntry, IndexState};
use crate::{AttachmentKind, CatalogRow};
use contracts::{AttachmentContent, CoreError};
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Mutex;

pub trait AttachmentRows: Send + Sync {
    /// Files an entry: the attachment_content row, and the words in `attachment.extracted_text` (the text index reads that column
    /// and re-indexes the message when it changes). Entries that are not Indexed remove a stale row.
    fn save(&self, entry: &AttachmentIndexEntry) -> Result<(), CoreError>;
    fn load(&self, attachment_id: &str) -> Result<Option<AttachmentContent>, CoreError>;
    /// The attachments of these messages, with what the search needs to name them and say their state.
    fn catalog(&self, message_ids: &[String]) -> Result<Vec<CatalogRow>, CoreError>;
}

pub struct SqliteRows {
    conn: Mutex<Connection>,
}

fn db(e: rusqlite::Error) -> CoreError {
    CoreError::Unexpected { reason_words: format!("the attachment index could not reach its table: {e}") }
}

fn labels_json(labels: &[String]) -> String {
    let items: Vec<String> = labels
        .iter()
        .map(|l| {
            let mut s = String::from("\"");
            for c in l.chars() {
                match c {
                    '"' => s.push_str("\\\""),
                    '\\' => s.push_str("\\\\"),
                    c if (c as u32) < 0x20 => s.push(' '),
                    c => s.push(c),
                }
            }
            s.push('"');
            s
        })
        .collect();
    format!("[{}]", items.join(","))
}

impl SqliteRows {
    pub fn new(conn: Connection) -> Self {
        Self { conn: Mutex::new(conn) }
    }
}

impl AttachmentRows for SqliteRows {
    fn save(&self, e: &AttachmentIndexEntry) -> Result<(), CoreError> {
        let mut conn = self.conn.lock().expect("lock");
        let tx = conn.transaction().map_err(db)?;
        if e.state != IndexState::Indexed {
            tx.execute("DELETE FROM attachment_content WHERE attachment_id = ?1", params![e.attachment_id]).map_err(db)?;
            tx.execute("UPDATE attachment SET extracted_text = NULL WHERE id = ?1", params![e.attachment_id]).map_err(db)?;
            return tx.commit().map_err(db);
        }
        let c = e.to_content();
        tx.execute(
            "INSERT INTO attachment_content (attachment_id, text, ocr_text, labels_json, transcript, embedded, updated_at_ms, vector_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(attachment_id) DO UPDATE SET text = ?2, ocr_text = ?3, labels_json = ?4, transcript = ?5, embedded = ?6, updated_at_ms = ?7, vector_key = ?8",
            params![c.attachment_id, c.text, c.ocr_text, labels_json(&c.labels), c.transcript, c.embedded as i64, c.updated_at_ms, c.vector_key],
        )
        .map_err(db)?;
        let words = e.word_text();
        tx.execute("UPDATE attachment SET extracted_text = ?2 WHERE id = ?1", params![e.attachment_id, if words.is_empty() { None } else { Some(words) }]).map_err(db)?;
        tx.commit().map_err(db)
    }

    fn load(&self, attachment_id: &str) -> Result<Option<AttachmentContent>, CoreError> {
        let conn = self.conn.lock().expect("lock");
        conn.query_row(
            "SELECT text, ocr_text, labels_json, transcript, embedded, updated_at_ms, vector_key FROM attachment_content WHERE attachment_id = ?1",
            params![attachment_id],
            |r| {
                let labels: String = r.get(2)?;
                Ok(AttachmentContent {
                    attachment_id: attachment_id.to_string(),
                    text: r.get(0)?,
                    ocr_text: r.get(1)?,
                    labels: parse_labels(&labels),
                    transcript: r.get(3)?,
                    embedded: r.get::<_, i64>(4)? != 0,
                    updated_at_ms: r.get(5)?,
                    vector_key: r.get(6)?,
                })
            },
        )
        .optional()
        .map_err(db)
    }

    fn catalog(&self, message_ids: &[String]) -> Result<Vec<CatalogRow>, CoreError> {
        let conn = self.conn.lock().expect("lock");
        let mut st = conn
            .prepare_cached(
                "SELECT a.message_id, a.id, a.file_name, a.mime_type, a.local_path, c.attachment_id IS NOT NULL, coalesce(a.extracted_text, '')
                 FROM attachment a LEFT JOIN attachment_content c ON c.attachment_id = a.id WHERE a.message_id = ?1 ORDER BY a.id",
            )
            .map_err(db)?;
        let mut out = Vec::new();
        for id in message_ids {
            let rows = st
                .query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, bool>(5)?, r.get::<_, String>(6)?)))
                .map_err(db)?;
            for row in rows {
                let (message_id, attachment_id, file_name, mime, local, indexed, text) = row.map_err(db)?;
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
}

/// labels_json is written by `labels_json`: an array of strings.
fn parse_labels(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut in_str, mut esc) = (Vec::new(), String::new(), false, false);
    for c in s.chars() {
        match (in_str, esc, c) {
            (true, true, 'n') => { cur.push('\n'); esc = false }
            (true, true, c) => { cur.push(c); esc = false }
            (true, false, '\\') => esc = true,
            (true, false, '"') => { out.push(std::mem::take(&mut cur)); in_str = false }
            (true, false, c) => cur.push(c),
            (false, _, '"') => in_str = true,
            _ => {}
        }
    }
    out
}
