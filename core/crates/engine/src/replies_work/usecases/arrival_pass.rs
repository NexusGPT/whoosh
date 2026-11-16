//! The auto use on arrival (J10.8), on the background thread, after the label flow filed what the sync stored: when "Brouillon automatique" is on,
//! each new INBOX message the owner did not write goes through the three steps in order (Filtre d'entrée: the label ticks and the entry rule, on the
//! label flow's facts and rule engine; Décision par l'IA: one yes/no question; Filtre de sortie: the exit rule), its verdicts are kept as ONE
//! `reply_pass` row (what `reply/counts` counts), and a message all three let through gets a draft (made_by auto) in the same transaction as its row.
//! Never for a message the owner sent, never twice for one message (a row whose model step answered is never redone).
use contracts::{reply_ai_step, reply_entry_step, reply_exit_step, CoreError, ReplyPass, ReplyStepResult, ReplyUseKind};
use store::INBOX_FOLDER;

use super::decide::decide;
use super::draft_on_demand::draft_for;
use super::{through, words};
use crate::replies_work::adapters::mail_reads::MailReads;
use crate::replies_work::limits::SHARE_THREAD;
use crate::replies_work::Shared;

/// What happened to one message (the tests read the pass rows; this is for the log).
fn one(s: &Shared, reads: &MailReads, id: &str) -> Result<(), CoreError> {
    if let Some(p) = s.store.reply_pass_of(id)? {
        // a model step that answered (or a stop before it) is never asked again; a model that did not answer is asked at the next arrival of the id
        if p.draft_id.is_some() || p.ai != ReplyStepResult::NotReached || !through(p.entry) { return Ok(()); }
    }
    let model = s.model(crate::ai_settings::AiJob::Draft);
    let room = model.as_ref().map(|m| m.prompt_chars()).unwrap_or(0);
    let (folder, attachments) = reads.folder_and_attachments(id)?;
    if folder != INBOX_FOLDER { return Ok(()); }
    let Some(f) = reads.facts(id, room / SHARE_THREAD)? else { return Ok(()) };
    if f.from_owner { return Ok(()); }
    let filter = s.store.reply_filter()?;
    let facts = f.rule_facts();
    let entry = reply_entry_step(filter.clone(), f.label_ids.clone(), facts.clone());
    let ai = if !through(entry) { ReplyStepResult::NotReached } else if !filter.ai_enabled { reply_ai_step(filter.clone(), None) } else {
        match &model {
            Ok(m) => reply_ai_step(filter.clone(), decide(&**m, &filter, &f, &attachments).0),
            Err(e) => { s.log(&format!("the auto use did not ask the model about {id}: {}", words(e))); ReplyStepResult::NotReached }
        }
    };
    let exit = if through(ai) { reply_exit_step(filter, facts) } else { ReplyStepResult::NotReached };
    let pass = ReplyPass { message_id: id.to_string(), received_at_ms: f.received_at_ms, entry, ai, exit, draft_id: None };
    if !(through(entry) && through(ai) && through(exit)) { return s.store.reply_record_pass(pass); }
    let written = model.and_then(|m| draft_for(s, &store::new_id("reply"), id, ReplyUseKind::Auto, None, &*m, Some(pass.clone())));
    if let Err(e) = written {
        s.log(&format!("the auto draft of {id} was not written: {}", words(&e)));
        // the three steps let it through: the row says so, with no draft
        s.store.reply_record_pass(pass)?;
    }
    Ok(())
}

pub(crate) fn run(s: &Shared, ids: &[String]) {
    match s.store.reply_use(ReplyUseKind::Auto) {
        Ok(u) if u.enabled => {}
        Ok(_) => return,
        Err(e) => return s.log(&format!("the auto use's switch could not be read: {}", words(&e))),
    }
    let reads = match MailReads::open(&s.store) { Ok(r) => r, Err(e) => return s.log(&format!("the auto use could not read the new mail: {}", words(&e))) };
    for id in ids {
        s.wait_unpaused();
        if let Err(e) = one(s, &reads, id) { s.log(&format!("the auto use stopped on {id}: {}", words(&e))); }
    }
}
