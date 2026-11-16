//! The real core, slice 1: an account added from Add account (detected or typed configuration, the password already in
//! the Keychain), and the accounts read back for the lists. Slice 2 (2026-11-09): the mail reads the real core answers
//! from what the IMAP sync stored (one message, its body, the mailbox lists, the list filters' facts, the sync states,
//! the accounts to sync and each folder's IMAP position). Every table and column exists already; no migration.
use std::collections::HashMap;

use crate::{Store, StoreError};
use contracts::{Account, AccountConfig, AccountSummary, InboxRow, ListFilterFacts, MailProvider, Message};
use rusqlite::{params, Connection, OptionalExtension};

/// The colours a new account takes in turn (the fixture's first five), so two new accounts never look the same.
const COLOURS: [&str; 5] = ["#3B82F6", "#10B981", "#F59E0B", "#EF4444", "#8B5CF6"];

fn provider_word(p: MailProvider) -> &'static str {
    match p { MailProvider::Google => "google", MailProvider::Microsoft => "microsoft", MailProvider::Icloud => "icloud", MailProvider::Fastmail => "fastmail", MailProvider::Imap => "imap" }
}
fn provider_from(w: &str) -> MailProvider {
    match w { "google" => MailProvider::Google, "microsoft" => MailProvider::Microsoft, "icloud" => MailProvider::Icloud, "fastmail" => MailProvider::Fastmail, _ => MailProvider::Imap }
}

