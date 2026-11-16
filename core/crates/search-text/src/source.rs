//! Reading the message table. The index never writes SQLite; it reads through its own read-only
//! connection, so it cannot block the store's single writer.
use crate::error::Error;
use rusqlite::{params, Connection, OpenFlags, Row};
use std::path::Path;

/// One message as the index sees it.
#[derive(Debug, Clone)]
pub struct Indexed {
    pub rowid: i64,
    pub id: String,
    pub stamp: u64,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub body: String,
    pub attachments: String,
}

pub fn open_reader(path: &Path) -> Result<Connection, Error> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// The sizes that say a row's searchable text changed: the stamp of a message. `octet_length` reads the
/// record header only, so a stamp scan over a million rows does not read a million bodies.
const STAMP_COLUMNS: &str = "m.rowid, m.id, octet_length(m.body_text), octet_length(m.body_html), octet_length(m.subject), \
     (SELECT count(*) FROM attachment a WHERE a.message_id = m.id), \
     (SELECT coalesce(sum(octet_length(a.extracted_text)), 0) FROM attachment a WHERE a.message_id = m.id)";

const TEXT_COLUMNS: &str = "m.sender_name, m.from_address, m.to_json, m.cc_json, m.subject, m.preview, m.body_text, m.body_html, \
     (SELECT group_concat(a.file_name || ' ' || coalesce(a.extracted_text, ''), char(10)) FROM attachment a WHERE a.message_id = m.id)";

fn stamp_of(row: &Row<'_>) -> rusqlite::Result<(i64, String, u64)> {
    let rowid: i64 = row.get(0)?;
    let id: String = row.get(1)?;
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for b in id.bytes() {
        mix(b as u64);
    }
    for i in 2..7 {
        mix(row.get::<_, Option<i64>>(i)?.map(|v| v as u64 + 1).unwrap_or(0));
    }
    Ok((rowid, id, h))
}

fn indexed_of(row: &Row<'_>) -> rusqlite::Result<Indexed> {
    let (rowid, id, stamp) = stamp_of(row)?;
    let (sender, from_address, to_json, cc_json): (String, String, String, String) = (row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?);
    let subject: String = row.get(11)?;
    let preview: String = row.get(12)?;
    let body_text: Option<String> = row.get(13)?;
    let body_html: Option<String> = row.get(14)?;
    let attachments: Option<String> = row.get(15)?;
    let body = match body_text.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => match body_html.filter(|h| !h.trim().is_empty()) {
            Some(h) => strip_tags(&h),
            None => preview,
        },
    };
    Ok(Indexed {
        rowid,
        id,
        stamp,
        subject,
        from: format!("{sender} {from_address}"),
        to: format!("{} {}", addresses(&to_json), addresses(&cc_json)),
        body,
        attachments: attachments.unwrap_or_default(),
    })
}

/// The addresses of a JSON array of strings, space separated; anything else is indexed as it is.
fn addresses(json: &str) -> String {
    match serde_json::from_str::<Vec<String>>(json) {
        Ok(items) => items.join(" "),
        Err(_) => json.to_string(),
    }
}

