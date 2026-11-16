//! contracts delta 15: an account's signature in the store. The `signature` table exists since v1 (address, text,
//! updated_at_ms), so no migration is needed: one row per sending address, an empty text removes the row.
use crate::{Store, StoreError};
use contracts::Signature;
use rusqlite::params;

impl Store {
    /// Keeps the signature of the address (lower case). An empty or blank text removes it: the account then signs nothing.
    pub fn set_signature(&self, address: &str, text: &str, now_ms: i64) -> Result<(), StoreError> {
        let a = address.trim().to_lowercase();
        let t = text.trim_end().to_string();
        self.write(move |ctx| {
            if t.trim().is_empty() {
                ctx.tx.execute("DELETE FROM signature WHERE address = ?1", params![a])?;
            } else {
                ctx.tx.execute(
                    "INSERT INTO signature (address, text, updated_at_ms) VALUES (?1, ?2, ?3) \
                     ON CONFLICT(address) DO UPDATE SET text = excluded.text, updated_at_ms = excluded.updated_at_ms",
                    params![a, t, now_ms],
                )?;
            }
            Ok(())
        })
    }

    /// The signature of the address (any case), or None.
    pub fn signature(&self, address: &str) -> Result<Option<Signature>, StoreError> {
        let a = address.trim().to_lowercase();
        self.read(move |c| {
            use rusqlite::OptionalExtension;
            Ok(c.query_row("SELECT address, text FROM signature WHERE address = ?1", params![a], |r| {
                Ok(Signature { address: r.get(0)?, text: r.get(1)? })
            }).optional()?)
        })
    }
}

/// real-compose: the reads and the one write the engine's compose commands (`SetSignature`, `ReplyAll`) and their state paths need.
/// A draft's header fields, in the order the `draft/<id>/...` state paths read them: (from account id, to, cc, subject, reply-to thread).
pub type DraftFields = (String, Vec<String>, Vec<String>, String, Option<String>);

impl Store {
    /// `SetSignature { account_id, text }`: the signature of the account's address. The text is trimmed at both ends and an empty
    /// one clears it (the test engine's rule). Ok(false) when no account has that id (nothing is written).
    pub fn set_account_signature(&self, account_id: &str, text: &str, now_ms: i64) -> Result<bool, StoreError> {
        let id = account_id.to_string();
        let address: Option<String> = self.read(move |c| {
            use rusqlite::OptionalExtension;
            Ok(c.query_row("SELECT address FROM account WHERE id = ?1", params![id], |r| r.get(0)).optional()?)
        })?;
        let Some(address) = address else { return Ok(false) };
        self.set_signature(&address, text.trim(), now_ms)?;
        Ok(true)
    }

    /// Each account's saved signature, by account id (one read; an account with none is absent). The accounts list shows it.
    pub fn signatures_by_account(&self) -> Result<std::collections::HashMap<String, String>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT a.id, s.text FROM account a JOIN signature s ON s.address = lower(a.address)")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            Ok(rows.collect::<Result<std::collections::HashMap<_, _>, _>>()?)
        })
    }

    /// Every address the owner writes from (one per account): Reply all leaves them out.
    pub fn own_addresses(&self) -> Result<Vec<String>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT address FROM account ORDER BY sort_order, id")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// The header fields of a saved draft; None when no such draft is open.
    pub fn draft_fields(&self, draft_id: &str) -> Result<Option<DraftFields>, StoreError> {
        let id = draft_id.to_string();
        self.read(move |c| {
            use rusqlite::OptionalExtension;
            let row = c.query_row("SELECT from_account_id, to_json, cc_json, subject, reply_to_thread_id FROM draft WHERE id = ?1", params![id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?))).optional()?;
            let Some((from_account_id, to, cc, subject, reply_to_thread_id)) = row else { return Ok(None) };
            let list = |j: String| -> Result<Vec<String>, StoreError> {
                let mut st = c.prepare_cached("SELECT value FROM json_each(?1) ORDER BY key")?;
                let rows = st.query_map(params![j], |r| r.get::<_, String>(0))?;
                Ok(rows.collect::<Result<Vec<_>, _>>()?)
            };
            Ok(Some((from_account_id, list(to)?, list(cc)?, subject, reply_to_thread_id)))
        })
    }

    /// How many drafts are saved.
    pub fn draft_count(&self) -> Result<i64, StoreError> {
        self.read(|c| Ok(c.query_row("SELECT count(*) FROM draft", [], |r| r.get(0))?))
    }
}
