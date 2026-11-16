//! Delete a saved draft (`Command::DeleteDraft`), and Z brings it back whole.
//!
//! Lead, 13 Nov (owner pictures 199 "impossible to delete", 205, 207 "commands dn't work when amessage in draft"): the contract has had the
//! command since delta 11 and no crate of the real core ran it. The app's every draft delete (the draft card's Delete, Discard in the reply,
//! the draft that leaves when its words are sent, the pop-out) answered "another part of the core handles this command" and the draft stayed:
//! a card nobody could remove, a draft still shown after it was sent, one more draft in Drafts at every try. The test engine deleted drafts,
//! so no hidden check saw it.
//!
//! The row leaves `draft` and its whole content is kept in the undo entry (JSON, written by SQLite from the row itself), so Z restores the
//! same id, words, recipients, thread and saved time, after a restart too.
use crate::constants::undo_depth;
use crate::engine::{new_id, optional, Env};
use contracts::{CommandOutcome, CoreError};
use rusqlite::params;
use store::{StoreError, WriteCtx};

pub struct DeleteDraft {
    env: Env,
}

impl DeleteDraft {
    pub fn new(env: Env) -> DeleteDraft {
        DeleteDraft { env }
    }

    /// Unknown draft = NotFound. One undo entry, kind `delete_draft`.
    pub fn execute(&self, draft_id: String) -> Result<CommandOutcome, CoreError> {
        let now = self.env.now();
        let id = draft_id.clone();
        let result = self.env.store.write(move |ctx| {
            let known: Option<i64> = optional(ctx.tx.query_row("SELECT 1 FROM draft WHERE id = ?1", params![id], |r| r.get(0)))?;
            if known.is_none() {
                return Ok(None);
            }
            let entry = new_id("undo");
            ctx.tx.execute(
                "INSERT INTO undo_entry (id, action_json, at_ms, consumed) \
                 SELECT ?1, json_object('kind','delete_draft','id',id,'from',from_account_id,'to',json(to_json),'cc',json(cc_json),'bcc',json(bcc_json),\
                        'subject',subject,'body',body,'attachment_ids',json(attachment_ids_json),'ai_marks',json(ai_marks_json),\
                        'thread',reply_to_thread_id,'saved_at',updated_at_ms,'remind',remind_if_no_reply_at_ms), ?2, 0 \
                 FROM draft WHERE id = ?3",
                params![entry, now, id],
            )?;
            ctx.tx.execute("DELETE FROM draft WHERE id = ?1", params![id])?;
            ctx.tx.execute(
                "DELETE FROM undo_entry WHERE consumed = 0 AND id NOT IN (SELECT id FROM undo_entry WHERE consumed = 0 ORDER BY at_ms DESC, rowid DESC LIMIT ?1)",
                params![undo_depth() as i64],
            )?;
            Ok(Some(entry))
        });
        match result {
            Ok(Some(entry)) => Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: None }),
            Ok(None) => Err(CoreError::NotFound { kind: "draft".into(), id: draft_id }),
            Err(e) => Err(CoreError::Unexpected { reason_words: format!("the draft was not deleted: {e}") }),
        }
    }
}

/// Z after a deleted draft: the row comes back from the entry. False when it cannot (a draft of that id exists again, or its account is gone).
pub(crate) fn restore(ctx: &mut WriteCtx<'_>, entry: &str) -> Result<bool, StoreError> {
    let n = ctx.tx.execute(
        "INSERT OR IGNORE INTO draft (id, from_account_id, to_json, cc_json, bcc_json, subject, body, attachment_ids_json, ai_marks_json, \
                                      reply_to_thread_id, updated_at_ms, remind_if_no_reply_at_ms) \
         SELECT json_extract(a,'$.id'), json_extract(a,'$.from'), json_extract(a,'$.to'), json_extract(a,'$.cc'), json_extract(a,'$.bcc'), \
                json_extract(a,'$.subject'), json_extract(a,'$.body'), json_extract(a,'$.attachment_ids'), json_extract(a,'$.ai_marks'), \
                json_extract(a,'$.thread'), json_extract(a,'$.saved_at'), json_extract(a,'$.remind') \
         FROM (SELECT action_json AS a FROM undo_entry WHERE id = ?1) \
         WHERE json_extract(a,'$.kind') = 'delete_draft' AND json_extract(a,'$.from') IN (SELECT id FROM account)",
        params![entry],
    )?;
    Ok(n > 0)
}
