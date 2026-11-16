//! lane real-replyto (migration 0027): each mail's Reply-To header kept on this Mac, so a reply goes where the sender asked replies to go.
//! One column of `message`: `reply_to_json`, a JSON array of {"name", "address"} in header order. NULL = not read yet (a mail stored before
//! 0027): it reads as "no Reply-To" until its headers are read again; '[]' = read, none.
//! `NewMessage` does not carry it (it is built field by field in crates this lane does not own), so, as the threading headers do
//! (`thread_headers.rs`), the arrival writes it beside the insert in the same transaction (`WriteCtx::set_reply_to`) and the fill-in pass
//! after a folder sync writes the rest (`set_reply_to_quiet`).
use crate::error::StoreError;
use crate::ops::json_string;
use crate::Store;
use rusqlite::{params, Connection};

/// One address of a Reply-To header: its display name ('' when the header gave none) and its address, as the header wrote them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTo {
    pub name: String,
    pub address: String,
}

/// The column's JSON for a header: `[{"name":"..","address":".."},..]`; an address-less entry is left out.
fn encode(items: &[ReplyTo]) -> String {
    let parts: Vec<String> = items.iter().filter(|r| !r.address.trim().is_empty())
        .map(|r| format!("{{\"name\":{},\"address\":{}}}", json_string(r.name.trim()), json_string(r.address.trim()))).collect();
    format!("[{}]", parts.join(","))
}

fn read_reply_to(c: &Connection, row_id: &str) -> Result<Vec<ReplyTo>, StoreError> {
    let mut st = c.prepare_cached(
        "SELECT COALESCE(json_extract(j.value, '$.name'), ''), json_extract(j.value, '$.address') \
         FROM message m, json_each(COALESCE(m.reply_to_json, '[]')) j WHERE m.id = ?1 AND j.type = 'object' ORDER BY j.key",
    )?;
    let rows = st.query_map(params![row_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (name, address) = row?;
        if let Some(address) = address.filter(|a| !a.trim().is_empty()) { out.push(ReplyTo { name, address }); }
    }
    Ok(out)
}

/// The first Reply-To display name's first word, the "name to greet" the contract's `Message.reply_to_name` carries (None when the mail has
/// no Reply-To, or its first entry has no name).
pub(crate) fn first_reply_to_name(c: &Connection, row_id: &str) -> Result<Option<String>, StoreError> {
    Ok(read_reply_to(c, row_id)?.into_iter().next().and_then(|r| r.name.split_whitespace().next().map(str::to_string)))
}

impl Store {
    /// The Reply-To addresses of one stored message, in header order. Empty when the mail has none, was stored before 0027 and not read
    /// again, or is not stored.
    pub fn reply_to(&self, message_id: &str) -> Result<Vec<ReplyTo>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| read_reply_to(c, &id))
    }

    /// The fill-in pass's write: each row's Reply-To replaces what it held ('[]' = the mail has none). One transaction; nothing is
    /// published (no list, count or thread shows this column: the update hook's notes are dropped, as `set_thread_headers_quiet` does).
    /// Answers how many rows changed.
    pub fn set_reply_to_quiet(&self, rows: Vec<(String, Vec<ReplyTo>)>) -> Result<usize, StoreError> {
        if rows.is_empty() { return Ok(0); }
        self.submit(move |st| {
            let out = (|| -> Result<usize, StoreError> {
                let tx = st.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let changed = write_reply_to(&tx, &rows)?;
                tx.commit()?;
                Ok(changed)
            })();
            st.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out
        })?
    }
}

fn write_reply_to(tx: &rusqlite::Transaction<'_>, rows: &[(String, Vec<ReplyTo>)]) -> Result<usize, StoreError> {
    let mut set = tx.prepare_cached("UPDATE message SET reply_to_json = ?2 WHERE id = ?1 AND reply_to_json IS NOT ?2")?;
    let mut changed = 0;
    for (id, items) in rows { changed += set.execute(params![id, encode(items)])?; }
    Ok(changed)
}

impl crate::WriteCtx<'_> {
    /// The arrival's write, inside the insert's own transaction: each message's Reply-To ('[]' for a mail that has none).
    pub fn set_reply_to(&mut self, rows: &[(String, Vec<ReplyTo>)]) -> Result<usize, StoreError> {
        write_reply_to(self.tx, rows)
    }
}
