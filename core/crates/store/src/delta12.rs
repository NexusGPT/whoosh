//! contracts delta 12: the drafts replying in a thread, with their header, for `drafts(thread_id)`.
//! real-folders (SPEC A153, "a saved folder keeps the sender-domain filter"): the smart folders of the owner, kept in table `smart_folder`
//! (migration 0001): the list in tree order, save, rename, change (rule and parent) and delete, and the members of a rule's hits.
//! fix-nesting (J2.6): a folder's parent is another smart folder (column `parent_id`) OR a label (column `parent_label_id`, migration 0029);
//! at most one is set, and every read answers whichever is set as the contract's one `SmartFolder.parent_id`.
use crate::{Store, StoreError};
use contracts::{CoreError, DraftSummary, InboxRow, SmartFolder};
use rusqlite::{params, Connection};

/// The refusals of the smart-folder writes, in the contract's words (the fixture core's own: Sources/CoreBridge/FixtureCore.swift,
/// FixtureLabels.swift). The store decides them inside the one write transaction, so two writes cannot both pass the same check.
mod refuse {
    use contracts::CoreError;
    pub fn needs_name() -> CoreError { CoreError::Invalid { field: "name".into(), reason_words: "a smart folder needs a name".into() } }
    pub fn needs_rule() -> CoreError { CoreError::Invalid { field: "rule".into(), reason_words: "a smart folder needs a rule".into() } }
    pub fn name_taken_on_save() -> CoreError { CoreError::Invalid { field: "name".into(), reason_words: "the name exists already".into() } }
    pub fn name_taken_on_rename(name: &str) -> CoreError { CoreError::Invalid { field: "name".into(), reason_words: format!("there is already a smart folder named \"{name}\"") } }
    pub fn not_found(id: &str) -> CoreError { CoreError::NotFound { kind: "smart folder".into(), id: id.into() } }
    pub fn parent_is_self() -> CoreError { CoreError::Invalid { field: "parent".into(), reason_words: "a folder cannot sit inside itself".into() } }
    pub fn parent_inside_itself() -> CoreError { CoreError::Invalid { field: "parent".into(), reason_words: "a folder cannot sit inside one of its own folders".into() } }
}

/// The rows in tree order: each parent before its children, the roots and each folder's children in the order saved. A row whose
/// parent is not in the list counts as a root; a loop (the writes below never make one) is appended last so no row is lost.
fn tree_order(rows: Vec<SmartFolder>) -> Vec<SmartFolder> {
    use std::collections::{HashMap, HashSet};
    let known: HashSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    let mut kids: HashMap<Option<&str>, Vec<usize>> = HashMap::new();
    for (i, r) in rows.iter().enumerate() {
        let key = r.parent_id.as_deref().filter(|p| known.contains(p));
        kids.entry(key).or_default().push(i);
    }
    let mut order: Vec<usize> = Vec::with_capacity(rows.len());
    let mut seen = vec![false; rows.len()];
    let mut stack: Vec<usize> = kids.get(&None).map(|v| v.iter().rev().copied().collect()).unwrap_or_default();
    while let Some(i) = stack.pop() {
        if std::mem::replace(&mut seen[i], true) { continue; }
        order.push(i);
        if let Some(v) = kids.get(&Some(rows[i].id.as_str())) { stack.extend(v.iter().rev().copied()); }
    }
    for (i, done) in seen.iter().enumerate() { if !done { order.push(i); } }
    let mut slots: Vec<Option<SmartFolder>> = rows.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| slots[i].take()).collect()
}