impl Store {
    /// Adds an account and its server configuration in one transaction. `keychain_ref` is the secrets crate's
    /// reference (`service` + char 31 + `account`): the password itself is never here. Refused in words when the
    /// address is already an account. Returns the new account id.
    pub fn add_account(&self, address: &str, display_name: &str, provider: MailProvider, keychain_ref: &str, oauth: bool, config: AccountConfig, now_ms: i64) -> Result<String, StoreError> {
        let address = address.trim().to_lowercase();
        let name = if display_name.trim().is_empty() { address.clone() } else { display_name.trim().to_string() };
        let id = crate::new_id("acc");
        let (aid, kref) = (id.clone(), keychain_ref.to_string());
        self.write(move |ctx| {
            let taken: bool = ctx.tx.query_row("SELECT EXISTS (SELECT 1 FROM account WHERE lower(address) = ?1)", params![address], |r| r.get(0))?;
            if taken { return Err(StoreError::Invalid(format!("{address} is already an account on this Mac"))); }
            let n: i64 = ctx.tx.query_row("SELECT count(*) FROM account", [], |r| r.get(0))?;
            // real-accounts: after the last one even when an account was removed (the order never collides), and the first palette colour no
            // account wears (a fresh store gives the palette in turn, as before; after a removal the freed colour is the next one taken)
            let after: i64 = ctx.tx.query_row("SELECT coalesce(max(sort_order) + 1, 0) FROM account", [], |r| r.get(0))?;
            let used: Vec<String> = { let mut st = ctx.tx.prepare("SELECT upper(colour) FROM account")?; let rows = st.query_map([], |r| r.get::<_, String>(0))?; rows.collect::<Result<Vec<_>, _>>()? };
            let colour = COLOURS.iter().find(|c| !used.iter().any(|u| u.eq_ignore_ascii_case(c))).copied().unwrap_or(COLOURS[(n as usize) % COLOURS.len()]);
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![aid, address, provider_word(provider), name, colour, after, if oauth { "oauth" } else { "manual" }, kref, now_ms],
            )?;
            Ok(())
        })?;
        self.set_account_config(id.clone(), config)?;
        Ok(id)
    }

    /// Every account in the owner's order, as the lists show them, with its unread count in the inbox (slice 2). Health is
    /// the sync state's error; the real core fills it from `sync_state_rows` (the failure's JSON is parsed by sync-engine).
    pub fn account_summaries(&self) -> Result<Vec<AccountSummary>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached(&format!(
                "SELECT a.id, a.display_name, a.address, a.provider, a.colour, a.icon_id, a.group_name, a.paused, \
                        (SELECT count(*) FROM message m WHERE m.account_id = a.id AND m.read = 0 AND {}), \
                        (SELECT f.local_path FROM account_icon_file f WHERE f.account_id = a.id) \
                 FROM account a ORDER BY a.sort_order, a.id", crate::read::VISIBLE))?;
            let rows = st.query_map([], |r| {
                Ok(AccountSummary {
                    id: r.get(0)?, name: r.get(1)?, address: r.get(2)?, provider: provider_from(&r.get::<_, String>(3)?), colour: r.get(4)?,
                    icon_id: r.get(5)?, unread: r.get::<_, i64>(8)?.max(0) as u32, health: None, group: r.get(6)?, paused: r.get::<_, i64>(7)? != 0,
                    backfill: None, signature: String::new(), icon_file: r.get(9)?,
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
}

/// real-accounts (Settings > Accounts): the account commands on the store. Every write is one transaction through the store's writer; every
/// function answers `Ok(false)` for an account that does not exist (the engine says so in words), never a SQL error.
impl Store {
    /// Whether this account is stored.
    pub fn account_exists(&self, account_id: &str) -> Result<bool, StoreError> {
        self.read(|c| Ok(c.prepare_cached("SELECT EXISTS (SELECT 1 FROM account WHERE id = ?1)")?.query_row(params![account_id], |r| r.get::<_, bool>(0))?))
    }

    /// Every account id in the owner's order.
    pub fn account_ids(&self) -> Result<Vec<String>, StoreError> {
        self.read(|c| { let mut st = c.prepare_cached("SELECT id FROM account ORDER BY sort_order, id")?; let rows = st.query_map([], |r| r.get::<_, String>(0))?; Ok(rows.collect::<Result<Vec<_>, _>>()?) })
    }

    /// The account's display name (RenameAccount). The caller trims and refuses an empty name.
    pub fn rename_account(&self, account_id: &str, name: &str, now_ms: i64) -> Result<bool, StoreError> {
        let (id, name) = (account_id.to_string(), name.to_string());
        self.write(move |ctx| Ok(ctx.tx.execute("UPDATE account SET display_name = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, name, now_ms])? > 0))
    }

    /// Pauses or resumes the account (PauseAccount): the flag the sync's launch and the outbox read. Nothing of the account's mail is touched.
    pub fn set_account_paused(&self, account_id: &str, paused: bool, now_ms: i64) -> Result<bool, StoreError> {
        let id = account_id.to_string();
        self.write(move |ctx| Ok(ctx.tx.execute("UPDATE account SET paused = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, paused as i64, now_ms])? > 0))
    }

    /// The account's colour, "#RRGGBB" (SetAccountColour); the schema's own check refuses any other length.
    pub fn set_account_colour(&self, account_id: &str, colour: &str, now_ms: i64) -> Result<bool, StoreError> {
        let (id, colour) = (account_id.to_string(), colour.to_string());
        self.write(move |ctx| Ok(ctx.tx.execute("UPDATE account SET colour = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, colour, now_ms])? > 0))
    }

    /// Picks a library icon (SetAccountIcon Library): the glyph id is kept and an upload the account had is forgotten, so the glyph is what shows.
    pub fn set_account_library_icon(&self, account_id: &str, icon_id: &str, now_ms: i64) -> Result<bool, StoreError> {
        let (id, icon) = (account_id.to_string(), icon_id.to_string());
        self.write(move |ctx| {
            let n = ctx.tx.execute("UPDATE account SET icon_id = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, icon, now_ms])?;
            if n > 0 { ctx.tx.execute("DELETE FROM account_icon_file WHERE account_id = ?1", params![id])?; }
            Ok(n > 0)
        })
    }

    /// Keeps the owner's own image (SetAccountIcon Uploaded): the file the app already holds on this Mac. The library glyph stays as it was, so it
    /// is there again when the owner goes back to the library.
    pub fn set_account_uploaded_icon(&self, account_id: &str, file_name: &str, mime_type: &str, size_bytes: u64, local_path: &str, now_ms: i64) -> Result<bool, StoreError> {
        let (id, name, mime, path) = (account_id.to_string(), file_name.to_string(), mime_type.to_string(), local_path.to_string());
        self.write(move |ctx| {
            let there: bool = ctx.tx.query_row("SELECT EXISTS (SELECT 1 FROM account WHERE id = ?1)", params![id], |r| r.get(0))?;
            if !there { return Ok(false); }
            ctx.tx.execute(
                "INSERT INTO account_icon_file (account_id, file_name, mime_type, size_bytes, local_path, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(account_id) DO UPDATE SET file_name = excluded.file_name, mime_type = excluded.mime_type, size_bytes = excluded.size_bytes, local_path = excluded.local_path, updated_at_ms = excluded.updated_at_ms",
                params![id, name, mime, size_bytes as i64, path, now_ms],
            )?;
            ctx.tx.execute("UPDATE account SET updated_at_ms = ?2 WHERE id = ?1", params![id, now_ms])?;
            Ok(true)
        })
    }

    /// The owner's order (ReorderAccounts): `ids` must hold every account exactly once, else `Invalid` in words. One transaction.
    pub fn reorder_accounts(&self, ids: &[String], now_ms: i64) -> Result<(), StoreError> {
        let ids = ids.to_vec();
        self.write(move |ctx| {
            let all: Vec<String> = { let mut st = ctx.tx.prepare("SELECT id FROM account")?; let rows = st.query_map([], |r| r.get::<_, String>(0))?; rows.collect::<Result<Vec<_>, _>>()? };
            let mut want = ids.clone(); want.sort(); want.dedup();
            let mut have = all; have.sort();
            if want.len() != ids.len() || want != have { return Err(StoreError::Invalid("the order must list every account once".into())); }
            for (i, id) in ids.iter().enumerate() {
                ctx.tx.execute("UPDATE account SET sort_order = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, i as i64, now_ms])?;
            }
            Ok(())
        })
    }

    /// What removing the account takes off this Mac, read before RemoveAccount (`WhooshCore::account_removal`); None for an unknown account.
    pub fn account_removal(&self, account_id: &str) -> Result<Option<contracts::AccountRemoval>, StoreError> {
        self.read(|c| {
            let name: Option<String> = c.prepare_cached("SELECT display_name FROM account WHERE id = ?1")?.query_row(params![account_id], |r| r.get(0)).optional()?;
            let Some(name) = name else { return Ok(None) };
            let n = |sql: &str| -> Result<u32, StoreError> { Ok(c.prepare_cached(sql)?.query_row(params![account_id], |r| r.get::<_, i64>(0))?.max(0) as u32) };
            let messages = n("SELECT count(*) FROM message WHERE account_id = ?1")?;
            let drafts = n("SELECT count(*) FROM draft WHERE from_account_id = ?1")?;
            let outbox = n("SELECT count(*) FROM outbox_entry WHERE state IN ('undoable','scheduled','sending','failed') AND json_extract(draft_json, '$.draft.from_account_id') = ?1")?;
            let labels = n("SELECT count(*) FROM label WHERE scope_account_id = ?1")?;
            let words = contracts::account_removal_words(name.clone(), messages, drafts, outbox, labels);
            Ok(Some(contracts::AccountRemoval { account_id: account_id.to_string(), account_name: name, messages, drafts, outbox, labels, words }))
        })
    }

    /// The Keychain reference of the account's credential and whether it signs in by OAuth; None for an unknown account.
    pub fn account_credential_ref(&self, account_id: &str) -> Result<Option<(String, bool)>, StoreError> {
        self.read(|c| Ok(c.prepare_cached("SELECT keychain_ref, credential_kind FROM account WHERE id = ?1")?
            .query_row(params![account_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)? == "oauth"))).optional()?))
    }

    /// Takes the account off this Mac (RemoveAccount), in ONE transaction: its messages and threads, its drafts, what waits to send from it, its
    /// pending server operations, its folders' sync positions, its sync state, its icon, its notification setting, and the labels that belong to it
    /// alone. Nothing of another account is read or written. The outbox rows hold the account in their JSON (no foreign key), so they are deleted
    /// by it here. Returns false for an unknown account. The Keychain item is the engine's to remove (the store never sees a secret).
    pub fn remove_account(&self, account_id: &str) -> Result<bool, StoreError> {
        let id = account_id.to_string();
        self.write(move |ctx| {
            let there: bool = ctx.tx.query_row("SELECT EXISTS (SELECT 1 FROM account WHERE id = ?1)", params![id], |r| r.get(0))?;
            if !there { return Ok(false); }
            // fix-blockforget: an undo entry that names a message or a queued send of this account goes too (Z would answer an error on it)
            ctx.tx.execute("DELETE FROM undo_entry WHERE EXISTS (SELECT 1 FROM json_tree(undo_entry.action_json) j WHERE j.type = 'text' AND (j.value IN (SELECT id FROM message WHERE account_id = ?1) OR j.value IN (SELECT id FROM outbox_entry WHERE json_extract(draft_json, '$.draft.from_account_id') = ?1)))", params![id])?;
            ctx.tx.execute("DELETE FROM outbox_entry WHERE json_extract(draft_json, '$.draft.from_account_id') = ?1", params![id])?;
            ctx.tx.execute("DELETE FROM draft WHERE from_account_id = ?1", params![id])?;
            ctx.tx.execute("DELETE FROM pending_action WHERE account_id = ?1", params![id])?;
            ctx.tx.execute("DELETE FROM message WHERE account_id = ?1", params![id])?;
            ctx.tx.execute("DELETE FROM thread WHERE account_id = ?1", params![id])?;
            // the account row last: its foreign keys take the rest (sync state, folder positions, icon file, locations, notification setting, junk
            // model) and the trigger `label_goes_with_its_mailbox` takes the labels scoped to it
            ctx.tx.execute("DELETE FROM account WHERE id = ?1", params![id])?;
            Ok(true)
        })
    }

    /// fix-blockforget: what the search indexes hold of an account, read BEFORE its removal (the rows go with it): the ids of its messages and
    /// the ids of their chunks (body and file pieces; each chunk's vector is filed under `chunk_vector_key(chunk id)`). One read.
    pub fn account_index_rows(&self, account_id: &str) -> Result<(Vec<String>, Vec<i64>), StoreError> {
        let id = account_id.to_string();
        self.read(move |c| {
            let messages = c.prepare_cached("SELECT id FROM message WHERE account_id = ?1")?.query_map([&id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            let chunks = c.prepare_cached("SELECT k.id FROM chunk k JOIN message m ON m.id = k.message_id WHERE m.account_id = ?1")?.query_map([&id], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
            Ok((messages, chunks))
        })
    }
}

/// One account's stored sync state, as the table holds it: the failure stays JSON (sync-engine's `parse_failure` reads it,
/// the store does not depend on the sync crate). An account the sync never touched reads as nothing fetched yet.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncStateRow {
    pub account_id: String,
    pub history_fetched: bool,
    pub messages_indexed: u64,
    pub last_error_json: Option<String>,
    pub provider_status_words: Option<String>,
    pub last_synced_ms: Option<i64>,
}

/// An account the real core syncs: the record, its server configuration and the Keychain reference of its credential.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncAccount {
    pub account: Account,
    pub config: Option<AccountConfig>,
    pub keychain_ref: String,
    pub oauth: bool,
}

/// What a message's body read needs: the stored body (text, sanitised HTML, trackers) and where to fetch it when the sync
/// kept only the preview (its folder and its IMAP UID, kept as `remote_id`).
#[derive(Debug, Clone, PartialEq)]
pub struct BodyParts {
    pub message_id: String,
    pub account_id: String,
    pub folder: String,
    pub remote_id: Option<String>,
    pub preview: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub trackers_blocked: u32,
    /// migration 0019 (delta 26, table `message_prepared`): the mail content engine's result as JSON, written with the body at fetch; None
    /// before 0019, before the fetch, or for a body kept without it (`set_body`).
    pub prepared: Option<String>,
}

/// delta 26 (migration 0019): the mail content engine's result of one kept body, in the same transaction as the body (None: no row).
fn keep_prepared(tx: &rusqlite::Transaction<'_>, message_id: &str, prepared: Option<&str>) -> Result<(), StoreError> {
    match prepared {
        Some(p) => { tx.prepare_cached("INSERT INTO message_prepared (message_id, prepared) VALUES (?1, ?2) ON CONFLICT(message_id) DO UPDATE SET prepared = excluded.prepared")?.execute(params![message_id, p])?; }
        None => { tx.prepare_cached("DELETE FROM message_prepared WHERE message_id = ?1")?.execute(params![message_id])?; }
    }
    Ok(())
}

/// The strings of a JSON array column, read by SQLite itself (the store has no JSON library). A NULL is an empty list.
fn strings(c: &Connection, json: Option<String>) -> Result<Vec<String>, StoreError> {
    let Some(j) = json else { return Ok(Vec::new()) };
    let mut st = c.prepare_cached("SELECT value FROM json_each(?1) ORDER BY key")?;
    let rows = st.query_map(params![j], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The newest `limit` ids of a WHERE clause over `message m`, newest first.
fn newest_ids(c: &Connection, where_sql: &str, args: &[&dyn rusqlite::ToSql], limit: usize) -> Result<Vec<String>, StoreError> {
    let mut st = c.prepare(&format!("SELECT m.id FROM message m WHERE {where_sql} ORDER BY m.received_at_ms DESC, m.id LIMIT {}", limit as i64))?;
    let rows = st.query_map(args, |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// `AND <alias>.account_id IN (...)` for an account set (None: every account), as literal quoted ids (ids are the store's own).
fn accounts_clause_on(alias: &str, accounts: Option<&[String]>) -> String {
    match accounts {
        None => String::new(),
        Some(a) => format!(" AND {alias}.account_id IN ({})", a.iter().map(|x| format!("'{}'", x.replace('\'', "''"))).collect::<Vec<_>>().join(",")),
    }
}

fn accounts_clause(accounts: Option<&[String]>) -> String { accounts_clause_on("m", accounts) }

impl Store {
    /// One stored message in the contract's shape. None when no such id is stored.
    pub fn message(&self, message_id: &str) -> Result<Option<Message>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| {
            let row = c.query_row(
                "SELECT m.id, m.thread_id, m.account_id, m.sender_name, m.from_address, m.to_json, m.subject, m.preview, m.received_at_ms, \
                        m.read, m.archived, m.folder, m.login_code, m.trackers_blocked, m.list_unsubscribe_json, m.list_unsubscribe_one_click, \
                        m.cc_json, m.bcc_json, lower(m.from_address) = lower(a.address) \
                 FROM message m JOIN account a ON a.id = m.account_id WHERE m.id = ?1",
                params![id],
                |r| Ok((
                    (r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?),
                    (r.get::<_, String>(5)?, r.get::<_, String>(6)?, r.get::<_, String>(7)?, r.get::<_, i64>(8)?, r.get::<_, i64>(9)? != 0),
                    (r.get::<_, i64>(10)? != 0, r.get::<_, String>(11)?, r.get::<_, Option<String>>(12)?, r.get::<_, i64>(13)?.max(0) as u32),
                    (r.get::<_, Option<String>>(14)?, r.get::<_, i64>(15)? != 0, r.get::<_, String>(16)?, r.get::<_, String>(17)?, r.get::<_, bool>(18)?),
                )),
            ).optional()?;
            let Some(((id, thread_id, account_id, sender_name, from_address), (to, subject, preview, received_at_ms, read), (archived, folder, login_code, trackers_blocked), (lu, one_click, cc, bcc, outgoing))) = row else { return Ok(None) };
            let sender_name = crate::read::clean_sender_name(&sender_name);   // without the quotes a sender wraps it in; empty stays empty (the reader then shows the address)
            let mut st = c.prepare_cached("SELECT a.file_name FROM attachment a WHERE a.message_id = ?1 AND NOT (a.inline OR coalesce(json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.inline'), 0)) ORDER BY a.file_name")?;   // files only: never a picture shown inside the text, never an invitation's calendar data (picture 212)
            let attachment_names = st.query_map(params![id], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            let reply_to_name = crate::reply_to::first_reply_to_name(c, &id)?; // 0027: the stored Reply-To's first name
            Ok(Some(Message {
                to: strings(c, Some(to))?, cc: strings(c, Some(cc))?, bcc: strings(c, Some(bcc))?, list_unsubscribe: strings(c, lu)?,
                id, thread_id, account_id, sender_name, from_address, subject, preview, received_at_ms, read, archived, folder,
                attachment_names, login_code, trackers_blocked, list_unsubscribe_one_click: one_click, reply_to_name, is_outgoing: outgoing,
            }))
        })
    }

    /// The stored body of one message and where its full text lives on the server. None when no such id is stored.
    pub fn body_parts(&self, message_id: &str) -> Result<Option<BodyParts>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| {
            Ok(c.query_row(
                "SELECT m.id, m.account_id, m.folder, m.remote_id, m.preview, m.body_text, m.body_html, m.trackers_blocked, p.prepared \
                 FROM message m LEFT JOIN message_prepared p ON p.message_id = m.id WHERE m.id = ?1",
                params![id],
                |r| Ok(BodyParts {
                    message_id: r.get(0)?, account_id: r.get(1)?, folder: r.get(2)?, remote_id: r.get(3)?, preview: r.get(4)?,
                    body_text: r.get(5)?, body_html: r.get(6)?, trackers_blocked: r.get::<_, i64>(7)?.max(0) as u32, prepared: r.get(8)?,
                }),
            ).optional()?)
        })
    }

    /// Keeps a body fetched on demand: the plain text, the SANITISED html (never the raw part) and the trackers it blocked.
    /// Written once; a second open reads it back without the server.
    /// body-r1: the row's `preview` is rewritten from the parsed text in the same write, so a preview the sync stored raw
    /// (a multipart message's boundary and part headers, before body-r1) is repaired by the body that arrives, without a migration.
    pub fn set_body(&self, message_id: &str, text: String, safe_html: Option<String>, trackers_blocked: u32, preview: String) -> Result<bool, StoreError> {
        let kept = BodyParts { message_id: message_id.to_string(), account_id: String::new(), folder: String::new(), remote_id: None, preview,
                               body_text: Some(text), body_html: safe_html, trackers_blocked, prepared: None };
        Ok(self.set_bodies(vec![kept])? == 1)
    }

    /// body-r1: several kept bodies in ONE transaction (the background prefetch stores a batch at a time, so the list and the
    /// counts are announced once per batch, not once per body). Reads `message_id`, `body_text`, `body_html`, `trackers_blocked`
    /// and `preview` of each; the location fields are ignored. Answers how many rows were written.
    pub fn set_bodies(&self, bodies: Vec<BodyParts>) -> Result<u32, StoreError> {
        if bodies.is_empty() { return Ok(0); }
        self.write(move |ctx| {
            let mut st = ctx.tx.prepare_cached("UPDATE message SET body_text = ?2, body_html = ?3, trackers_blocked = ?4, preview = ?5 WHERE id = ?1")?;
            let mut n = 0u32;
            for b in bodies {
                let one = st.execute(params![b.message_id, b.body_text.unwrap_or_default(), b.body_html, b.trackers_blocked as i64, b.preview])?;
                // delta 26: the prepared form goes with the body it was made from (a body kept without one drops a stale one)
                if one == 1 { keep_prepared(ctx.tx, &b.message_id, b.prepared.as_deref())?; }
                n += one as u32;
            }
            Ok(n)
        })
    }

    /// junk-r3: of `addresses`, the ones the owner has written to from this account: a stored message FROM the account's own address
    /// names it in To, Cc or Bcc (a reply, a first mail, a sent copy synced from the server's Sent folder). Lower-case in and out. The spam
    /// filter never moves such a sender's mail on its score alone (a false positive on real correspondence is the worst outcome).
    pub fn written_to(&self, account_id: &str, addresses: &[String]) -> Result<std::collections::HashSet<String>, StoreError> {
        if addresses.is_empty() { return Ok(Default::default()); }
        let a = account_id.to_string();
        let wanted = format!("[{}]", addresses.iter().map(|x| format!("\"{}\"", x.to_ascii_lowercase().replace('\\', "\\\\").replace('"', "\\\""))).collect::<Vec<_>>().join(","));
        self.read(move |c| {
            let mut out = std::collections::HashSet::new();
            for col in ["to_json", "cc_json", "bcc_json"] {
                let mut st = c.prepare_cached(&format!(
                    "SELECT DISTINCT lower(j.value) FROM message m JOIN account acc ON acc.id = m.account_id, json_each(m.{col}) j \
                     WHERE m.account_id = ?1 AND lower(m.from_address) = lower(acc.address) AND lower(j.value) IN (SELECT value FROM json_each(?2))"))?;
                for row in st.query_map(params![a, wanted], |r| r.get::<_, String>(0))? { out.insert(row?); }
            }
            Ok(out)
        })
    }

    /// body-r1: of the newest `limit` messages of one folder (by arrival), the ones with no body stored yet, newest first, as
    /// (IMAP UID, message id): what the background prefetch downloads. Rows without a UID (imported mail) are never listed.
    pub fn bodies_missing(&self, account_id: &str, folder: &str, limit: usize) -> Result<Vec<(u32, String)>, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT remote_id, id FROM (SELECT remote_id, id, body_text, body_html, received_at_ms FROM message \
                 WHERE account_id = ?1 AND folder = ?2 ORDER BY received_at_ms DESC, id LIMIT ?3) \
                 WHERE body_text IS NULL AND body_html IS NULL AND remote_id IS NOT NULL ORDER BY received_at_ms DESC, id")?;
            let rows = st.query_map(params![a, f, limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for row in rows {
                let (uid, id) = row?;
                if let Ok(u) = uid.parse::<u32>() { out.push((u, id)); }
            }
            Ok(out)
        })
    }

    /// Every account's sync state, in the owner's account order (an account with no row yet reads as nothing fetched).
    pub fn sync_state_rows(&self) -> Result<Vec<SyncStateRow>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached(
                "SELECT a.id, COALESCE(s.history_fetched, 0), COALESCE(s.messages_indexed, 0), s.last_error_json, s.provider_status_words, s.last_synced_ms \
                 FROM account a LEFT JOIN account_sync_state s ON s.account_id = a.id ORDER BY a.sort_order, a.id")?;
            let rows = st.query_map([], |r| Ok(SyncStateRow {
                account_id: r.get(0)?, history_fetched: r.get::<_, i64>(1)? != 0, messages_indexed: r.get::<_, i64>(2)?.max(0) as u64,
                last_error_json: r.get(3)?, provider_status_words: r.get(4)?, last_synced_ms: r.get(5)?,
            }))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// The accounts the real core syncs at launch, with their configuration and credential reference.
    pub fn sync_accounts(&self) -> Result<Vec<SyncAccount>, StoreError> {
        let heads: Vec<(Account, String, bool)> = self.read(|c| {
            let mut st = c.prepare_cached("SELECT id, address, provider, display_name, colour, sort_order, paused, group_name, icon_id, keychain_ref, credential_kind FROM account ORDER BY sort_order, id")?;
            let rows = st.query_map([], |r| Ok((Account {
                id: r.get(0)?, address: r.get(1)?, provider: provider_from(&r.get::<_, String>(2)?), display_name: r.get(3)?, colour: r.get(4)?,
                order: r.get::<_, i64>(5)?.max(0) as u32, paused: r.get::<_, i64>(6)? != 0, group: r.get(7)?, icon_id: r.get(8)?,
            }, r.get::<_, String>(9)?, r.get::<_, String>(10)? == "oauth")))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?;
        let mut out = Vec::with_capacity(heads.len());
        for (account, keychain_ref, oauth) in heads {
            let config = self.account_config(&account.id)?;
            out.push(SyncAccount { account, config, keychain_ref, oauth });
        }
        Ok(out)
    }

    /// The server folders stored for an account (the mailbox lists map them to roles).
    pub fn account_folders(&self, account_id: &str) -> Result<Vec<String>, StoreError> {
        let id = account_id.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT DISTINCT folder FROM message WHERE account_id = ?1 ORDER BY folder")?;
            let rows = st.query_map(params![id], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// The newest rows of the given (account, server folder) pairs: Sent, Drafts, Junk, Trash and Archive as each account
    /// names them. Each pair is one index walk (message_by_folder); the merge keeps `limit` rows, newest first.
    pub fn folder_rows(&self, pairs: &[(String, String)], limit: usize) -> Result<Vec<InboxRow>, StoreError> {
        let pairs = pairs.to_vec();
        self.read(move |c| {
            let mut keys: Vec<(i64, String)> = Vec::new();
            let mut st = c.prepare_cached("SELECT received_at_ms, id FROM message WHERE account_id = ?1 AND folder = ?2 ORDER BY received_at_ms DESC LIMIT ?3")?;
            for (a, f) in &pairs {
                let rows = st.query_map(params![a, f, limit as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                for row in rows { keys.push(row?); }
            }
            keys.sort_by(|x, y| y.0.cmp(&x.0).then_with(|| x.1.cmp(&y.1)));
            let ids: Vec<String> = keys.into_iter().take(limit).map(|(_, id)| id).collect();
            crate::read::project(c, &ids)
        })
    }

    /// Starred, Muted and All mail: the flag columns (0005) and every stored message, newest first, in an account set.
    pub fn view_rows(&self, view: StoredView, accounts: Option<Vec<String>>, limit: usize) -> Result<Vec<InboxRow>, StoreError> {
        self.read(move |c| {
            let scope = accounts_clause(accounts.as_deref());
            let ids = match view {
                StoredView::Starred => newest_ids(c, &format!("m.starred = 1{scope}"), &[], limit)?,
                StoredView::Muted => newest_ids(c, &format!("m.muted = 1{scope}"), &[], limit)?,
                StoredView::AllMail => newest_ids(c, &format!("1 = 1{scope}"), &[], limit)?,
                // delta 18: a thread whose reply reminder fired (its time came and no reply arrived by then), one row per thread
                StoredView::Reminders { now_ms } => crate::reminders::fired_message_ids(c, now_ms, &accounts_clause_on("s", accounts.as_deref()), limit)?,
                StoredView::Vips => newest_ids(c, &format!(
                    "(EXISTS (SELECT 1 FROM vip v WHERE v.kind = 'address' AND v.value = lower(m.from_address)) \
                      OR EXISTS (SELECT 1 FROM vip v WHERE v.kind = 'domain' AND lower(m.from_address) LIKE '%@' || v.value)) AND {}{scope}", crate::read::VISIBLE), &[], limit)?,
            };
            crate::read::project(c, &ids)
        })
    }

    /// What the list filters (unread, starred, important, awaiting a reply) need about each row. Important = filed in the
    /// Needs Reply tab or from a VIP; awaiting a reply = the thread's newest message is not the owner's own.
    pub fn filter_facts(&self, message_ids: &[String]) -> Result<HashMap<String, ListFilterFacts>, StoreError> {
        let ids = message_ids.to_vec();
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT m.read, m.starred, \
                        (EXISTS (SELECT 1 FROM message_category mc WHERE mc.message_id = m.id AND mc.tab_id = 'needs-reply') \
                         OR EXISTS (SELECT 1 FROM vip v WHERE v.kind = 'address' AND v.value = lower(m.from_address))), \
                        (SELECT lower(n.from_address) <> lower(a.address) FROM message n WHERE n.thread_id = m.thread_id ORDER BY n.received_at_ms DESC LIMIT 1) \
                 FROM message m JOIN account a ON a.id = m.account_id WHERE m.id = ?1")?;
            let mut out = HashMap::new();
            for id in &ids {
                if let Some(f) = st.query_row(params![id], |r| Ok(ListFilterFacts {
                    unread: r.get::<_, i64>(0)? == 0, starred: r.get::<_, i64>(1)? != 0, important: r.get::<_, bool>(2)?,
                    awaiting_reply: r.get::<_, Option<bool>>(3)?.unwrap_or(false),
                })).optional()? {
                    out.insert(id.clone(), f);
                }
            }
            Ok(out)
        })
    }

    /// A folder's stored IMAP position (UIDVALIDITY, HIGHESTMODSEQ), written by the sync's pages. None before the first page.
    pub fn imap_cursor(&self, account_id: &str, folder: &str) -> Result<Option<(u32, u64)>, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.read(move |c| {
            Ok(c.query_row(
                "SELECT imap_uid_validity, imap_highest_mod_seq FROM folder_sync_state WHERE account_id = ?1 AND folder = ?2 AND cursor_kind = 'imap'",
                params![a, f], |r| Ok((r.get::<_, i64>(0)?.max(0) as u32, r.get::<_, i64>(1)?.max(0) as u64)),
            ).optional()?)
        })
    }

    /// Moves a folder's IMAP position after a change set was applied (IDLE, then CONDSTORE/QRESYNC resync).
    pub fn set_imap_cursor(&self, account_id: &str, folder: &str, uid_validity: u32, highest_mod_seq: u64) -> Result<(), StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.write(move |ctx| {
            ctx.tx.execute("INSERT INTO folder_sync_state (account_id, folder) VALUES (?1, ?2) ON CONFLICT(account_id, folder) DO NOTHING", params![a, f])?;
            ctx.tx.execute(
                "UPDATE folder_sync_state SET cursor_kind = 'imap', imap_uid_validity = ?3, imap_highest_mod_seq = ?4, cursor_value = NULL WHERE account_id = ?1 AND folder = ?2",
                params![a, f, uid_validity as i64, highest_mod_seq as i64])?;
            Ok(())
        })
    }

    /// The stored messages of one folder by IMAP UID (`remote_id`), for a resync's known UIDs and its flag changes.
    /// lane reply-headers (lead item): the stored read and starred values of one account's folder, one read for the folder: a resync on a
    /// server with no change list (Microsoft 365, no CONDSTORE) reports every known UID's flags, and the sync writes only the ones that differ.
    pub fn flags_in_folder(&self, account_id: &str, folder: &str) -> Result<std::collections::HashMap<String, (bool, bool)>, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT id, read, starred FROM message WHERE account_id = ?1 AND folder = ?2 AND remote_id IS NOT NULL")?;
            let rows = st.query_map(params![a, f], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, i64>(1)? != 0, r.get::<_, i64>(2)? != 0))))?;
            Ok(rows.collect::<Result<std::collections::HashMap<_, _>, _>>()?)
        })
    }

    pub fn uids_in_folder(&self, account_id: &str, folder: &str) -> Result<Vec<(u32, String)>, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT remote_id, id FROM message WHERE account_id = ?1 AND folder = ?2 AND remote_id IS NOT NULL")?;
            let rows = st.query_map(params![a, f], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for row in rows {
                let (uid, id) = row?;
                if let Ok(u) = uid.parse::<u32>() { out.push((u, id)); }
            }
            Ok(out)
        })
    }
}

/// The stored views that are a column of `message`, not a folder role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredView { Starred, Muted, AllMail, Vips, Reminders { now_ms: i64 } }

// sync-full-r1: the background crawl (every body of every folder) and the indexer (the word and meaning indexes). Their reads go
// through a read-only connection of their own (`background_reads`), never the shared reader the views wait on; the crawl's bodies
// are kept on the writer thread without announcing the lists or the counts (`set_bodies_quiet`), so a crawl over a whole mailbox
// never redraws the inbox and never makes the counts be recomputed. Every table and column exists; no migration.

/// One row of a folder as the crawl walks it, newest first: its IMAP UID, its id, its date and size, and whether its body is kept.
#[derive(Debug, Clone, PartialEq)]
pub struct CrawlRow { pub uid: u32, pub message_id: String, pub received_at_ms: i64, pub size_bytes: Option<i64>, pub has_body: bool }

/// One folder of one account: the messages stored, and of them the ones whose full body is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct FolderBodies { pub account_id: String, pub folder: String, pub total: u64, pub stored: u64 }

/// A stored body as the indexes take it: the row number (the meaning index's message key) and what a retrieved chunk carries
/// (date, sender, folder, subject), beside the text.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexRow {
    pub rowid: i64,
    pub message_id: String,
    pub account_id: String,
    pub folder: String,
    pub subject: String,
    pub sender_name: String,
    pub from_address: String,
    pub received_at_ms: i64,
    pub preview: String,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
}

/// The background work's own read-only connection (WAL: it never waits for a write, and a view never waits for it).
pub struct BackgroundReads { conn: Connection }

const INDEX_ROW_COLUMNS: &str = "rowid, id, account_id, folder, subject, sender_name, from_address, received_at_ms, preview, body_text, body_html";

fn index_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<IndexRow> {
    Ok(IndexRow {
        rowid: r.get(0)?, message_id: r.get(1)?, account_id: r.get(2)?, folder: r.get(3)?, subject: r.get(4)?, sender_name: r.get(5)?,
        from_address: r.get(6)?, received_at_ms: r.get(7)?, preview: r.get(8)?, body_text: r.get(9)?, body_html: r.get(10)?,
    })
}

impl Store {
    /// A read-only connection for the background work (the crawl, the indexer, the progress read).
    pub fn background_reads(&self) -> Result<BackgroundReads, StoreError> {
        Ok(BackgroundReads { conn: crate::open::open_reader(&self.path)? })
    }

    /// Of one folder, up to `limit` messages whose body is stored but whose engine record is missing (kept before the engine was wired) or
    /// was stopped by the time limit, newest first, as (IMAP UID, message id): what the prefetch re-makes (owner's picture 210, 13 Nov 2026).
    /// Rows without a UID are never listed.
    pub fn prepared_missing(&self, account_id: &str, folder: &str, limit: usize) -> Result<Vec<(u32, String)>, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT m.remote_id, m.id FROM message m LEFT JOIN message_prepared p ON p.message_id = m.id \
                 WHERE m.account_id = ?1 AND m.folder = ?2 AND m.remote_id IS NOT NULL AND (m.body_text IS NOT NULL OR m.body_html IS NOT NULL) \
                   AND (p.message_id IS NULL OR instr(p.prepared, 'time limit reached') > 0) \
                 ORDER BY m.received_at_ms DESC, m.id LIMIT ?3")?;
            let rows = st.query_map(params![a, f, limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            let mut out = Vec::new();
            for row in rows {
                let (uid, id) = row?;
                if let Ok(u) = uid.parse::<u32>() { out.push((u, id)); }
            }
            Ok(out)
        })
    }

    /// Keeps the engine's record of messages whose body is stored already: (message id, record). Nothing else of a row changes and no
    /// list is redrawn. A message without a stored body is skipped. Answers the ids written.
    pub fn set_prepared_quiet(&self, rows: Vec<(String, String)>) -> Result<Vec<String>, StoreError> {
        if rows.is_empty() { return Ok(Vec::new()); }
        self.submit(move |st| {
            let out = (|| -> Result<Vec<String>, StoreError> {
                let tx = st.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let mut written = Vec::new();
                {
                    let mut has = tx.prepare_cached("SELECT 1 FROM message WHERE id = ?1 AND (body_text IS NOT NULL OR body_html IS NOT NULL)")?;
                    for (id, prepared) in rows {
                        if has.exists(params![id])? {
                            keep_prepared(&tx, &id, Some(&prepared))?;
                            written.push(id);
                        }
                    }
                }
                tx.commit()?;
                Ok(written)
            })();
            st.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out
        })?
    }

    /// Keeps crawled bodies in ONE transaction on the writer thread, only on rows that have no body yet (a body the reader's own
    /// download kept first stays), and announces nothing: no list, count or thread event, no table change. A body changes no count
    /// and no list row but the preview, which the next read shows. Returns the ids written.
    pub fn set_bodies_quiet(&self, bodies: Vec<BodyParts>) -> Result<Vec<String>, StoreError> {
        if bodies.is_empty() { return Ok(Vec::new()); }
        self.submit(move |st| {
            let out = (|| -> Result<Vec<String>, StoreError> {
                let tx = st.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let mut written = Vec::new();
                {
                    let mut up = tx.prepare_cached(
                        "UPDATE message SET body_text = ?2, body_html = ?3, trackers_blocked = ?4, preview = ?5 \
                         WHERE id = ?1 AND body_text IS NULL AND body_html IS NULL")?;
                    for b in bodies {
                        if up.execute(params![b.message_id, b.body_text.unwrap_or_default(), b.body_html, b.trackers_blocked as i64, b.preview])? == 1 {
                            keep_prepared(&tx, &b.message_id, b.prepared.as_deref())?;
                            written.push(b.message_id);
                        }
                    }
                }
                tx.commit()?;
                Ok(written)
            })();
            // the update hook noted the rows; nothing is published for them
            st.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out
        })?
    }
}

impl BackgroundReads {
    /// The folders of one account that hold stored messages.
    pub fn folders(&self, account_id: &str) -> Result<Vec<String>, StoreError> {
        let mut st = self.conn.prepare_cached("SELECT DISTINCT folder FROM message WHERE account_id = ?1 ORDER BY folder")?;
        let rows = st.query_map(params![account_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Up to `limit` rows of one folder strictly below `below` (date, id), newest first (ties by id, descending), the rows
    /// without a UID left out (imported mail is on no server). The body test reads each row's header only.
    pub fn crawl_page(&self, account_id: &str, folder: &str, below: Option<(i64, String)>, limit: usize) -> Result<Vec<CrawlRow>, StoreError> {
        let (at, id) = below.unwrap_or((i64::MAX, String::new()));
        let mut st = self.conn.prepare_cached(CRAWL_PAGE_SQL)?;
        let rows = st.query_map(params![account_id, folder, at, id, limit as i64, below_is_set(&id, at)], |r| Ok((
            r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<i64>>(3)?, r.get::<_, bool>(4)?,
        )))?;
        let mut out = Vec::new();
        for row in rows {
            let (uid, message_id, received_at_ms, size_bytes, has_body) = row?;
            if let Ok(uid) = uid.parse::<u32>() { out.push(CrawlRow { uid, message_id, received_at_ms, size_bytes, has_body }); }
        }
        Ok(out)
    }

    /// Every folder's messages and stored bodies, by account then folder.
    pub fn folder_bodies(&self) -> Result<Vec<FolderBodies>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT account_id, folder, count(*), sum(body_text IS NOT NULL OR body_html IS NOT NULL) FROM message \
             GROUP BY account_id, folder ORDER BY account_id, folder")?;
        let rows = st.query_map([], |r| Ok(FolderBodies {
            account_id: r.get(0)?, folder: r.get(1)?, total: r.get::<_, i64>(2)?.max(0) as u64, stored: r.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as u64,
        }))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Up to `limit` stored bodies after row `after_rowid`, by row: (row, account, folder). The indexer's pass over what is kept.
    pub fn stored_after(&self, after_rowid: i64, limit: usize) -> Result<Vec<(i64, String, String)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT rowid, account_id, folder FROM message WHERE rowid > ?1 AND (body_text IS NOT NULL OR body_html IS NOT NULL) ORDER BY rowid LIMIT ?2")?;
        let rows = st.query_map(params![after_rowid, limit as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// The rows of these row numbers that have a stored body, by row.
    pub fn index_rows(&self, rowids: &[i64]) -> Result<Vec<IndexRow>, StoreError> {
        let mut st = self.conn.prepare_cached(&format!(
            "SELECT {INDEX_ROW_COLUMNS} FROM message WHERE rowid = ?1 AND (body_text IS NOT NULL OR body_html IS NOT NULL)"))?;
        let mut out = Vec::new();
        for r in rowids {
            if let Some(row) = st.query_row(params![r], index_row).optional()? { out.push(row); }
        }
        Ok(out)
    }

    /// The row numbers of these message ids (the ones still stored).
    pub fn rowids_of(&self, ids: &[String]) -> Result<Vec<i64>, StoreError> {
        let mut st = self.conn.prepare_cached("SELECT rowid FROM message WHERE id = ?1")?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(r) = st.query_row(params![id], |r| r.get::<_, i64>(0)).optional()? { out.push(r); }
        }
        Ok(out)
    }

    /// The plan SQLite chooses for the crawl's page read (a test reads it: the page walks message_by_folder, never the table).
    pub fn crawl_page_plan(&self) -> Result<Vec<String>, StoreError> {
        let mut st = self.conn.prepare(&format!("EXPLAIN QUERY PLAN {CRAWL_PAGE_SQL}"))?;
        let rows = st.query_map(params!["a", "INBOX", i64::MAX, "", 10i64, false], |r| r.get::<_, String>(3))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// The crawl's page: the index range on (account, folder, date) bounds the walk, the id breaks ties within one date.
const CRAWL_PAGE_SQL: &str =
    "SELECT remote_id, id, received_at_ms, size_bytes, (body_text IS NOT NULL OR body_html IS NOT NULL) FROM message \
     WHERE account_id = ?1 AND folder = ?2 AND remote_id IS NOT NULL AND received_at_ms <= ?3 AND (?6 = 0 OR received_at_ms < ?3 OR id < ?4) \
     ORDER BY received_at_ms DESC, id DESC LIMIT ?5";

/// False for the first page (no row read yet): every row of the folder is below it.
fn below_is_set(id: &str, at: i64) -> bool { !(id.is_empty() && at == i64::MAX) }

// engine-act-r6: the Archive view (and every server-folder mailbox) answered from the local store at once.
impl Store {
    /// The rows a server-folder mailbox lists: the newest rows of the (account, folder) pairs and, with `flag_archived_of`, the rows those
    /// accounts' owner archived by the flag (folder INBOX, `archived = 1`: what the core's own E writes, before and after the server moves
    /// its copy), newest first, `limit` kept. A folder row that IS the server's copy of another stored message (the place a core move
    /// recorded in `message_location`, engine-act-r5) is left out: that message is listed by its own row, so the next sync of the folder,
    /// which stores the copy under its own id, adds no second row. Each part is one index walk (message_by_folder).
    pub fn mailbox_rows(&self, pairs: &[(String, String)], flag_archived_of: Option<&[String]>, limit: usize) -> Result<Vec<InboxRow>, StoreError> {
        let pairs = pairs.to_vec();
        let flagged: Vec<String> = flag_archived_of.map(<[String]>::to_vec).unwrap_or_default();
        self.read(move |c| {
            let mut keys: Vec<(i64, String)> = Vec::new();
            let mut st = c.prepare_cached(
                "SELECT m.received_at_ms, m.id FROM message m WHERE m.account_id = ?1 AND m.folder = ?2 \
                 AND NOT EXISTS (SELECT 1 FROM message_location l WHERE l.account_id = m.account_id AND l.folder = m.folder \
                                 AND m.remote_id IS NOT NULL AND l.uid = CAST(m.remote_id AS INTEGER) AND l.message_id <> m.id) \
                 ORDER BY m.received_at_ms DESC LIMIT ?3")?;
            for (a, f) in &pairs {
                let rows = st.query_map(params![a, f, limit as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                for row in rows { keys.push(row?); }
            }
            let mut st = c.prepare_cached(
                "SELECT received_at_ms, id FROM message WHERE account_id = ?1 AND folder = ?2 AND archived = 1 ORDER BY received_at_ms DESC LIMIT ?3")?;
            for a in &flagged {
                let rows = st.query_map(params![a, crate::INBOX_FOLDER, limit as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                for row in rows { keys.push(row?); }
            }
            keys.sort_by(|x, y| y.0.cmp(&x.0).then_with(|| x.1.cmp(&y.1)));
            keys.dedup_by(|x, y| x.1 == y.1);
            let ids: Vec<String> = keys.into_iter().take(limit).map(|(_, id)| id).collect();
            crate::read::project(c, &ids)
        })
    }
}

// sync-full-r1: a server without UIDPLUS names no place when the core's E moves a message to Archive, so the Archive folder's next sync
// stores the server's copy as a row of its own. Linking that copy to the owner's archived row records the place the move did not name:
// `mailbox_rows` then lists the owner's row once, and the way back (⇧E) names the copy's folder and UID.
impl Store {
    /// For each row of the account's Archive `folder` that is the server's copy of a message the owner archived (INBOX, archived, no place
    /// recorded; the same thread, date, sender and subject), records the copy as that message's server place. Returns how many were linked.
    pub fn link_archived_copies(&self, account_id: &str, folder: &str, uid_validity: u32) -> Result<usize, StoreError> {
        let (a, f) = (account_id.to_string(), folder.to_string());
        self.write(move |ctx| {
            let pairs: Vec<(String, i64)> = {
                let mut st = ctx.tx.prepare(
                    "SELECT o.id, CAST(c.remote_id AS INTEGER) FROM message c JOIN message o \
                       ON o.account_id = c.account_id AND o.folder = ?3 AND o.archived = 1 AND o.id <> c.id \
                      AND o.thread_id = c.thread_id AND o.received_at_ms = c.received_at_ms AND o.from_address = c.from_address AND o.subject = c.subject \
                     WHERE c.account_id = ?1 AND c.folder = ?2 AND c.remote_id IS NOT NULL \
                       AND NOT EXISTS (SELECT 1 FROM message_location l WHERE l.message_id = o.id) \
                       AND NOT EXISTS (SELECT 1 FROM message_location l WHERE l.account_id = c.account_id AND l.folder = c.folder AND l.uid = CAST(c.remote_id AS INTEGER))")?;
                let rows = st.query_map(params![a, f, crate::INBOX_FOLDER], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            let mut n = 0;
            for (id, uid) in pairs {
                if uid > 0 && ctx.place_record(&id, &a, &f, uid_validity, uid as u32)? { n += 1; }
            }
            Ok(n)
        })
    }
}

// body-r2: rows stored before the text decoder held HTML character references ("Bonjour&nbsp;Assem") in `body_text` and `preview`. No raw
// message is stored (only the text and the SANITISED html), so the repair decodes the stored text in place, once.
impl Store {
    /// Repairs the text of every stored row that holds a character reference, ONCE: `fix` (the parse crate's decoder) is applied to
    /// `body_text`, and to `preview` with its whitespace collapsed again. Once is kept by `marker`, a file the repair's own transaction
    /// writes just before it commits: a marker that exists means the repair ran and nothing is read; a failed write of the marker
    /// undoes the repair (the next launch tries again). `fix` is not idempotent (`&amp;nbsp;` becomes `&nbsp;`, which decodes again to
    /// a space), so a second run would damage the text it already repaired: the marker is what stops it. A body stored after the
    /// decoder existed is already decoded and is not asked again. Answers the ids of the rows changed; None when the marker was there.
    pub fn repair_stored_text(&self, marker: &std::path::Path, fix: impl Fn(&str) -> String + Send + 'static) -> Result<Option<Vec<String>>, StoreError> {
        if marker.exists() { return Ok(None); }
        let marker = marker.to_path_buf();
        self.write(move |ctx| {
            let rows: Vec<(String, Option<String>, String)> = {
                let mut st = ctx.tx.prepare("SELECT id, body_text, preview FROM message WHERE body_text LIKE '%&%;%' OR preview LIKE '%&%;%'")?;
                let found = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)))?;
                found.collect::<Result<Vec<_>, _>>()?
            };
            let mut changed: Vec<String> = Vec::new();
            {
                let mut up = ctx.tx.prepare_cached("UPDATE message SET body_text = ?2, preview = ?3 WHERE id = ?1")?;
                for (id, text, preview) in rows {
                    let new_text = text.as_deref().map(&fix);
                    let new_preview = fix(&preview).split_whitespace().collect::<Vec<_>>().join(" ");
                    if new_text == text && new_preview == preview { continue; }
                    if up.execute(params![id, new_text, new_preview])? == 1 { changed.push(id); }
                }
            }
            std::fs::write(&marker, format!("text repaired: {} rows\n", changed.len())).map_err(|e| StoreError::Invalid(format!("the repair's marker {} was not written: {e}", marker.display())))?;
            Ok(Some(changed))
        })
    }

    /// The one-time repair of previews that hold a drawn line ("---", "_____", "====="): `fix(stored text, old preview)` answers the preview
    /// to keep. Only rows whose preview holds three rule characters in a row are read. The marker is written in the same transaction's
    /// closure, so a failed write undoes the repair and the next start tries again. Answers the number of rows changed; None when the marker was there.
    pub fn repair_stored_previews(&self, marker: &std::path::Path, fix: impl Fn(Option<&str>, &str) -> String + Send + 'static) -> Result<Option<usize>, StoreError> {
        if marker.exists() { return Ok(None); }
        let marker = marker.to_path_buf();
        self.write(move |ctx| {
            let rows: Vec<(String, Option<String>, String)> = {
                let mut st = ctx.tx.prepare("SELECT id, body_text, preview FROM message WHERE preview GLOB '*---*' OR preview GLOB '*___*' OR preview GLOB '*===*' \
                    OR preview GLOB '*[*][*][*]*' OR preview GLOB '*~~~*' OR preview GLOB '*###*' OR preview GLOB '*\u{2014}\u{2014}\u{2014}*' OR preview GLOB '*\u{2013}\u{2013}\u{2013}*' \
                    OR preview GLOB '*\u{2500}\u{2500}\u{2500}*' OR preview GLOB '*\u{2501}\u{2501}\u{2501}*' OR preview GLOB '*\u{2550}\u{2550}\u{2550}*' OR preview GLOB '*\u{2022}\u{2022}\u{2022}*'")?;
                let found = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)))?;
                found.collect::<Result<Vec<_>, _>>()?
            };
            let mut changed = 0usize;
            {
                let mut up = ctx.tx.prepare_cached("UPDATE message SET preview = ?2 WHERE id = ?1")?;
                for (id, text, preview) in rows {
                    let new_preview = fix(text.as_deref(), &preview);
                    if new_preview == preview { continue; }
                    changed += up.execute(params![id, new_preview])?;
                }
            }
            std::fs::write(&marker, format!("previews repaired: {changed} rows\n")).map_err(|e| StoreError::Invalid(format!("the repair's marker {} was not written: {e}", marker.display())))?;
            Ok(Some(changed))
        })
    }

    /// 14 Nov 2026 (owner's picture 216; on the owner's store 14 previews held source: a doctype, a part boundary, page code): the one-time
    /// repair of previews that hold source, with the list of mails whose text part is page code. Rows whose preview starts with `<` or
    /// `--` or names a part header, and rows whose text starts with `<`, are read; `fix(stored text, old preview)` answers the preview to
    /// keep and whether the text is page code (the caller indexes its words again). The marker is written in the same transaction's
    /// closure, so a failed write undoes the repair and the next start tries again. Answers (rows changed, ids to index again); None
    /// when the marker was there.
    pub fn repair_source_previews(&self, marker: &std::path::Path, fix: impl Fn(Option<&str>, &str) -> (String, bool) + Send + 'static) -> Result<Option<(usize, Vec<String>)>, StoreError> {
        if marker.exists() { return Ok(None); }
        let marker = marker.to_path_buf();
        self.write(move |ctx| {
            let rows: Vec<(String, Option<String>, String)> = {
                let mut st = ctx.tx.prepare("SELECT id, body_text, preview FROM message WHERE ltrim(preview, char(32, 9, 10, 13)) LIKE '<%' OR ltrim(preview, char(32, 9, 10, 13)) LIKE '--%' \
                    OR preview LIKE '%Content-Type:%' OR preview LIKE '%Content-Transfer-Encoding:%' OR ltrim(body_text, char(32, 9, 10, 13)) LIKE '<%'")?;
                let found = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, String>(2)?)))?;
                found.collect::<Result<Vec<_>, _>>()?
            };
            let (mut changed, mut again) = (0usize, Vec::new());
            {
                let mut up = ctx.tx.prepare_cached("UPDATE message SET preview = ?2 WHERE id = ?1")?;
                for (id, text, preview) in rows {
                    let (new_preview, code) = fix(text.as_deref(), &preview);
                    if code { again.push(id.clone()); }
                    if new_preview == preview { continue; }
                    changed += up.execute(params![id, new_preview])?;
                }
            }
            std::fs::write(&marker, format!("previews repaired: {changed} rows; mails with page code as text: {}\n", again.len())).map_err(|e| StoreError::Invalid(format!("the repair's marker {} was not written: {e}", marker.display())))?;
            Ok(Some((changed, again)))
        })
    }
}

// engine-act-r6 round 2 (contracts delta 22, migration 0015): the files of a draft, then of its queued message (`outgoing_attachment`).
/// The owner kinds of an `outgoing_attachment` row: a draft, then (after Send) the outbox entry the draft became.
const OWNER_DRAFT: &str = "draft";
const OWNER_OUTBOX: &str = "outbox";

const OUTGOING_COLS: &str = "id, owner_id, source_kind, path, message_id, attachment_id, file_name, media_type, size_bytes, state";

fn outgoing_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<contracts::OutgoingAttachment> {
    let kind: String = r.get(2)?;
    let source = if kind == "local_file" {
        contracts::AttachmentSource::LocalFile { path: r.get::<_, Option<String>>(3)?.unwrap_or_default() }
    } else {
        contracts::AttachmentSource::StoredMessage { message_id: r.get::<_, Option<String>>(4)?.unwrap_or_default(), attachment_id: r.get::<_, Option<String>>(5)?.unwrap_or_default() }
    };
    let state: String = r.get(9)?;
    Ok(contracts::OutgoingAttachment {
        id: r.get(0)?, owner_id: r.get(1)?, source, file_name: r.get(6)?, media_type: r.get(7)?, size_bytes: r.get::<_, i64>(8)?.max(0) as u64,
        state: contracts::outgoing_attachment_state_from_key(state).unwrap_or(contracts::OutgoingAttachmentState::Ready),
    })
}

fn outgoing_of(c: &Connection, owner_kind: &str, owner_id: &str) -> Result<Vec<contracts::OutgoingAttachment>, StoreError> {
    let mut st = c.prepare_cached(&format!("SELECT {OUTGOING_COLS} FROM outgoing_attachment WHERE owner_kind = ?1 AND owner_id = ?2 ORDER BY sort_order, id"))?;
    let rows = st.query_map(params![owner_kind, owner_id], outgoing_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

impl Store {
    /// The files of a draft, in the order they were added.
    pub fn draft_files(&self, draft_id: &str) -> Result<Vec<contracts::OutgoingAttachment>, StoreError> {
        let o = draft_id.to_string();
        self.read(move |c| outgoing_of(c, OWNER_DRAFT, &o))
    }

    /// The files of a queued (or sent) message: the rows its draft handed over at Send.
    pub fn outbox_files(&self, outbox_id: &str) -> Result<Vec<contracts::OutgoingAttachment>, StoreError> {
        let o = outbox_id.to_string();
        self.read(move |c| outgoing_of(c, OWNER_OUTBOX, &o))
    }

    /// The rows with these ids, whoever owns them now, in the order of `ids` (an id with no row is left out). The sender reads a queued
    /// message's files by the ids its draft carried.
    pub fn outgoing_attachments_by_ids(&self, ids: &[String]) -> Result<Vec<contracts::OutgoingAttachment>, StoreError> {
        let ids = ids.to_vec();
        self.read(move |c| {
            let mut st = c.prepare_cached(&format!("SELECT {OUTGOING_COLS} FROM outgoing_attachment WHERE id = ?1"))?;
            let mut out = Vec::new();
            for id in &ids {
                if let Some(a) = st.query_row(params![id], outgoing_row).optional()? { out.push(a); }
            }
            Ok(out)
        })
    }

    /// Whether a draft row exists (open for editing), and whether an outbox entry was made from a draft of this id (queued or sent).
    pub fn draft_open_or_queued(&self, draft_id: &str) -> Result<(bool, bool), StoreError> {
        let id = draft_id.to_string();
        self.read(move |c| {
            let open = c.prepare_cached("SELECT 1 FROM draft WHERE id = ?1")?.exists(params![id])?;
            let queued = c.prepare_cached("SELECT 1 FROM outbox_entry WHERE json_extract(draft_json, '$.draft.id') = ?1")?.exists(params![id])?;
            Ok((open, queued))
        })
    }

    /// One attachment of a stored message, with where its bytes live on this Mac, and the message's server place (folder, UID) to fetch it
    /// from when they do not. None when the message holds no such attachment.
    pub fn stored_attachment(&self, message_id: &str, attachment_id: &str) -> Result<Option<(contracts::AttachmentFile, String, Option<String>, String)>, StoreError> {
        let (m, a) = (message_id.to_string(), attachment_id.to_string());
        self.read(move |c| {
            Ok(c.query_row(
                "SELECT a.id, a.file_name, a.mime_type, a.size_bytes, a.local_path, m.folder, m.remote_id, m.account_id, a.inline OR coalesce(json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.inline'), 0), coalesce(a.content_id, json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.cid')) FROM attachment a JOIN message m ON m.id = a.message_id \
                 WHERE a.message_id = ?1 AND a.id = ?2",
                params![m, a],
                |r| Ok((
                    contracts::AttachmentFile { id: r.get(0)?, message_id: m.clone(), file_name: r.get(1)?, mime_type: r.get(2)?, size_bytes: r.get::<_, i64>(3)?.max(0) as u64, local_path: r.get(4)?, inline: r.get::<_, i64>(8)? == 1, content_id: r.get::<_, Option<String>>(9)?.filter(|s| !s.is_empty()) },
                    r.get::<_, String>(5)?, r.get::<_, Option<String>>(6)?, r.get::<_, String>(7)?,
                )),
            ).optional()?)
        })
    }

    /// The sent copy's files (engine-act-r6 round 2): one `attachment` row per file the message carried, as a synced message has them.
    /// Rows already there are kept (a second call adds nothing).
    pub fn put_message_attachments(&self, message_id: &str, files: Vec<contracts::AttachmentFile>) -> Result<usize, StoreError> {
        if files.is_empty() { return Ok(0); }
        let m = message_id.to_string();
        self.write(move |ctx| {
            let mut n = 0;
            for f in &files {
                n += ctx.tx.execute(
                    "INSERT OR IGNORE INTO attachment (id, message_id, file_name, mime_type, size_bytes, local_path) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![f.id, m, f.file_name, f.mime_type, f.size_bytes as i64, f.local_path],
                )?;
            }
            Ok(n)
        })
    }
}

impl crate::WriteCtx<'_> {
    /// The files of a draft, inside a larger transaction.
    pub fn draft_files_in(&self, draft_id: &str) -> Result<Vec<contracts::OutgoingAttachment>, StoreError> { outgoing_of(self.tx, OWNER_DRAFT, draft_id) }

    /// The next place in a draft's order (the forward's ids `<draft id>/<n>` and an added file's id use it).
    pub fn draft_file_next_order(&self, draft_id: &str) -> Result<i64, StoreError> {
        Ok(self.tx.query_row("SELECT coalesce(max(sort_order), 0) + 1 FROM outgoing_attachment WHERE owner_kind = ?1 AND owner_id = ?2", params![OWNER_DRAFT, draft_id], |r| r.get(0))?)
    }

    /// Adds one file to its draft (`a.owner_id`), after the ones it holds. Answers the row's place in the order.
    pub fn draft_file_insert(&mut self, a: &contracts::OutgoingAttachment) -> Result<i64, StoreError> {
        let owner_kind = OWNER_DRAFT;
        let order = self.draft_file_next_order(&a.owner_id)?;
        let (kind, path, mid, aid) = match &a.source {
            contracts::AttachmentSource::LocalFile { path } => ("local_file", Some(path.clone()), None, None),
            contracts::AttachmentSource::StoredMessage { message_id, attachment_id } => ("stored_message", None, Some(message_id.clone()), Some(attachment_id.clone())),
        };
        self.tx.execute(
            "INSERT INTO outgoing_attachment (id, owner_kind, owner_id, source_kind, path, message_id, attachment_id, file_name, media_type, size_bytes, state, sort_order) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![a.id, owner_kind, a.owner_id, kind, path, mid, aid, a.file_name, a.media_type, a.size_bytes as i64, contracts::outgoing_attachment_state_key(a.state), order],
        )?;
        Ok(order)
    }

    /// Removes one file from a draft (the file itself is never touched). Answers whether a row went.
    pub fn draft_file_delete(&mut self, draft_id: &str, attachment_id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.execute("DELETE FROM outgoing_attachment WHERE owner_kind = ?1 AND owner_id = ?2 AND id = ?3", params![OWNER_DRAFT, draft_id, attachment_id])? == 1)
    }

    /// Send: the draft's files go to the outbox entry it became (BEFORE the draft row is deleted: its trigger takes the rows a draft still owns).
    pub fn files_to_outbox(&mut self, draft_id: &str, outbox_id: &str) -> Result<usize, StoreError> { self.reown(OWNER_DRAFT, draft_id, OWNER_OUTBOX, outbox_id) }

    /// Undo or cancel of a send: the entry's files go back to the draft it gives back.
    pub fn files_to_draft(&mut self, outbox_id: &str, draft_id: &str) -> Result<usize, StoreError> { self.reown(OWNER_OUTBOX, outbox_id, OWNER_DRAFT, draft_id) }

    fn reown(&mut self, from_kind: &str, from_id: &str, to_kind: &str, to_id: &str) -> Result<usize, StoreError> {
        Ok(self.tx.execute("UPDATE outgoing_attachment SET owner_kind = ?3, owner_id = ?4 WHERE owner_kind = ?1 AND owner_id = ?2", params![from_kind, from_id, to_kind, to_id])?)
    }

    /// `Draft.attachment_ids` is the list of the draft's rows: the draft row's column is written from them.
    pub fn outgoing_sync_draft_ids(&mut self, draft_id: &str) -> Result<(), StoreError> {
        let ids: Vec<String> = outgoing_of(self.tx, OWNER_DRAFT, draft_id)?.into_iter().map(|a| a.id).collect();
        self.tx.execute("UPDATE draft SET attachment_ids_json = ?2 WHERE id = ?1", params![draft_id, crate::ops::json_strings(&ids)])?;
        Ok(())
    }
}

// engine-act-r6: the server copy of a message on its way back.
impl Store {
    /// The rows a sync stored for the server copies of these messages (the place a core move recorded: same account, folder and UID), when
    /// the messages come back (Unarchive, Not spam): the copy is the same server message as the owner's row, and the way back moves it, so
    /// it is dropped here, or the Archive (Junk) view would list it until the next sync of that folder. A move that fails leaves the copy on
    /// the server, and that sync stores it again. Answers how many rows went.
    pub fn forget_server_copies(&self, message_ids: &[String]) -> Result<usize, StoreError> {
        let ids = message_ids.to_vec();
        self.write(move |ctx| {
            let mut copies: Vec<String> = Vec::new();
            {
                let mut st = ctx.tx.prepare_cached(
                    "SELECT m.id FROM message_location l JOIN message m ON m.account_id = l.account_id AND m.folder = l.folder \
                     AND m.remote_id IS NOT NULL AND CAST(m.remote_id AS INTEGER) = l.uid WHERE l.message_id = ?1 AND m.id <> ?1")?;
                for id in &ids {
                    for row in st.query_map(params![id], |r| r.get::<_, String>(0))? { copies.push(row?); }
                }
            }
            if copies.is_empty() { return Ok(0); }
            ctx.delete_messages(&copies)
        })
    }
}

// sync-full-r1 round 2 (contracts delta 21, migration 0014): the crawl's place in the three `folder_sync_state` columns (record
// `BodyCrawlPlace`), the `chunk` rows the meaning index files its vectors under (each chunk's OWN id), and the progress per folder.
impl Store {
    /// Writes one folder's crawl place (a folder the sync never wrote gets its row). An illegal shape is refused before the write
    /// (`crawl_place_is_legal`; the 0014 triggers refuse it too).
    pub fn set_crawl_place(&self, place: contracts::BodyCrawlPlace) -> Result<(), StoreError> {
        if !contracts::crawl_place_is_legal(place.clone()) {
            return Err(StoreError::Invalid("a crawl place has both halves, and a finished walk has none".into()));
        }
        self.write(move |ctx| {
            ctx.tx.execute("INSERT INTO folder_sync_state (account_id, folder) VALUES (?1, ?2) ON CONFLICT(account_id, folder) DO NOTHING", params![place.account_id, place.folder])?;
            ctx.tx.execute(
                "UPDATE folder_sync_state SET body_crawl_before_ms = ?3, body_crawl_before_id = ?4, body_crawl_done_ms = ?5 WHERE account_id = ?1 AND folder = ?2",
                params![place.account_id, place.folder, place.before_ms, place.before_id, place.done_ms])?;
            Ok(())
        })
    }

    /// Replaces the body chunks of one message by `texts` (chunk_no 0, 1, ...), carrying its date, sender, folder and subject.
    /// Answers (the new chunks' ids, the ids of the chunks it replaced: their vectors are to be removed).
    pub fn replace_body_chunks(&self, message_id: &str, texts: Vec<String>) -> Result<(Vec<i64>, Vec<i64>), StoreError> {
        let id = message_id.to_string();
        self.write(move |ctx| {
            let old: Vec<i64> = {
                let mut st = ctx.tx.prepare_cached("SELECT id FROM chunk WHERE message_id = ?1 AND part = 'body'")?;
                let rows = st.query_map(params![id], |r| r.get::<_, i64>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            ctx.tx.execute("DELETE FROM chunk WHERE message_id = ?1 AND part = 'body'", params![id])?;
            let mut new = Vec::new();
            let mut st = ctx.tx.prepare_cached(
                "INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text) \
                 SELECT id, 'body', ?2, received_at_ms, CASE WHEN sender_name = '' THEN from_address ELSE sender_name || ' <' || from_address || '>' END, folder, subject, ?3 \
                 FROM message WHERE id = ?1")?;
            for (n, text) in texts.iter().enumerate().filter(|(_, t)| !t.trim().is_empty()) {
                if st.execute(params![id, n as i64, text])? == 1 { new.push(ctx.tx.last_insert_rowid()); }
            }
            Ok((new, old))
        })
    }

    /// attach-engine: replaces the chunks of ONE received file (`part` = `attachment:<id>`, chunk_no 0, 1, ...), under its message, carrying the message's date,
    /// sender, folder and subject as the body chunks do. A file of another message, or an unknown one, writes nothing. Answers (new ids, replaced ids). The
    /// chunks start with no vector: embedding them is the indexer's (the progress and the pending walk count BODY chunks only, so they never hold a message back).
    pub fn replace_attachment_chunks(&self, attachment_id: &str, texts: Vec<String>) -> Result<(Vec<i64>, Vec<i64>), StoreError> {
        let id = attachment_id.to_string();
        self.write(move |ctx| {
            let part = format!("attachment:{id}");
            let Some(message_id): Option<String> = ctx.tx.query_row("SELECT message_id FROM attachment WHERE id = ?1", params![id], |r| r.get(0)).optional()? else { return Ok((Vec::new(), Vec::new())) };
            let old: Vec<i64> = {
                let mut st = ctx.tx.prepare_cached("SELECT id FROM chunk WHERE message_id = ?1 AND part = ?2")?;
                let rows = st.query_map(params![message_id, part], |r| r.get::<_, i64>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            ctx.tx.execute("DELETE FROM chunk WHERE message_id = ?1 AND part = ?2", params![message_id, part])?;
            let mut new = Vec::new();
            let mut st = ctx.tx.prepare_cached(
                "INSERT INTO chunk (message_id, part, chunk_no, received_at_ms, sender, folder, subject, text) \
                 SELECT id, ?2, ?3, received_at_ms, CASE WHEN sender_name = '' THEN from_address ELSE sender_name || ' <' || from_address || '>' END, folder, subject, ?4 \
                 FROM message WHERE id = ?1")?;
            for (n, text) in texts.iter().enumerate().filter(|(_, t)| !t.trim().is_empty()) {
                if st.execute(params![message_id, part, n as i64, text])? == 1 { new.push(ctx.tx.last_insert_rowid()); }
            }
            Ok((new, old))
        })
    }

    /// attach-engine: the chunks of one received file, (id, text, has its vector), by chunk number.
    pub fn attachment_chunks(&self, attachment_id: &str) -> Result<Vec<(i64, String, bool)>, StoreError> {
        let part = format!("attachment:{attachment_id}");
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT id, text, embedded_at_ms IS NOT NULL FROM chunk WHERE part = ?1 ORDER BY chunk_no")?;
            let rows = st.query_map(params![part], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// The chunks' vectors are in the meaning index from `now_ms`.
    pub fn mark_chunks_embedded(&self, ids: Vec<i64>, now_ms: i64) -> Result<usize, StoreError> {
        self.write(move |ctx| {
            let mut st = ctx.tx.prepare_cached("UPDATE chunk SET embedded_at_ms = ?2 WHERE id = ?1")?;
            let mut n = 0;
            for id in &ids { n += st.execute(params![id, now_ms])?; }
            Ok(n)
        })
    }

    /// The meaning index was lost or rebuilt: no chunk has its vector any more. Answers how many were marked.
    pub fn unembed_all_chunks(&self) -> Result<usize, StoreError> {
        self.write(|ctx| Ok(ctx.tx.execute("UPDATE chunk SET embedded_at_ms = NULL WHERE embedded_at_ms IS NOT NULL", [])?))
    }

    /// stale-fix (item c1436f49): these messages were handed to the indexer while it could not cut them (no model): their body chunks lose
    /// their vector mark, so the next model hand-over's backfill queues them and the step cuts them again from today's text. A message
    /// with no body chunk is untouched. Answers how many chunks were marked.
    pub fn unembed_body_chunks(&self, message_ids: &[String]) -> Result<usize, StoreError> {
        if message_ids.is_empty() { return Ok(0); }
        let ids = message_ids.to_vec();
        self.write(move |ctx| {
            let mut st = ctx.tx.prepare_cached("UPDATE chunk SET embedded_at_ms = NULL WHERE part = 'body' AND message_id = ?1 AND embedded_at_ms IS NOT NULL")?;
            let mut n = 0;
            for id in &ids { n += st.execute(params![id])?; }
            Ok(n)
        })
    }
}

/// The SQL of the progress per (account, folder): messages, bodies, bodies whose chunks are all in the meaning index (the contract's
/// `FolderProgress`; the same figures as `Store::reply_folder_progress`, through the background connection).
const FOLDER_PROGRESS_SQL: &str =
    "SELECT account_id, folder, count(*), sum(CASE WHEN body_text IS NOT NULL OR body_html IS NOT NULL THEN 1 ELSE 0 END), \
     sum(CASE WHEN (body_text IS NOT NULL OR body_html IS NOT NULL) AND EXISTS (SELECT 1 FROM chunk k WHERE k.message_id = message.id AND k.part = 'body') \
              AND NOT EXISTS (SELECT 1 FROM chunk k WHERE k.message_id = message.id AND k.part = 'body' AND k.embedded_at_ms IS NULL) THEN 1 ELSE 0 END) \
     FROM message GROUP BY account_id, folder ORDER BY account_id, folder";

impl BackgroundReads {
    /// Every folder's crawl place, as the columns hold them (a folder with neither a place nor a finished walk is left out).
    pub fn crawl_places(&self) -> Result<Vec<contracts::BodyCrawlPlace>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT account_id, folder, body_crawl_before_ms, body_crawl_before_id, body_crawl_done_ms FROM folder_sync_state \
             WHERE body_crawl_before_ms IS NOT NULL OR body_crawl_done_ms IS NOT NULL ORDER BY account_id, folder")?;
        let rows = st.query_map([], |r| Ok(contracts::BodyCrawlPlace {
            account_id: r.get(0)?, folder: r.get(1)?, before_ms: r.get(2)?, before_id: r.get(3)?, done_ms: r.get(4)?,
        }))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// The progress per (account, folder), the contract's `FolderProgress`.
    pub fn folder_progress(&self) -> Result<Vec<contracts::FolderProgress>, StoreError> {
        let mut st = self.conn.prepare_cached(FOLDER_PROGRESS_SQL)?;
        let rows = st.query_map([], |r| Ok(contracts::FolderProgress {
            account_id: r.get(0)?, folder: r.get(1)?, messages: r.get::<_, i64>(2)?.max(0) as u64,
            bodies: r.get::<_, Option<i64>>(3)?.unwrap_or(0).max(0) as u64, indexed: r.get::<_, Option<i64>>(4)?.unwrap_or(0).max(0) as u64,
        }))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// One message's body chunks: (id, text, has its vector), by chunk number.
    pub fn body_chunks(&self, message_id: &str) -> Result<Vec<(i64, String, bool)>, StoreError> {
        let mut st = self.conn.prepare_cached("SELECT id, text, embedded_at_ms IS NOT NULL FROM chunk WHERE message_id = ?1 AND part = 'body' ORDER BY chunk_no")?;
        let rows = st.query_map(params![message_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, bool>(2)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// One chunk and its message's account (a vector whose chunk is gone answers None: its message was deleted).
    pub fn chunk(&self, id: i64) -> Result<Option<(contracts::Chunk, String)>, StoreError> {
        Ok(self.conn.prepare_cached(
            "SELECT k.id, k.message_id, k.part, k.chunk_no, k.received_at_ms, k.sender, k.folder, k.subject, k.text, k.embedded_at_ms, m.account_id \
             FROM chunk k JOIN message m ON m.id = k.message_id WHERE k.id = ?1")?
            .query_row(params![id], |r| {
                let part: String = r.get(2)?;
                Ok((contracts::Chunk {
                    id: r.get(0)?, message_id: r.get(1)?, part: contracts::chunk_part_from_key(part).unwrap_or(contracts::ChunkPart::Body),
                    chunk_no: r.get::<_, i64>(3)?.max(0) as u32, received_at_ms: r.get(4)?, sender: r.get(5)?, folder: r.get(6)?, subject: r.get(7)?,
                    text: r.get(8)?, embedded_at_ms: r.get(9)?,
                }, r.get::<_, String>(10)?))
            }).optional()?)
    }

    /// Stored bodies after row `after_rowid` that are not fully in the meaning index (no chunk yet, or a chunk without its vector).
    pub fn bodies_to_index_after(&self, after_rowid: i64, limit: usize) -> Result<Vec<i64>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT m.rowid FROM message m WHERE m.rowid > ?1 AND (m.body_text IS NOT NULL OR m.body_html IS NOT NULL) \
             AND (NOT EXISTS (SELECT 1 FROM chunk k WHERE k.message_id = m.id AND k.part = 'body') OR EXISTS (SELECT 1 FROM chunk k WHERE k.message_id = m.id AND k.part = 'body' AND k.embedded_at_ms IS NULL)) \
             ORDER BY m.rowid LIMIT ?2")?;
        let rows = st.query_map(params![after_rowid, limit as i64], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Ids of the chunks whose vector is in the meaning index, after `after`: what remains when the index is folded.
    pub fn embedded_chunks_after(&self, after: i64, limit: usize) -> Result<Vec<i64>, StoreError> {
        let mut st = self.conn.prepare_cached("SELECT id FROM chunk WHERE id > ?1 AND embedded_at_ms IS NOT NULL ORDER BY id LIMIT ?2")?;
        let rows = st.query_map(params![after, limit as i64], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

// body-r3: the files of a received message. attach-engine: migration 0016's four columns (`part_path`, `transfer_encoding`, `content_id`, `inline`) are
// WRITTEN as columns now, and the same four also ride in `remote_id` as one small JSON object (`{"part":"2","enc":"base64","cid":"x@y","inline":true}`,
// written by SQLite's own json functions) because older readers (and tests) read that; every read takes the column first and falls back to the JSON,
// so a row an older build wrote (JSON only, columns NULL/0) still reads. The row's id is
// `<message id>:att:<ordinal, 3 digits>` in the order the parse crate lists the parts, so a row keeps its id when the message is read again
// (an upsert: the file name, type, size and `remote_id` follow the parse; `local_path` and `extracted_text` stay) and `ORDER BY id` is the
// message's own order. A row goes with its message (ON DELETE CASCADE).

/// One file of a message as the parse crate lists it (`parse::Attachment`): what a row keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentIn { pub part: String, pub name: String, pub mime_type: String, pub size_bytes: u64, pub content_id: Option<String>, pub inline: bool, pub encoding: String }

/// Where one attachment's bytes come from: the message's place on the server, the part's number and its transfer encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentSource { pub id: String, pub message_id: String, pub account_id: String, pub folder: String, pub uid: Option<u32>, pub part: String, pub encoding: String,
                              pub file_name: String, pub size_bytes: u64, pub local_path: Option<String> }

/// The id of the n-th file (from 0) of a message.
pub fn attachment_row_id(message_id: &str, ordinal: usize) -> String { format!("{message_id}:att:{ordinal:03}") }

fn put_rows(tx: &rusqlite::Transaction<'_>, message_id: &str, files: &[AttachmentIn]) -> Result<bool, StoreError> {
    let known: Option<i64> = tx.query_row("SELECT 1 FROM message WHERE id = ?1", params![message_id], |r| r.get(0)).optional()?;
    if known.is_none() { return Ok(false); }
    {
        let mut up = tx.prepare_cached(
            "INSERT INTO attachment (id, message_id, remote_id, file_name, mime_type, size_bytes, part_path, transfer_encoding, content_id, inline) \
             VALUES (?1, ?2, json_object('part', ?3, 'enc', ?4, 'cid', ?5, 'inline', CASE WHEN ?6 THEN json('true') ELSE json('false') END), ?7, ?8, ?9, \
                     nullif(trim(?3), ''), nullif(trim(?4), ''), nullif(trim(coalesce(?5, '')), ''), ?6) \
             ON CONFLICT(id) DO UPDATE SET remote_id = excluded.remote_id, file_name = excluded.file_name, mime_type = excluded.mime_type, size_bytes = excluded.size_bytes, \
                     part_path = excluded.part_path, transfer_encoding = excluded.transfer_encoding, content_id = excluded.content_id, inline = excluded.inline")?;
        for (i, f) in files.iter().enumerate() {
            up.execute(params![attachment_row_id(message_id, i), message_id, f.part, f.encoding, f.content_id, f.inline, f.name, f.mime_type, f.size_bytes as i64])?;
        }
    }
    // rows of this reading's own numbering that the message no longer has (a shorter list than before) go; no other writer's row is touched
    let prefix = format!("{message_id}:att:");
    tx.execute("DELETE FROM attachment WHERE message_id = ?1 AND substr(id, 1, length(?2)) = ?2 AND id > ?3", params![message_id, prefix, if files.is_empty() { format!("{prefix}") } else { attachment_row_id(message_id, files.len() - 1) }])?;
    Ok(true)
}

impl Store {
    /// Writes the files of each message (idempotent, see above), in ONE transaction. `quiet` announces nothing (the crawl's way: no list is
    /// redrawn for a batch of a whole-mailbox walk). A message that is not stored is skipped. Answers how many messages were written.
    pub fn put_attachment_rows(&self, rows: Vec<(String, Vec<AttachmentIn>)>, quiet: bool) -> Result<u32, StoreError> {
        if rows.is_empty() { return Ok(0); }
        if !quiet {
            return self.write(move |ctx| { let mut n = 0; for (id, files) in &rows { if put_rows(&ctx.tx, id, files)? { n += 1; } } Ok(n) });
        }
        self.submit(move |st| {
            let out = (|| -> Result<u32, StoreError> {
                let tx = st.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let mut n = 0;
                for (id, files) in &rows { if put_rows(&tx, id, files)? { n += 1; } }
                tx.commit()?;
                Ok(n)
            })();
            st.touched.lock().unwrap_or_else(|e| e.into_inner()).clear();
            out
        })?
    }

    /// Where one attachment's bytes come from; None for an unknown id. `uid` is the IMAP UID the message was synced under.
    pub fn attachment_source(&self, attachment_id: &str) -> Result<Option<AttachmentSource>, StoreError> {
        let id = attachment_id.to_string();
        self.read(move |c| {
            Ok(c.query_row(
                "SELECT a.id, a.message_id, m.account_id, m.folder, m.remote_id, coalesce(a.part_path, json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.part'), ''), coalesce(a.transfer_encoding, json_extract(CASE WHEN json_valid(a.remote_id) THEN a.remote_id END, '$.enc'), 'none'), \
                        a.file_name, a.size_bytes, a.local_path \
                 FROM attachment a JOIN message m ON m.id = a.message_id WHERE a.id = ?1",
                params![id],
                |r| Ok(AttachmentSource {
                    id: r.get(0)?, message_id: r.get(1)?, account_id: r.get(2)?, folder: r.get(3)?, uid: r.get::<_, Option<String>>(4)?.and_then(|u| u.parse().ok()),
                    part: r.get(5)?, encoding: r.get(6)?, file_name: r.get(7)?, size_bytes: r.get::<_, i64>(8)?.max(0) as u64, local_path: r.get(9)?,
                }),
            ).optional()?)
        })
    }

    /// Records where an attachment's file is now on this Mac (None: no longer there). Announced like any row change (a gallery redraws).
    pub fn set_attachment_local_path(&self, attachment_id: &str, path: Option<&str>) -> Result<bool, StoreError> {
        let (id, p) = (attachment_id.to_string(), path.map(str::to_string));
        self.write(move |ctx| Ok(ctx.tx.execute("UPDATE attachment SET local_path = ?2 WHERE id = ?1", params![id, p])? == 1))
    }

    /// Every file path the attachment rows hold (the sweep of orphan files reads it).
    pub fn attachment_paths(&self) -> Result<Vec<String>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT local_path FROM attachment WHERE local_path IS NOT NULL")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// The messages of one account whose body is stored but that have no file row and whose size (RFC822.SIZE) exceeds their stored text and
    /// html by more than `gap_bytes`: the ones that probably carry a file (the stored body has no raw source to read it from). Newest first, as
    /// (IMAP UID, message id, folder). Rows with no UID (imported mail) are never listed.
    pub fn attachment_backfill_candidates(&self, account_id: &str, gap_bytes: u64, limit: usize) -> Result<Vec<(u32, String, String)>, StoreError> {
        let a = account_id.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT remote_id, id, folder FROM message m WHERE account_id = ?1 AND remote_id IS NOT NULL AND (body_text IS NOT NULL OR body_html IS NOT NULL) \
                 AND NOT EXISTS (SELECT 1 FROM attachment x WHERE x.message_id = m.id) \
                 AND coalesce(size_bytes, 0) - (length(coalesce(body_text, '')) + length(coalesce(body_html, ''))) > ?2 \
                 ORDER BY received_at_ms DESC, id LIMIT ?3")?;
            let rows = st.query_map(params![a, gap_bytes as i64, limit as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
            let mut out = Vec::new();
            for row in rows { let (uid, id, folder) = row?; if let Ok(u) = uid.parse::<u32>() { out.push((u, id, folder)); } }
            Ok(out)
        })
    }
}

// engine-act-r6 round 3: Z of a core archive is a way back too.
impl Store {
    /// `forget_server_copies` for the messages an undo entry of an archive names (`{"kind":"archive","ids":[...]}`, the actions crate's
    /// stored form): Z brings them back to the inbox and the write-back moves their server copy with them. Any other kind: nothing.
    pub fn forget_server_copies_of_undo(&self, undo_entry_id: &str) -> Result<usize, StoreError> {
        let id = undo_entry_id.to_string();
        let ids: Vec<String> = self.read(move |c| {
            let mut st = c.prepare_cached(
                "SELECT j.value FROM undo_entry u, json_each(u.action_json, '$.ids') j WHERE u.id = ?1 AND u.consumed = 0 AND json_extract(u.action_json, '$.kind') = 'archive'")?;
            let rows = st.query_map(params![id], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })?;
        if ids.is_empty() { return Ok(0); }
        self.forget_server_copies(&ids)
    }
}

// engine-act-r6 round 6: the per-message marks the screens read as state paths, a saved draft's text, the recent searches (0001's tables).
/// What `message/<id>/starred|archived` and `snooze/<id>/*` are read from: one row of `message` and its `snooze` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageMarks {
    pub account_id: String,
    pub folder: String,
    pub starred: bool,
    pub archived: bool,
    pub snoozed_until_ms: Option<i64>,
}

impl Store {
    /// The marks of one stored message; None when no such message is stored.
    pub fn message_marks(&self, message_id: &str) -> Result<Option<MessageMarks>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| {
            Ok(c.prepare_cached(
                "SELECT m.account_id, m.folder, m.starred, m.archived, (SELECT s.return_at_ms FROM snooze s WHERE s.message_id = m.id) FROM message m WHERE m.id = ?1")?
                .query_row(params![id], |r| Ok(MessageMarks {
                    account_id: r.get(0)?, folder: r.get(1)?, starred: r.get::<_, i64>(2)? != 0, archived: r.get::<_, i64>(3)? != 0, snoozed_until_ms: r.get(4)?,
                }))
                .optional()?)
        })
    }

    /// The whole text of a saved draft; None when no such draft is open.
    pub fn draft_body(&self, draft_id: &str) -> Result<Option<String>, StoreError> {
        let id = draft_id.to_string();
        self.read(move |c| Ok(c.prepare_cached("SELECT body FROM draft WHERE id = ?1")?.query_row(params![id], |r| r.get::<_, String>(0)).optional()?))
    }

    /// Enter on a search (`search_history`, 0001): the words go first, a row with the same words in any case goes, and only the newest `keep`
    /// rows stay. The caller trims the words and refuses empty ones.
    pub fn record_search(&self, query: &str, now_ms: i64, keep: usize) -> Result<(), StoreError> {
        let q = query.to_string();
        self.write(move |ctx| {
            ctx.tx.execute("DELETE FROM search_history WHERE lower(query) = lower(?1)", params![q])?;
            ctx.tx.execute("INSERT INTO search_history (query, at_ms) VALUES (?1, ?2)", params![q, now_ms])?;
            ctx.tx.execute("DELETE FROM search_history WHERE rowid NOT IN (SELECT rowid FROM search_history ORDER BY at_ms DESC, rowid DESC LIMIT ?1)", params![keep as i64])?;
            Ok(())
        })
    }

    /// The recent searches, newest first, at most `keep`.
    pub fn recent_searches(&self, keep: usize) -> Result<Vec<String>, StoreError> {
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT query FROM search_history ORDER BY at_ms DESC, rowid DESC LIMIT ?1")?;
            let rows = st.query_map(params![keep as i64], |r| r.get::<_, String>(0))?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }
}

// engine-act-r6 round 7: a list left is left for its whole thread.
impl Store {
    /// When the owner left the mailing list of this message: the earliest unsubscribe recorded on any message of its thread, or of its
    /// account carrying the very same List-Unsubscribe header (the same list). None when it was never left (or no such message is stored).
    pub fn list_left_at(&self, message_id: &str) -> Result<Option<i64>, StoreError> {
        let id = message_id.to_string();
        self.read(move |c| {
            Ok(c.prepare_cached(
                "SELECT min(o.unsubscribed_at_ms) FROM message m JOIN message o ON o.account_id = m.account_id \
                 AND (o.thread_id = m.thread_id OR (m.list_unsubscribe_json IS NOT NULL AND o.list_unsubscribe_json = m.list_unsubscribe_json)) \
                 WHERE m.id = ?1 AND o.unsubscribed_at_ms IS NOT NULL")?
                .query_row(params![id], |r| r.get::<_, Option<i64>>(0)).optional()?.flatten())
        })
    }
}

// attach-engine (the owner's "it should store locally"): the excerpts of the reply sources other than mail, kept in `source_chunk` (0016) with each item's
// stamp in `source_item` (0020). The key of a source is the contract's `reply_source_key` (`local` or `calendar`, U+001F, its parts); an item's key is the
// file's path inside its folder, or the event's "<id>@<start ms>". An item is replaced WHOLE (its rows and its stamp in one transaction), so a reader never
// sees half of a file.

/// An excerpt as the walk cut it: (chunk number, date, title, text). A stored one: (id, source key, item key, chunk number, date, title, text).
pub type SourceChunkIn = (u32, Option<i64>, String, String);
pub type SourceChunkRow = (i64, String, String, u32, Option<i64>, String, String);

impl Store {
    /// The items one source holds: (item key, version, excerpt count).
    pub fn source_item_stamps(&self, source_key: &str) -> Result<Vec<(String, String, u32)>, StoreError> {
        let key = source_key.to_string();
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT item_key, version, excerpt_count FROM source_item WHERE source_key = ?1 ORDER BY item_key")?;
            let rows = st.query_map(params![key], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?.max(0) as u32)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Replaces every excerpt of one item and its stamp, in one transaction (no excerpt = the item is known and holds no text). A blank text is not stored.
    pub fn source_put_item(&self, source_key: &str, item_key: &str, version: &str, excerpt_count: u32, chunks: Vec<SourceChunkIn>) -> Result<(), StoreError> {
        let (key, item, version) = (source_key.to_string(), item_key.to_string(), version.to_string());
        self.write(move |ctx| {
            ctx.tx.execute("DELETE FROM source_chunk WHERE source_key = ?1 AND item_key = ?2", params![key, item])?;
            {
                let mut st = ctx.tx.prepare_cached("INSERT INTO source_chunk (source_key, item_key, chunk_no, date_ms, title, text) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
                for (no, date, title, text) in chunks.iter().filter(|c| !c.3.trim().is_empty()) { st.execute(params![key, item, *no as i64, date, title, text])?; }
            }
            ctx.tx.execute(
                "INSERT INTO source_item (source_key, item_key, version, excerpt_count) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(source_key, item_key) DO UPDATE SET version = excluded.version, excerpt_count = excluded.excerpt_count",
                params![key, item, version, excerpt_count as i64])?;
            Ok(())
        })
    }

    /// Removes one item: its excerpts and its stamp.
    pub fn source_remove_item(&self, source_key: &str, item_key: &str) -> Result<(), StoreError> {
        let (key, item) = (source_key.to_string(), item_key.to_string());
        self.write(move |ctx| {
            ctx.tx.execute("DELETE FROM source_chunk WHERE source_key = ?1 AND item_key = ?2", params![key, item])?;
            ctx.tx.execute("DELETE FROM source_item WHERE source_key = ?1 AND item_key = ?2", params![key, item])?;
            Ok(())
        })
    }

    /// Removes every excerpt and stamp of one source (a folder or calendar taken off the Sources pane). Answers how many excerpts went.
    pub fn source_remove_source(&self, source_key: &str) -> Result<usize, StoreError> {
        let key = source_key.to_string();
        self.write(move |ctx| {
            let n = ctx.tx.execute("DELETE FROM source_chunk WHERE source_key = ?1", params![key])?;
            ctx.tx.execute("DELETE FROM source_item WHERE source_key = ?1", params![key])?;
            Ok(n)
        })
    }

    /// The keys of the sources that hold a stamp or an excerpt.
    pub fn source_keys(&self) -> Result<Vec<String>, StoreError> {
        self.read(|c| {
            let mut st = c.prepare_cached("SELECT source_key FROM source_item UNION SELECT source_key FROM source_chunk ORDER BY 1")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// How many excerpts one source holds.
    pub fn source_chunk_count(&self, source_key: &str) -> Result<u64, StoreError> {
        let key = source_key.to_string();
        self.read(move |c| Ok(c.query_row("SELECT count(*) FROM source_chunk WHERE source_key = ?1", params![key], |r| r.get::<_, i64>(0))?.max(0) as u64))
    }

    /// The best `limit` excerpts of the named sources for `words` (lower-cased, each more than two characters: the caller's `lowered_words`), RANKED BY THE INDEX
    /// (0021's `source_chunk_fts`): an excerpt scores one for each of `words` (a repeated word counts again) that is a word of its title or text, the best score first,
    /// ties by the excerpt's id string (source key, item key and number joined by U+001F: `reply_sources::excerpt_id`, the order the plain scan breaks ties in);
    /// an excerpt that scores none is not read. Answers (score, row), at most `limit` rows: the rows the search READS are these, never every row of the source.
    pub fn source_chunks_ranked(&self, source_keys: Vec<String>, words: Vec<String>, limit: usize) -> Result<Vec<(u32, SourceChunkRow)>, StoreError> {
        if source_keys.is_empty() || words.is_empty() || limit == 0 { return Ok(Vec::new()); }
        self.read(move |c| {
            use rusqlite::types::Value;
            let mut params: Vec<Value> = words.iter().map(|w| Value::Text(format!("\"{}\"", w.replace('"', "\"\"")))).collect();
            let score = (1..=words.len()).map(|k| format!("(c.id IN (SELECT rowid FROM source_chunk_fts WHERE source_chunk_fts MATCH ?{k}))")).collect::<Vec<_>>().join(" + ");
            let first_key = words.len() + 1;
            let keys = (0..source_keys.len()).map(|i| format!("?{}", first_key + i)).collect::<Vec<_>>().join(", ");
            params.extend(source_keys.iter().map(|k| Value::Text(k.clone())));
            params.push(Value::Integer(limit as i64));
            let sql = format!(
                "SELECT {score} AS score, c.id, c.source_key, c.item_key, c.chunk_no, c.date_ms, c.title, c.text FROM source_chunk c WHERE c.source_key IN ({keys}) AND score > 0 \
                 ORDER BY score DESC, c.source_key || char(31) || c.item_key || char(31) || CAST(c.chunk_no AS TEXT) ASC LIMIT ?{}", first_key + source_keys.len());
            let mut st = c.prepare(&sql)?;
            let rows = st.query_map(rusqlite::params_from_iter(params.iter()), |r| Ok((
                r.get::<_, i64>(0)?.max(0) as u32,
                (r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?.max(0) as u32, r.get::<_, Option<i64>>(5)?, r.get::<_, String>(6)?, r.get::<_, String>(7)?),
            )))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// Every excerpt of the named sources, oldest id first (the retrieval ranks them; a plain scan).
    pub fn source_chunks(&self, source_keys: Vec<String>) -> Result<Vec<SourceChunkRow>, StoreError> {
        self.read(move |c| {
            let mut st = c.prepare_cached("SELECT id, source_key, item_key, chunk_no, date_ms, title, text FROM source_chunk WHERE source_key = ?1 ORDER BY id")?;
            let mut out = Vec::new();
            for key in &source_keys {
                let rows = st.query_map(params![key], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?.max(0) as u32, r.get::<_, Option<i64>>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?)))?;
                for row in rows { out.push(row?); }
            }
            Ok(out)
        })
    }
}
