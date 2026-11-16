//! What the draft engine reads of the mail, through its OWN read-only connection to the store's file (WAL: it never waits for the writer, and no
//! view waits on it, as the crawl's `background_reads`): a message's facts (the label flow's, `smart_inbox::load::facts_for`), the earlier
//! messages of its thread, the newest message of a thread, and the owner's Sent mail (`SentMail` of reply-voice).
use std::sync::Mutex;

use contracts::CoreError;
use reply_voice::{SentMail, SentMessage, SentQuery};
use rusqlite::{params, Connection};
use smart_inbox::load::{facts_for, load_context, open_reader, Context};
use smart_inbox::thread::attach_thread;
use smart_inbox::MessageFacts;
use store::Store;

pub(crate) struct MailReads { conn: Mutex<Connection>, ctx: Context }

fn words(what: &str, e: impl std::fmt::Display) -> CoreError { CoreError::Unexpected { reason_words: format!("{what}: {e}") } }

/// The facts of one message with the earlier messages of its thread (cut to `thread_chars`, newest kept; 0 = none loaded).
impl MailReads {
    pub(crate) fn open(store: &Store) -> Result<MailReads, CoreError> {
        let conn = open_reader(store.path()).map_err(|e| words("the mail database could not be opened for reading", e))?;
        let ctx = load_context(&conn).map_err(|e| words("the owner's addresses could not be read", e))?;
        Ok(MailReads { conn: Mutex::new(conn), ctx })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> { self.conn.lock().unwrap_or_else(|e| e.into_inner()) }

    pub(crate) fn facts(&self, message_id: &str, thread_chars: usize) -> Result<Option<MessageFacts>, CoreError> {
        let c = self.conn();
        let mut all = facts_for(&c, &self.ctx, &[message_id.to_string()]).map_err(|e| words("the facts of the message could not be read", e))?;
        let Some(mut f) = all.pop() else { return Ok(None) };
        attach_thread(&c, &mut f, thread_chars).map_err(|e| words("the earlier messages of the thread could not be read", e))?;
        Ok(Some(f))
    }

    /// The newest message of a thread the owner did not write (the one a reply answers), else the newest of all; None = no such thread.
    pub(crate) fn newest_in_thread(&self, thread_id: &str) -> Result<Option<String>, CoreError> {
        let c = self.conn();
        c.query_row(
            "SELECT id FROM message WHERE thread_id = ?1 ORDER BY (lower(from_address) IN (SELECT lower(address) FROM account)), received_at_ms DESC, id DESC LIMIT 1",
            params![thread_id], |r| r.get::<_, String>(0)).map(Some).or_else(|e| match e { rusqlite::Error::QueryReturnedNoRows => Ok(None), e => Err(words("the thread could not be read", e)) })
    }

    /// The folder a stored message sits in and its attachment names.
    pub(crate) fn folder_and_attachments(&self, message_id: &str) -> Result<(String, Vec<String>), CoreError> {
        let c = self.conn();
        let folder: String = c.query_row("SELECT folder FROM message WHERE id = ?1", params![message_id], |r| r.get(0)).map_err(|e| words("the message could not be read", e))?;
        let mut st = c.prepare_cached("SELECT file_name FROM attachment WHERE message_id = ?1 ORDER BY rowid").map_err(|e| words("the attachments could not be read", e))?;
        let names = st.query_map(params![message_id], |r| r.get::<_, String>(0)).and_then(|rows| rows.collect::<Result<Vec<_>, _>>()).map_err(|e| words("the attachments could not be read", e))?;
        Ok((folder, names))
    }
}

impl SentMail for MailReads {
    fn execute(&self, query: &SentQuery, limit: usize) -> Result<Vec<SentMessage>, CoreError> {
        let (filter, arg) = match query {
            SentQuery::Latest => ("1 = 1", String::new()),
            SentQuery::ToRecipient { recipient } => ("EXISTS (SELECT 1 FROM json_each(m.to_json) WHERE lower(value) = lower(?2))", recipient.clone()),
            SentQuery::InThread { thread_id } => ("m.thread_id = ?2", thread_id.clone()),
        };
        let sql = format!(
            "SELECT m.thread_id, (SELECT group_concat(value, char(31)) FROM json_each(m.to_json)), m.received_at_ms, COALESCE(m.body_text, m.preview) FROM message m \
             WHERE lower(m.from_address) IN (SELECT lower(address) FROM account) AND ({filter}) AND (?2 = ?2) ORDER BY m.received_at_ms DESC LIMIT ?1");
        let c = self.conn();
        let mut st = c.prepare(&sql).map_err(|e| words("the sent mail could not be read", e))?;
        let rows = st.query_map(params![limit as i64, arg], |r| {
            let to: Option<String> = r.get(1)?;
            Ok(SentMessage { thread_id: r.get(0)?, recipients: to.map(|t| t.split('\u{1f}').map(str::to_string).collect()).unwrap_or_default(), date_ms: r.get(2)?, text: r.get(3)? })
        }).map_err(|e| words("the sent mail could not be read", e))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| words("the sent mail could not be read", e))
    }
}
