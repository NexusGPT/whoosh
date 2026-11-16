//! "Essayer sur un message" (J10.4, J10.6), on the interactive thread: retrieval and the three steps on one message, and the text a draft would have
//! when every step lets it through. STORES NO DRAFT, writes no pass row, changes no message: the report is kept in memory (`reply/try/<run id>`).
//! On demand no filter applies (the owner picks the thread), so its three steps read Skipped.
use contracts::{
    reply_ai_step, reply_entry_step, reply_exit_step, rule_set_matches, CoreError, MatchMode, ReplyFilter, ReplyRetrievalReport, ReplyStepReport, ReplyStepResult,
    ReplyTryReport, ReplyUseKind, RuleCondition, RuleSet,
};

use super::decide::decide;
use super::write_draft::{assemble, retrieve, write, Ports};
use super::{through, words};
use crate::replies_work::adapters::excerpt_index::Index;
use crate::replies_work::adapters::mail_reads::MailReads;
use crate::replies_work::limits::{REPLY_TRY_REPORTS_KEPT, SHARE_THREAD};
use crate::replies_work::model::TextModel;
use crate::replies_work::Shared;

/// The conditions of a rule that hold for the message (each read alone).
fn matched(rule: &RuleSet, facts: &contracts::MessageFacts) -> Vec<RuleCondition> {
    rule.conditions.iter().filter(|c| rule_set_matches(RuleSet { conditions: vec![(*c).clone()], match_mode: MatchMode::Any }, facts.clone())).cloned().collect()
}

fn skipped() -> ReplyStepReport { ReplyStepReport { result: ReplyStepResult::Skipped, matched: vec![], model_note: None } }

fn report(s: &Shared, run_id: &str, message_id: &str, kind: ReplyUseKind, model: &dyn TextModel) -> Result<ReplyTryReport, CoreError> {
    let reads = MailReads::open(&s.store)?;
    let f = reads.facts(message_id, model.prompt_chars() / SHARE_THREAD)?.ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: message_id.to_string() })?;
    let (_, attachments) = reads.folder_and_attachments(message_id)?;
    let index = Index { store: &s.store, bodies: s.bodies.upgrade(), memory: &s.memory };
    let ports = Ports { store: &s.store, index: &index, sent: &reads, model, memory: &s.memory };
    let facts = f.rule_facts();
    let (entry, ai, exit) = if kind == ReplyUseKind::OnDemand { (skipped(), skipped(), skipped()) } else {
        let filter: ReplyFilter = s.store.reply_filter()?;
        let entry = ReplyStepReport { result: reply_entry_step(filter.clone(), f.label_ids.clone(), facts.clone()), matched: matched(&filter.entry_rule, &facts), model_note: None };
        let ai = if !through(entry.result) { ReplyStepReport { result: ReplyStepResult::NotReached, matched: vec![], model_note: None } }
            else if !filter.ai_enabled { skipped() }
            else { let (verdict, note) = decide(model, &filter, &f, &attachments); ReplyStepReport { result: reply_ai_step(filter.clone(), verdict), matched: vec![], model_note: note } };
        let exit = if !through(ai.result) { ReplyStepReport { result: ReplyStepResult::NotReached, matched: vec![], model_note: None } }
            else { ReplyStepReport { result: reply_exit_step(filter.clone(), facts.clone()), matched: matched(&filter.exit_rule, &facts), model_note: None } };
        (entry, ai, exit)
    };
    let all_through = through(entry.result) && through(ai.result) && through(exit.result);
    let (retrieval, draft_text): (ReplyRetrievalReport, Option<String>) = if all_through {
        let assembled = assemble(&ports, kind, &f, None)?;
        let text = write(model, &assembled.prompt).map_err(|e| { s.log(&format!("the try {run_id} wrote no text: {}", words(&e))); e }).ok();
        (assembled.retrieval.report(), text)
    } else {
        (retrieve(&ports, &f, None)?.report(), None)
    };
    Ok(ReplyTryReport { run_id: run_id.to_string(), message_id: message_id.to_string(), kind, retrieval, entry, ai, exit, draft_text })
}

pub(crate) fn run(s: &Shared, run_id: &str, message_id: &str, kind: ReplyUseKind, model: &dyn TextModel) {
    match report(s, run_id, message_id, kind, model) {
        Ok(r) => {
            let mut kept = s.tries.lock().unwrap_or_else(|e| e.into_inner());
            kept.push_back(r);
            while kept.len() > REPLY_TRY_REPORTS_KEPT { kept.pop_front(); }
        }
        Err(e) => s.log(&format!("the try {run_id} on {message_id} did not run: {}", words(&e))),
    }
}
