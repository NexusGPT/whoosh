//! Typed writes that move or remove messages: a folder move (Trash, Spam, back), an account move, a delete, plus the small
//! reads those use cases need. Each declares the messages it changes before changing them, so the counts move by a difference.
//! They are `WriteCtx` methods, so a use case can run them inside its own transaction.
use crate::error::StoreError;
use crate::writer::WriteCtx;
use crate::Store;
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeSet;

/// What an account move changed, so the same move can be put back exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountMove {
    pub from_account_id: String,
    pub to_account_id: String,
    /// Each moved message with the `remote_id` it had (the target server issues its own; sync fills it in later).
    pub moved: Vec<(String, Option<String>)>,
    /// Threads that moved with their messages (every message of the thread moved), with the account they came from.
    pub threads: Vec<(String, String)>,
}

impl AccountMove {
    pub fn moved_ids(&self) -> Vec<String> {
        self.moved.iter().map(|(id, _)| id.clone()).collect()
    }
}

impl WriteCtx<'_> {
    /// Put each (message, folder) pair in its folder. Returns how many rows changed; a message already there is left alone.
    pub fn set_folders(&mut self, moves: &[(String, String)]) -> Result<usize, StoreError> {
        let ids: Vec<String> = moves.iter().map(|(id, _)| id.clone()).collect();
        self.will_change(&ids)?;
        let mut st = self.tx.prepare_cached("UPDATE message SET folder = ?2 WHERE id = ?1 AND folder <> ?2")?;
        let mut changed = 0;
        for (id, folder) in moves {
            changed += st.execute(params![id, folder])?;
        }
        Ok(changed)
    }

    /// Put all of `ids` in one folder.
    pub fn set_folder(&mut self, ids: &[String], folder: &str) -> Result<usize, StoreError> {
        let moves: Vec<(String, String)> = ids.iter().map(|i| (i.clone(), folder.to_string())).collect();
        self.set_folders(&moves)
    }

    pub fn account_exists(&self, account_id: &str) -> Result<bool, StoreError> {
        Ok(self.tx.prepare_cached("SELECT 1 FROM account WHERE id = ?1")?.exists(params![account_id])?)
    }

    /// The name the owner gave the account, when it exists.
    pub fn account_display_name(&self, account_id: &str) -> Result<Option<String>, StoreError> {
        Ok(self.tx.prepare_cached("SELECT display_name FROM account WHERE id = ?1")?.query_row(params![account_id], |r| r.get(0)).optional()?)
    }

    /// Move messages to another account. A thread whose every message moves moves with them. `remote_id` is cleared: the
    /// target server issues its own id and sync fills it in when it sees the copy. Messages already in the target stay out.
    pub fn move_to_account(&mut self, ids: &[String], target: &str) -> Result<AccountMove, StoreError> {
        if !self.account_exists(target)? {
            return Err(StoreError::Invalid(format!("the account {target} does not exist")));
        }
        let mut rows: Vec<(String, String, String, Option<String>)> = Vec::new();
        {
            let mut st = self.tx.prepare_cached("SELECT id, account_id, thread_id, remote_id FROM message WHERE id = ?1")?;
            for id in ids {
                match st.query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional()? {
                    Some(row) if row.1 != target => rows.push(row),
                    Some(_) => {}
                    None => return Err(StoreError::Invalid(format!("the message {id} does not exist"))),
                }
            }
        }
        let source = rows.first().map(|r| r.1.clone()).unwrap_or_default();
        if rows.iter().any(|r| r.1 != source) {
            return Err(StoreError::Invalid("move messages from one account at a time".into()));
        }
        let moved_ids: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
        let mut threads: Vec<(String, String)> = Vec::new();
        for t in rows.iter().map(|r| r.2.clone()).collect::<BTreeSet<_>>() {
            let total: i64 = self.tx.query_row("SELECT count(*) FROM message WHERE thread_id = ?1", params![t], |r| r.get(0))?;
            let here = rows.iter().filter(|r| r.2 == t).count() as i64;
            if total == here {
                let from: Option<String> = self.tx.query_row("SELECT account_id FROM thread WHERE id = ?1", params![t], |r| r.get(0)).optional()?;
                if let Some(acc) = from {
                    threads.push((t, acc));
                }
            }
        }
        self.will_change(&moved_ids)?;
        for id in &moved_ids {
            self.tx.execute("UPDATE message SET account_id = ?1, remote_id = NULL WHERE id = ?2", params![target, id])?;
        }
        for (t, _) in &threads {
            self.tx.execute("UPDATE thread SET account_id = ?1 WHERE id = ?2", params![target, t])?;
        }
        Ok(AccountMove { from_account_id: source, to_account_id: target.to_string(), moved: rows.into_iter().map(|r| (r.0, r.3)).collect(), threads })
    }

    /// Put back the messages of an earlier `move_to_account` that are named in `only`: their account, their `remote_id`, and the
    /// threads that moved with them.
    pub fn restore_account_move(&mut self, moved: &AccountMove, only: &[String]) -> Result<usize, StoreError> {
        let back: Vec<&(String, Option<String>)> = moved.moved.iter().filter(|(id, _)| only.contains(id)).collect();
        if back.is_empty() {
            return Ok(0);
        }
        let ids: Vec<String> = back.iter().map(|(id, _)| id.clone()).collect();
        self.will_change(&ids)?;
        for (id, remote) in &back {
            self.tx.execute("UPDATE message SET account_id = ?1, remote_id = ?2 WHERE id = ?3", params![moved.from_account_id, remote, id])?;
        }
        for (t, acc) in &moved.threads {
            self.tx.execute("UPDATE thread SET account_id = ?1 WHERE id = ?2", params![acc, t])?;
        }
        Ok(back.len())
    }

    /// Every message of the threads the named messages belong to (the named ones included), each once, in the order found.
    pub fn thread_member_ids(&self, message_ids: &[String]) -> Result<Vec<String>, StoreError> {
        let mut out: Vec<String> = Vec::new();
        let mut st = self.tx.prepare_cached("SELECT id FROM message WHERE thread_id = (SELECT thread_id FROM message WHERE id = ?1)")?;
        for id in message_ids {
            for m in st.query_map(params![id], |r| r.get::<_, String>(0))? {
                let m = m?;
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
        Ok(out)
    }

    /// The accounts that hold mail in any of `folders` (one account only when `only` names it), in id order.
    pub fn accounts_with_mail_in(&self, folders: &[&str], only: Option<&str>) -> Result<Vec<String>, StoreError> {
        let marks = vec!["?"; folders.len()].join(",");
        let sql = format!("SELECT DISTINCT account_id FROM message WHERE folder IN ({marks}) AND (?{n} IS NULL OR account_id = ?{n}) ORDER BY account_id", n = folders.len() + 1);
        let mut st = self.tx.prepare_cached(&sql)?;
        let mut args: Vec<&dyn rusqlite::ToSql> = folders.iter().map(|f| f as &dyn rusqlite::ToSql).collect();
        args.push(&only);
        let rows = st.query_map(rusqlite::params_from_iter(args), |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Delete every message one account holds in `folders`, then the threads that were left with no message. Returns the ids
    /// deleted. A delete cannot be taken back: the caller asks the server first.
    pub fn delete_folder_messages(&mut self, account_id: &str, folders: &[&str]) -> Result<Vec<String>, StoreError> {
        let marks = vec!["?"; folders.len()].join(",");
        let ids: Vec<String> = {
            let sql = format!("SELECT id FROM message WHERE account_id = ?1 AND folder IN ({marks}) ORDER BY id");
            let mut st = self.tx.prepare_cached(&sql)?;
            let mut args: Vec<&dyn rusqlite::ToSql> = vec![&account_id];
            args.extend(folders.iter().map(|f| f as &dyn rusqlite::ToSql));
            let rows = st.query_map(rusqlite::params_from_iter(args), |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        self.delete_messages(&ids)?;
        Ok(ids)
    }

    /// Delete the named messages (their pins, snoozes, labels and attachments go with them), then the threads left empty.
    pub fn delete_messages(&mut self, ids: &[String]) -> Result<usize, StoreError> {
        let mut threads: BTreeSet<String> = BTreeSet::new();
        {
            let mut find = self.tx.prepare_cached("SELECT thread_id FROM message WHERE id = ?1")?;
            for id in ids {
                if let Some(t) = find.query_row(params![id], |r| r.get::<_, String>(0)).optional()? {
                    threads.insert(t);
                }
            }
        }
        // A thread's "waiting" belongs to its newest message. Deleting the newest hands that to the one before it, so that
        // message is declared too, before anything is deleted.
        let mut declare: Vec<String> = ids.to_vec();
        {
            let mut members = self.tx.prepare_cached("SELECT id FROM message WHERE thread_id = ?1 ORDER BY received_at_ms DESC, id DESC")?;
            for t in &threads {
                for row in members.query_map(params![t], |r| r.get::<_, String>(0))? {
                    let id = row?;
                    if !ids.contains(&id) {
                        declare.push(id);
                        break;
                    }
                }
            }
        }
        self.will_change(&declare)?;
        let mut removed = 0;
        {
            let mut del = self.tx.prepare_cached("DELETE FROM message WHERE id = ?1")?;
            for id in ids {
                removed += del.execute(params![id])?;
            }
        }
        let mut empty = self.tx.prepare_cached("DELETE FROM thread WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM message WHERE message.thread_id = thread.id)")?;
        for t in &threads {
            empty.execute(params![t])?;
        }
        Ok(removed)
    }
}

impl Store {
    pub fn set_folder(&self, ids: Vec<String>, folder: String) -> Result<usize, StoreError> {
        self.write(move |ctx| ctx.set_folder(&ids, &folder))
    }

    pub fn move_to_account(&self, ids: Vec<String>, target: String) -> Result<AccountMove, StoreError> {
        self.write(move |ctx| ctx.move_to_account(&ids, &target))
    }

    pub fn delete_messages(&self, ids: Vec<String>) -> Result<usize, StoreError> {
        self.write(move |ctx| ctx.delete_messages(&ids))
    }

    /// Every unread message id of one inbox view, without building a single row (see `read::unread_ids`).
    pub fn unread_ids(&self, query: &contracts::InboxQuery) -> Result<Vec<String>, StoreError> {
        self.read(|c| crate::read::unread_ids(c, query))
    }

    /// The name the owner gave an account.
    pub fn account_display_name(&self, account_id: &str) -> Result<Option<String>, StoreError> {
        self.read(|c| Ok(c.prepare_cached("SELECT display_name FROM account WHERE id = ?1")?.query_row(params![account_id], |r| r.get(0)).optional()?))
    }
}