fn all_folders(c: &Connection) -> Result<Vec<SmartFolder>, StoreError> {
    let mut st = c.prepare_cached("SELECT id, name, rule_words, COALESCE(parent_id, parent_label_id) FROM smart_folder ORDER BY rowid")?;
    let rows = st.query_map([], |r| Ok(SmartFolder { id: r.get(0)?, name: r.get(1)?, rule_words: r.get(2)?, parent_id: r.get(3)? }))?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

impl Store {
    /// Every smart folder in the owner's order, parents before children (the sidebar's list, `WhooshCore::smart_folders`).
    pub fn smart_folders(&self) -> Result<Vec<SmartFolder>, StoreError> {
        self.read(|c| Ok(tree_order(all_folders(c)?)))
    }

    /// Saves a new folder at the top level and answers its id (`CoreError` in words when refused). Refused: no name, a name another folder has (any case), no rule.
    pub fn smart_folder_save(&self, name: &str, rule_words: &str, now_ms: i64) -> Result<Result<String, CoreError>, StoreError> {
        let (name, rule) = (name.trim().to_string(), rule_words.trim().to_string());
        self.write(move |c| {
            if name.is_empty() { return Ok(Err(refuse::needs_name())); }
            if rule.is_empty() { return Ok(Err(refuse::needs_rule())); }
            let folders = all_folders(c.tx)?;
            let lower = name.to_lowercase();
            if folders.iter().any(|f| f.name.to_lowercase() == lower) { return Ok(Err(refuse::name_taken_on_save())); }
            let id = crate::labels::new_id("sf");
            c.tx.execute("INSERT INTO smart_folder (id, name, rule_words, parent_id, updated_at_ms) VALUES (?1, ?2, ?3, NULL, ?4)", params![id, name, rule, now_ms])?;
            Ok(Ok(id))
        })
    }

    /// Renames a folder. Refused: an unknown folder, no name, a name another folder has (any case; its own name in another case is allowed).
    pub fn smart_folder_rename(&self, id: &str, name: &str, now_ms: i64) -> Result<Result<(), CoreError>, StoreError> {
        let (id, name) = (id.to_string(), name.trim().to_string());
        self.write(move |c| {
            let folders = all_folders(c.tx)?;
            if !folders.iter().any(|f| f.id == id) { return Ok(Err(refuse::not_found(&id))); }
            if name.is_empty() { return Ok(Err(refuse::needs_name())); }
            let lower = name.to_lowercase();
            if folders.iter().any(|f| f.id != id && f.name.to_lowercase() == lower) { return Ok(Err(refuse::name_taken_on_rename(&name))); }
            c.tx.execute("UPDATE smart_folder SET name = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, name, now_ms])?;
            Ok(Ok(()))
        })
    }

    /// Rewrites a folder's rule and parent (`None` = the top level). The parent is a smart folder (by id, else by name, any case) or, when no
    /// smart folder matches, a label: by id, else by name as the label commands find one (`labels::find_label`: its path, the case ignored,
    /// the oldest first). A smart folder of that name wins over a label (the app's own order, LabelsContactsModel.swift:80).
    /// Refused: an unknown folder, no rule, a parent that is the folder itself, sits inside it, or is neither a smart folder nor a label.
    pub fn smart_folder_update(&self, id: &str, rule_words: &str, parent_id: Option<&str>, now_ms: i64) -> Result<Result<(), CoreError>, StoreError> {
        let (id, rule, parent) = (id.to_string(), rule_words.trim().to_string(), parent_id.map(str::to_string));
        self.write(move |c| {
            let folders = all_folders(c.tx)?;
            if !folders.iter().any(|f| f.id == id) { return Ok(Err(refuse::not_found(&id))); }
            if rule.is_empty() { return Ok(Err(refuse::needs_rule())); }
            // (a smart folder parent, a label parent): at most one is set
            let mut to: (Option<String>, Option<String>) = (None, None);
            if let Some(p) = &parent {
                // a smart folder by id, else by name (any case, its own name left out as the app leaves it out): a smart folder wins over a
                // label of the same name
                let lower = p.to_lowercase();
                let folder = folders.iter().find(|f| f.id == *p)
                    .or_else(|| folders.iter().find(|f| f.id != id && f.name.to_lowercase() == lower))
                    .map(|f| f.id.clone());
                if let Some(fp) = folder {
                    if fp == id { return Ok(Err(refuse::parent_is_self())); }
                    // walk up from the new parent: meeting this folder means the move would make a loop
                    let mut at = Some(fp.clone());
                    for _ in 0..=folders.len() {
                        let Some(cur) = at else { break };
                        if cur == id { return Ok(Err(refuse::parent_inside_itself())); }
                        at = folders.iter().find(|f| f.id == cur).and_then(|f| f.parent_id.clone());
                    }
                    to.0 = Some(fp);
                } else if *p == id {
                    return Ok(Err(refuse::parent_is_self()));
                } else if let Some(label) = crate::labels::get_label(c.tx, p)?.or(crate::labels::find_label(c.tx, p)?) {
                    // a label cannot sit inside a smart folder, so a label parent can make no loop
                    to.1 = Some(label.id);
                } else {
                    return Ok(Err(refuse::not_found(p)));
                }
            }
            c.tx.execute(
                "UPDATE smart_folder SET rule_words = ?2, parent_id = ?3, parent_label_id = ?4, updated_at_ms = ?5 WHERE id = ?1",
                params![id, rule, to.0, to.1, now_ms],
            )?;
            Ok(Ok(()))
        })
    }

    /// Deletes a folder; the folders inside it move up to its parent (a smart folder or a label, on its own column). No mail is touched (a
    /// folder holds a rule, not messages).
    pub fn smart_folder_delete(&self, id: &str) -> Result<Result<(), CoreError>, StoreError> {
        let id = id.to_string();
        self.write(move |c| {
            use rusqlite::OptionalExtension;
            let up: Option<(Option<String>, Option<String>)> = c.tx
                .query_row("SELECT parent_id, parent_label_id FROM smart_folder WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            let Some((parent, parent_label)) = up else { return Ok(Err(refuse::not_found(&id))) };
            c.tx.execute("UPDATE smart_folder SET parent_id = ?2, parent_label_id = ?3 WHERE parent_id = ?1", params![id, parent, parent_label])?;
            c.tx.execute("DELETE FROM smart_folder WHERE id = ?1", params![id])?;
            Ok(Ok(()))
        })
    }

    /// Of these message ids (the hits of a folder's rule), the ones the folder lists: mail still in the inbox (not archived, muted or
    /// snoozed, no sent copy) from someone other than the owner's own addresses, newest first (ties by id), each with whether it is
    /// unread. One statement on the reader, so a folder's count, its unread figure and its list are read from the same rows.
    pub fn smart_folder_members(&self, ids: &[String]) -> Result<Vec<(String, bool)>, StoreError> {
        if ids.is_empty() { return Ok(Vec::new()); }
        let json = crate::ops::json_strings(ids);
        self.read(move |c| {
            let mut st = c.prepare_cached(&format!(
                "SELECT m.id, m.read FROM message m WHERE m.id IN (SELECT value FROM json_each(?1)) AND {} \
                 AND lower(m.from_address) NOT IN (SELECT lower(address) FROM account) ORDER BY m.received_at_ms DESC, m.id",
                crate::read::VISIBLE
            ))?;
            let rows = st.query_map(params![json], |r| Ok((r.get::<_, String>(0)?, !r.get::<_, bool>(1)?)))?.collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
    }

    /// One smart folder as saved (delta 14), or None.
    pub fn smart_folder(&self, id: &str) -> Result<Option<SmartFolder>, StoreError> {
        let id = id.to_string();
        self.read(move |c| {
            use rusqlite::OptionalExtension;
            Ok(c.query_row("SELECT id, name, rule_words, COALESCE(parent_id, parent_label_id) FROM smart_folder WHERE id = ?1", params![id], |r| {
                Ok(SmartFolder { id: r.get(0)?, name: r.get(1)?, rule_words: r.get(2)?, parent_id: r.get(3)? })
            }).optional()?)
        })
    }

    /// The list rows of these messages, in the order given (delta 14: a smart folder's search results). Only a message the
    /// inbox shows is kept (in the inbox, not archived, not snoozed): an archive takes a row out of its smart folder, as the
    /// sidebar's count expects. A message no longer in the store is left out.
    pub fn rows_for(&self, ids: &[String]) -> Result<Vec<InboxRow>, StoreError> {
        let ids = ids.to_vec();
        self.read(move |c| {
            let sql = format!("SELECT EXISTS (SELECT 1 FROM message m WHERE m.id = ?1 AND {})", crate::read::VISIBLE);
            let mut st = c.prepare_cached(&sql)?;
            let mut kept = Vec::with_capacity(ids.len());
            for id in ids { if st.query_row(params![id], |r| r.get::<_, bool>(0))? { kept.push(id); } }
            crate::read::project(c, &kept)
        })
    }

    /// The drafts saved against a thread, newest first, each with its account, its first DRAFT_SUMMARY_LINES lines and
    /// its header (To, Cc, Bcc, subject), so a reopened draft restores the header and not only the words.
    pub fn draft_summaries(&self, thread_id: &str) -> Result<Vec<DraftSummary>, StoreError> {
        let t = thread_id.to_string();
        self.read(move |c| draft_summaries_of(c, &t))
    }
}

fn strings(conn: &Connection, json: &str) -> Result<Vec<String>, StoreError> {
    let mut st = conn.prepare_cached("SELECT value FROM json_each(?1) ORDER BY key")?;
    let out = st.query_map(params![json], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

fn draft_summaries_of(conn: &Connection, thread_id: &str) -> Result<Vec<DraftSummary>, StoreError> {
    let mut st = conn.prepare_cached(
        "SELECT id, from_account_id, updated_at_ms, body, to_json, cc_json, bcc_json, subject FROM draft \
         WHERE reply_to_thread_id = ?1 ORDER BY updated_at_ms DESC, id DESC",
    )?;
    let rows = st
        .query_map(params![thread_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?,
                r.get::<_, String>(5)?, r.get::<_, String>(6)?, r.get::<_, String>(7)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, account_id, saved_at, body, to, cc, bcc, subject)| {
            Ok(DraftSummary {
                id,
                thread_id: thread_id.to_string(),
                account_id,
                saved_at,
                first_lines: contracts::draft_first_lines(body, contracts::constants::DRAFT_SUMMARY_LINES as u32),
                to: strings(conn, &to)?,
                cc: strings(conn, &cc)?,
                bcc: strings(conn, &bcc)?,
                subject,
            })
        })
        .collect()
}
