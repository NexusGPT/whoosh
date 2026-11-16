//! Save a draft without sending it (delta 8, group 6). One row in `draft`; the composer reads it back and Send turns it into an
//! outbox entry. A draft with nothing in it is refused: there is nothing to keep.
use crate::engine::Env;
use contracts::{CommandOutcome, CoreError};
use rusqlite::params;
use store::new_id;

pub struct SaveDraft {
    env: Env,
}

fn json_list(items: &[String]) -> String {
    let mut out = String::from("[");
    for (i, s) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    out.push(']');
    out
}

impl SaveDraft {
    pub fn new(env: Env) -> SaveDraft {
        SaveDraft { env }
    }

    pub fn execute(&self, from_account_id: Option<String>, to: Vec<String>, cc: Vec<String>, subject: String, body: String, reply_to_thread_id: Option<String>) -> Result<CommandOutcome, CoreError> {
        if to.iter().all(|a| a.trim().is_empty()) && subject.trim().is_empty() && body.trim().is_empty() {
            return Err(CoreError::Invalid { field: "draft".into(), reason_words: "there is nothing to save: add a recipient, a subject or some words".into() });
        }
        let now = self.env.now();
        let id = new_id("draft");
        let draft_id = id.clone();
        let result = self.env.store.write(move |ctx| {
            let account: Option<String> = match &from_account_id {
                Some(a) => ctx.tx.query_row("SELECT id FROM account WHERE id = ?1", params![a], |r| r.get(0)).ok(),
                None => reply_to_thread_id
                    .as_deref()
                    .and_then(|t| ctx.tx.query_row("SELECT account_id FROM thread WHERE id = ?1", params![t], |r| r.get(0)).ok())
                    .or_else(|| ctx.tx.query_row("SELECT id FROM account ORDER BY sort_order LIMIT 1", [], |r| r.get(0)).ok()),
            };
            let Some(account) = account else { return Ok(None) };
            ctx.tx.execute(
                "INSERT INTO draft (id, from_account_id, to_json, cc_json, subject, body, reply_to_thread_id, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, account, json_list(&to), json_list(&cc), subject, body, reply_to_thread_id, now],
            )?;
            Ok(Some(()))
        });
        match result {
            Ok(Some(())) => Ok(CommandOutcome { undo_entry_id: None, created_id: Some(draft_id) }),
            Ok(None) => Err(CoreError::NotFound { kind: "account".into(), id: "the sending account".into() }),
            Err(e) => Err(CoreError::Unexpected { reason_words: format!("the draft was not saved: {e}") }),
        }
    }

    /// Delta 11, item 8: saves a reopened draft in place. The id and the reply thread stay; `from_account_id` None keeps the
    /// account. No undo entry. Unknown draft or account = NotFound; nothing left in it = the SaveDraft refusal.
    pub fn update(&self, draft_id: String, from_account_id: Option<String>, to: Vec<String>, cc: Vec<String>, bcc: Vec<String>, subject: String, body: String) -> Result<CommandOutcome, CoreError> {
        if to.iter().all(|a| a.trim().is_empty()) && subject.trim().is_empty() && body.trim().is_empty() {
            return Err(CoreError::Invalid { field: "draft".into(), reason_words: "there is nothing to save: add a recipient, a subject or some words".into() });
        }
        let now = self.env.now();
        let id = draft_id.clone();
        let result = self.env.store.write(move |ctx| {
            let current: Option<String> = ctx.tx.query_row("SELECT from_account_id FROM draft WHERE id = ?1", params![id], |r| r.get(0)).ok();
            let Some(current) = current else { return Ok(Err("draft")) };
            let account: Option<String> = match &from_account_id {
                Some(a) => ctx.tx.query_row("SELECT id FROM account WHERE id = ?1", params![a], |r| r.get(0)).ok(),
                None => Some(current),
            };
            let Some(account) = account else { return Ok(Err("account")) };
            ctx.tx.execute(
                "UPDATE draft SET from_account_id = ?2, to_json = ?3, cc_json = ?4, subject = ?5, body = ?6, updated_at_ms = ?7, bcc_json = ?8 WHERE id = ?1",
                params![id, account, json_list(&to), json_list(&cc), subject, body, now, json_list(&bcc)],
            )?;
            Ok(Ok(()))
        });
        match result {
            Ok(Ok(())) => Ok(CommandOutcome { undo_entry_id: None, created_id: Some(draft_id) }),
            Ok(Err("draft")) => Err(CoreError::NotFound { kind: "draft".into(), id: draft_id }),
            Ok(Err(_)) => Err(CoreError::NotFound { kind: "account".into(), id: "the sending account".into() }),
            Err(e) => Err(CoreError::Unexpected { reason_words: format!("the draft was not saved: {e}") }),
        }
    }
}
