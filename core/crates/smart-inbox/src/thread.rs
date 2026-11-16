//! The rest of the thread (lane labels-r5): the earlier messages of a message's thread, for a label whose AI card reads it.
//!
//! Owner, 2026-11-10: in the AI card "we should add 'rest of message thread'". The decision crate cuts what is loaded here to each
//! model's size budget (oldest first, `decision::thread_context`); this file only loads, and it loads no more than the biggest
//! budget can use (`up_to_chars`), so a thread of 400 messages costs one short read, not 400 bodies in memory.
//! Read from columns already synced, never a second fetch. Off by default: nothing here runs unless a label ticks the input.
use crate::facts::{MessageFacts, ThreadPart};
use rusqlite::{params, Connection};

/// Folders whose messages are not part of the conversation the model should read: drafts (unsent words), trash and spam.
/// Folder names differ by server, so the test is the word, not a role.
const NOT_CONVERSATION_SQL: &str = "lower(m.folder) NOT LIKE '%draft%' AND lower(m.folder) NOT LIKE '%trash%' \
    AND lower(m.folder) NOT LIKE '%junk%' AND lower(m.folder) NOT LIKE '%spam%' AND lower(m.folder) NOT LIKE '%deleted%'";

/// The lines that start a quoted earlier message: an attribution ("On Mon, X wrote:") or a quote mark. A reply repeats the messages
/// before it; those messages are read from their own rows, so the repeat only spends the budget.
fn starts_a_quote(line: &str) -> bool {
    let l = line.trim();
    if l.starts_with('>') {
        return true;
    }
    let low = l.to_lowercase();
    (low.ends_with("wrote:") || low.ends_with("a écrit :") || low.ends_with("a écrit:") || low.ends_with("schrieb:") || low.ends_with("escribió:") || low.ends_with("ha scritto:"))
        && l.chars().count() < 200
}

/// The text of a message without the quoted messages it carries: cut at the first attribution or quote line, blank lines trimmed.
/// When nothing is left (the whole text was a quote) the text is kept as it is: a message must not read as empty.
pub fn without_quote(text: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    for line in text.lines() {
        if starts_a_quote(line) {
            break;
        }
        kept.push(line);
    }
    let cut = kept.join("\n").trim().to_string();
    if cut.is_empty() { text.trim().to_string() } else { cut }
}

/// Fills `f.thread_before` with the messages of the thread that came before `f`, oldest first, newest kept: the loader walks back from
/// the message and stops once it holds `up_to_chars` characters of text (the biggest budget any model can use), each message cut to
/// that many characters. Any folder counts except drafts, trash and spam: the owner's own replies in Sent are part of the thread.
pub fn attach_thread(conn: &Connection, f: &mut MessageFacts, up_to_chars: usize) -> rusqlite::Result<()> {
    f.thread_before.clear();
    if up_to_chars == 0 || f.thread_id.is_empty() {
        return Ok(());
    }
    // earlier = the complement of the store's "later in the thread" (received_at_ms, then id), without the message itself
    let sql = format!(
        "SELECT m.from_address, m.received_at_ms, COALESCE(m.body_text, m.preview), lower(m.from_address) IN (SELECT lower(address) FROM account) \
         FROM message m WHERE m.thread_id = ?1 AND m.id <> ?3 AND (m.received_at_ms < ?2 OR (m.received_at_ms = ?2 AND m.id < ?3)) \
           AND {NOT_CONVERSATION_SQL} ORDER BY m.received_at_ms DESC, m.id DESC"
    );
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(params![f.thread_id, f.received_at_ms, f.message_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)? != 0))
    })?;
    let mut held = 0usize;
    for row in rows {
        let (from, at_ms, text, from_owner) = row?;
        let text: String = without_quote(&text).chars().take(up_to_chars).collect();
        if text.is_empty() {
            continue;
        }
        held += text.chars().count();
        f.thread_before.push(ThreadPart { from, at_ms, from_owner, text });
        if held >= up_to_chars {
            break;
        }
    }
    f.thread_before.reverse();
    Ok(())
}