/// HTML to text for a message that has no plain body: tags and script/style blocks dropped, common entities decoded.
pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let skip_block = ["<script", "<style"].iter().find(|t| lower[i..].starts_with(**t));
            if let Some(tag) = skip_block {
                let close = format!("</{}", &tag[1..]);
                match lower[i..].find(&close) {
                    Some(at) => i += at,
                    None => break,
                }
            }
            match html[i..].find('>') {
                Some(end) => i += end + 1,
                None => break,
            }
            out.push(' ');
        } else {
            let ch = html[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// The next `limit` messages after `after_rowid`, in rowid order.
pub fn fetch_after(conn: &Connection, after_rowid: i64, limit: usize) -> Result<Vec<Indexed>, Error> {
    let sql = format!("SELECT {STAMP_COLUMNS}, {TEXT_COLUMNS} FROM message m WHERE m.rowid > ?1 ORDER BY m.rowid LIMIT ?2");
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(params![after_rowid, limit as i64], indexed_of)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The messages with these rowids (those that still exist).
pub fn fetch_rowids(conn: &Connection, rowids: &[i64]) -> Result<Vec<Indexed>, Error> {
    let list = serde_json::to_string(rowids).map_err(|e| Error::Io(e.to_string()))?;
    let sql = format!("SELECT {STAMP_COLUMNS}, {TEXT_COLUMNS} FROM message m WHERE m.rowid IN (SELECT value FROM json_each(?1)) ORDER BY m.rowid");
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(params![list], indexed_of)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The messages with these ids.
pub fn fetch_ids(conn: &Connection, ids: &[String]) -> Result<Vec<Indexed>, Error> {
    let list = serde_json::to_string(ids).map_err(|e| Error::Io(e.to_string()))?;
    let sql = format!("SELECT {STAMP_COLUMNS}, {TEXT_COLUMNS} FROM message m WHERE m.id IN (SELECT value FROM json_each(?1)) ORDER BY m.rowid");
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(params![list], indexed_of)?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// (rowid, stamp) of every message with `lo < rowid <= hi`, no text read.
pub fn stamps_between(conn: &Connection, lo: i64, hi: i64) -> Result<Vec<(i64, u64)>, Error> {
    let sql = format!("SELECT {STAMP_COLUMNS} FROM message m WHERE m.rowid > ?1 AND m.rowid <= ?2 ORDER BY m.rowid");
    let mut st = conn.prepare_cached(&sql)?;
    let rows = st.query_map(params![lo, hi], |r| stamp_of(r).map(|(rowid, _, stamp)| (rowid, stamp)))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn max_message_rowid(conn: &Connection) -> Result<i64, Error> {
    Ok(conn.query_row("SELECT coalesce(max(rowid), 0) FROM message", [], |r| r.get(0))?)
}

pub fn max_attachment_rowid(conn: &Connection) -> Result<i64, Error> {
    Ok(conn.query_row("SELECT coalesce(max(rowid), 0) FROM attachment", [], |r| r.get(0))?)
}

/// The messages owning attachments added after `after_rowid`.
pub fn attachment_owners_after(conn: &Connection, after_rowid: i64) -> Result<Vec<String>, Error> {
    let mut st = conn.prepare_cached("SELECT DISTINCT message_id FROM attachment WHERE rowid > ?1")?;
    let rows = st.query_map(params![after_rowid], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The text a result snippet is cut from.
pub fn body_of(conn: &Connection, id: &str) -> Result<Option<String>, Error> {
    let mut st = conn.prepare_cached("SELECT m.preview, m.body_text, m.body_html FROM message m WHERE m.id = ?1")?;
    let mut rows = st.query(params![id])?;
    match rows.next()? {
        None => Ok(None),
        Some(r) => {
            let preview: String = r.get(0)?;
            let text: Option<String> = r.get(1)?;
            let html: Option<String> = r.get(2)?;
            Ok(Some(match (text.filter(|t| !t.trim().is_empty()), html.filter(|h| !h.trim().is_empty())) {
                (Some(t), _) => t,
                (None, Some(h)) => strip_tags(&h),
                _ => preview,
            }))
        }
    }
}

/// match-inside (SPEC A149): what a ranked hit's `matched_in` and snippet are read from: the text the passage is cut from (as
/// `body_of` when `with_text`, the typed words having been found in the text; else the preview, so a subject or sender match reads
/// no body), the subject, the sender's name and address, and each attachment's file name with its extracted text. None when the
/// message is gone.
pub fn hit_facts(conn: &Connection, id: &str, with_text: bool) -> Result<Option<crate::query::HitFacts>, Error> {
    let head: Option<(String, String, String, String)> = {
        let mut st = conn.prepare_cached("SELECT m.subject, m.sender_name, m.from_address, m.preview FROM message m WHERE m.id = ?1")?;
        let mut rows = st.query(params![id])?;
        let head = match rows.next()? {
            None => None,
            Some(r) => Some((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
        };
        head
    };
    let Some((subject, sender_name, from_address, preview)) = head else { return Ok(None) };
    let body = if with_text { body_of(conn, id)?.unwrap_or(preview) } else { preview };
    let mut st = conn.prepare_cached("SELECT a.file_name, coalesce(a.extracted_text, '') FROM attachment a WHERE a.message_id = ?1 ORDER BY a.id")?;
    let attachments = st.query_map(params![id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
    Ok(Some(crate::query::HitFacts { body, subject, sender_name, from_address, attachments }))
}
