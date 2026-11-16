//! Typed contact operations: one contact per person, holding the addresses it is known by and a per-account mail count that
//! is recounted on every write. `WriteCtx` methods, so a use case runs them inside its own transaction.
use crate::error::StoreError;
use crate::ops::{json_string, json_strings};
use crate::writer::WriteCtx;
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeMap;

/// A contact as stored (the logo and the counts are the store's to keep).
#[derive(Debug, Clone, PartialEq)]
pub struct ContactRecord {
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
}

fn addresses_of(ctx: &WriteCtx<'_>, json: &str) -> Result<Vec<String>, StoreError> {
    let mut st = ctx.tx.prepare_cached("SELECT value FROM json_each(?1)")?;
    let rows = st.query_map([json], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

impl WriteCtx<'_> {
    /// The oldest contact that lists the address (matched in lower case; pass the address lower-cased).
    pub fn contact_by_address(&self, address_lower: &str) -> Result<Option<ContactRecord>, StoreError> {
        let head = self
            .tx
            .prepare_cached("SELECT c.id, c.name, c.addresses_json FROM contact c, json_each(c.addresses_json) j WHERE lower(j.value) = ?1 ORDER BY c.rowid LIMIT 1")?
            .query_row([address_lower], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .optional()?;
        let Some((id, name, json)) = head else { return Ok(None) };
        Ok(Some(ContactRecord { id, name, addresses: addresses_of(self, &json)? }))
    }

    pub fn contact_get(&self, id: &str) -> Result<Option<ContactRecord>, StoreError> {
        let head = self
            .tx
            .prepare_cached("SELECT id, name, addresses_json FROM contact WHERE id = ?1")?
            .query_row([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
            .optional()?;
        let Some((id, name, json)) = head else { return Ok(None) };
        Ok(Some(ContactRecord { id, name, addresses: addresses_of(self, &json)? }))
    }

    pub fn contacts_all(&self) -> Result<Vec<ContactRecord>, StoreError> {
        let heads: Vec<(String, String, String)> = {
            let mut st = self.tx.prepare_cached("SELECT id, name, addresses_json FROM contact")?;
            let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        heads.into_iter().map(|(id, name, json)| Ok(ContactRecord { id, name, addresses: addresses_of(self, &json)? })).collect()
    }

    /// Write the contact (insert or replace) with a freshly counted per-account mail count.
    pub fn contact_save(&mut self, id: &str, name: &str, addresses: &[String], now_ms: i64) -> Result<(), StoreError> {
        let counts = self.mail_counts(addresses)?;
        let counts_json = format!("{{{}}}", counts.iter().map(|(k, v)| format!("{}:{v}", json_string(k))).collect::<Vec<_>>().join(","));
        self.tx.execute(
            "INSERT INTO contact (id, name, addresses_json, logo_id, mail_count_json, updated_at_ms) VALUES (?1, ?2, ?3, NULL, ?4, ?5) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, addresses_json = excluded.addresses_json, mail_count_json = excluded.mail_count_json, updated_at_ms = excluded.updated_at_ms",
            params![id, name, json_strings(addresses), counts_json, now_ms],
        )?;
        Ok(())
    }

    pub fn contact_delete(&mut self, id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.execute("DELETE FROM contact WHERE id = ?1", params![id])? > 0)
    }

    /// Mail per account over all the given addresses (the match ignores case): the sum is the number of those messages.
    pub fn mail_counts(&self, addresses: &[String]) -> Result<BTreeMap<String, i64>, StoreError> {
        let mut st = self.tx.prepare_cached("SELECT account_id, count(*) FROM message WHERE lower(from_address) IN (SELECT lower(value) FROM json_each(?1)) GROUP BY account_id")?;
        let rows = st.query_map([json_strings(addresses)], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        Ok(rows.collect::<Result<BTreeMap<_, _>, _>>()?)
    }

    /// The name the newest message from this address carries (pass it lower-cased), if any message exists.
    pub fn sender_name_of(&self, address_lower: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .tx
            .prepare_cached("SELECT sender_name FROM message WHERE lower(from_address) = ?1 ORDER BY received_at_ms DESC LIMIT 1")?
            .query_row([address_lower], |r| r.get::<_, String>(0))
            .optional()?)
    }

    /// Every sender address that is not one of the owner's own, lower-cased, each with the name of its newest message.
    pub fn outside_senders(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut st = self.tx.prepare_cached(
            "SELECT lower(m.from_address), (SELECT sender_name FROM message n WHERE lower(n.from_address) = lower(m.from_address) ORDER BY n.received_at_ms DESC LIMIT 1) \
             FROM message m WHERE lower(m.from_address) NOT IN (SELECT lower(address) FROM account) GROUP BY lower(m.from_address)",
        )?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// The ids of every message sent from any of the contact's addresses, newest first. None when the contact does not exist.
    pub fn contact_message_ids(&self, contact_id: &str) -> Result<Option<Vec<String>>, StoreError> {
        let Some(contact) = self.contact_get(contact_id)? else { return Ok(None) };
        let mut st = self.tx.prepare_cached("SELECT id FROM message WHERE lower(from_address) IN (SELECT lower(value) FROM json_each(?1)) ORDER BY received_at_ms DESC")?;
        let rows = st.query_map([json_strings(&contact.addresses)], |r| r.get::<_, String>(0))?;
        Ok(Some(rows.collect::<Result<Vec<_>, _>>()?))
    }
}

// ---------------------------------------------------------------- lane real-notes: MergeContacts

/// What a merge did (`WriteCtx::contacts_merge`).
#[derive(Debug, Clone, PartialEq)]
pub enum MergeResult {
    /// The card that now holds every address of both.
    Merged(ContactRecord),
    /// Both ids name the same card (the id, or an address one card already holds).
    SameCard,
    /// This id names no contact and no person of the stored mail.
    Unknown(String),
}

impl WriteCtx<'_> {
    /// The card an id names: a stored card by its id, or a person of the people list (`person:<address>`: the card that holds the address,
    /// else a card made from the address and the newest sender name; a person nobody wrote or got mail from is unknown).
    fn card_named(&self, id: &str) -> Result<Option<ContactRecord>, StoreError> {
        if let Some(c) = self.contact_get(id)? { return Ok(Some(c)); }
        let Some(address) = id.strip_prefix("person:") else { return Ok(None) };
        let address = address.trim().to_lowercase();
        if address.is_empty() { return Ok(None); }
        if let Some(c) = self.contact_by_address(&address)? { return Ok(Some(c)); }
        let known: i64 = self.tx.query_row(
            "SELECT (SELECT count(*) FROM message WHERE lower(from_address) = ?1) + (SELECT count(*) FROM message m, json_each(m.to_json) j WHERE instr(lower(j.value), ?1) > 0) \
             + (SELECT count(*) FROM message m, json_each(m.cc_json) j WHERE instr(lower(j.value), ?1) > 0)", [&address], |r| r.get(0))?;
        if known == 0 { return Ok(None); }
        let name = self.sender_name_of(&address)?.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| address.clone());
        Ok(Some(ContactRecord { id: id.to_string(), name, addresses: vec![address] }))
    }

    /// Two cards become one: `keep_id`'s card (its id and its name) takes every address of `merge_id`'s, the merged card goes, the per-account mail
    /// counts are recounted over all the addresses. No message is changed: a mail's sender stays what it was, and the card finds it by any of the addresses.
    pub fn contacts_merge(&mut self, keep_id: &str, merge_id: &str, now_ms: i64) -> Result<MergeResult, StoreError> {
        let Some(keep) = self.card_named(keep_id)? else { return Ok(MergeResult::Unknown(keep_id.to_string())) };
        let Some(gone) = self.card_named(merge_id)? else { return Ok(MergeResult::Unknown(merge_id.to_string())) };
        if keep.id == gone.id { return Ok(MergeResult::SameCard); }
        let mut addresses = keep.addresses.clone();
        for a in &gone.addresses { if !addresses.iter().any(|k| k.eq_ignore_ascii_case(a)) { addresses.push(a.clone()); } }
        self.contact_save(&keep.id, &keep.name, &addresses, now_ms)?;
        self.contact_delete(&gone.id)?;
        let merged = self.contact_get(&keep.id)?.unwrap_or(ContactRecord { id: keep.id, name: keep.name, addresses });
        Ok(MergeResult::Merged(merged))
    }
}

impl crate::Store {
    pub fn contacts_merge(&self, keep_id: String, merge_id: String, now_ms: i64) -> Result<MergeResult, StoreError> { self.write(move |ctx| ctx.contacts_merge(&keep_id, &merge_id, now_ms)) }

    /// fix-blockforget (the contract's `contact_messages`): the ids of the mail a contact sent, newest first. The id is a stored card (every
    /// address it holds) or a person of the people list, `person:<address>` (the card that holds the address, so a merged person lists the
    /// mail of all its addresses; else that address alone). An unknown id, or a person id with no address, has none. One read, no write.
    pub fn contact_mail_ids(&self, contact_id: &str) -> Result<Vec<String>, StoreError> {
        let id = contact_id.to_string();
        self.read(move |c| {
            let card: Option<String> = c.prepare_cached("SELECT addresses_json FROM contact WHERE id = ?1")?.query_row([&id], |r| r.get(0)).optional()?;
            let addresses = match card {
                Some(json) => json,
                None => {
                    let Some(address) = id.strip_prefix("person:").map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()) else { return Ok(Vec::new()) };
                    let held: Option<String> = c
                        .prepare_cached("SELECT c.addresses_json FROM contact c, json_each(c.addresses_json) j WHERE lower(j.value) = ?1 ORDER BY c.rowid LIMIT 1")?
                        .query_row([&address], |r| r.get(0))
                        .optional()?;
                    held.unwrap_or_else(|| json_strings(&[address]))
                }
            };
            let mut st = c.prepare_cached("SELECT id FROM message WHERE lower(from_address) IN (SELECT lower(value) FROM json_each(?1)) ORDER BY received_at_ms DESC, id")?;
            let rows = st.query_map([&addresses], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
}
