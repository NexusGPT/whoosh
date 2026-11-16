//! engine-act-r5: where the SERVER's copy of a message sits after the core moved it. The row keeps the place the core wrote locally
//! (folder INBOX with `archived = 1`, or the local word Spam), because the lists and the UI read that; the server's own copy is in the
//! Archive or Junk folder under a new UID, which the server names in its MOVE answer (RFC 4315 COPYUID). That place is one
//! `message_location` row per message (the table of "a copy of this message in that folder"): written when the move is applied, read
//! when the way back is built, dropped when the message is back in its INBOX. UIDs are per folder, so a place is the pair
//! (folder, UID) and never a UID alone.
use crate::error::StoreError;
use crate::writer::WriteCtx;
use crate::Store;
use rusqlite::{params, OptionalExtension};

/// The server folder holding a message's copy and the UID it has there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerPlace {
    pub folder: String,
    pub uid: u32,
}

impl WriteCtx<'_> {
    /// The message's copy now sits in `folder` under `uid` (the server's COPYUID answer). A message has one such place: an older
    /// one is replaced. A message the store does not hold is left alone.
    pub fn place_record(&mut self, message_id: &str, account_id: &str, folder: &str, uid_validity: u32, uid: u32) -> Result<bool, StoreError> {
        if !self.tx.prepare_cached("SELECT 1 FROM message WHERE id = ?1")?.exists(params![message_id])? { return Ok(false); }
        self.tx.execute("DELETE FROM message_location WHERE message_id = ?1", params![message_id])?;
        self.tx.execute(
            "INSERT INTO message_location (account_id, folder, uid_validity, uid, message_id) VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(account_id, folder, uid_validity, uid) DO UPDATE SET message_id = excluded.message_id",
            params![account_id, folder, uid_validity, uid, message_id],
        )?;
        Ok(true)
    }

    /// The place the server's copy of this message sits, when a move recorded one.
    pub fn place_of(&self, message_id: &str) -> Result<Option<ServerPlace>, StoreError> {
        place_query(&self.tx, message_id)
    }

    /// The message is back in its INBOX under `uid`: its recorded place goes, and the row's server id is the INBOX UID (the old one
    /// belonged to the copy the server moved away, and a UID of another folder would collide in `message`'s unique key).
    /// Answers whether the row now carries the UID. A UID another row of the account's INBOX already holds is refused, not forced.
    pub fn place_back_in_inbox(&mut self, message_id: &str, uid: u32) -> Result<bool, StoreError> {
        self.tx.execute("DELETE FROM message_location WHERE message_id = ?1", params![message_id])?;
        let n = self.tx.execute("UPDATE OR IGNORE message SET remote_id = ?2 WHERE id = ?1 AND folder = ?3", params![message_id, uid.to_string(), crate::INBOX_FOLDER])?;
        Ok(n == 1)
    }

    /// UIDs belong to a folder: a message that goes to another folder keeps none of the old folder's, or the move collides with a
    /// message of the target that has the same number. The server names the new one (the COPYUID answer, or the sync's re-read).
    pub fn clear_remote_ids(&mut self, message_ids: &[String]) -> Result<usize, StoreError> {
        let mut st = self.tx.prepare_cached("UPDATE message SET remote_id = NULL WHERE id = ?1 AND remote_id IS NOT NULL")?;
        let mut n = 0;
        for id in message_ids { n += st.execute(params![id])?; }
        Ok(n)
    }
}

fn place_query(c: &rusqlite::Connection, message_id: &str) -> Result<Option<ServerPlace>, StoreError> {
    Ok(c.prepare_cached("SELECT folder, uid FROM message_location WHERE message_id = ?1 ORDER BY rowid DESC LIMIT 1")?
        .query_row(params![message_id], |r| Ok(ServerPlace { folder: r.get(0)?, uid: r.get::<_, i64>(1)? as u32 }))
        .optional()?)
}

impl Store {
    /// `WriteCtx::place_of`, for a caller outside a transaction (the write-back worker).
    pub fn message_place(&self, message_id: &str) -> Result<Option<ServerPlace>, StoreError> {
        self.read(|c| place_query(c, message_id))
    }
}
