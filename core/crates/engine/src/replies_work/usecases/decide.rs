//! The auto use's second step, "Décision par l'IA": ONE yes or no question, the owner's description, over what `ai_reads` ticks (subject, body,
//! attachment names, the rest of the thread cut to its share of the model's room). The answer's first word is read as reply-retrieval's judge reads
//! it (yes, oui, true / no, non, false); anything else, or a model that fails, is no answer (`reply_ai_step` then leaves the message NotReached).
use contracts::ReplyFilter;
use decision::thread_context::{fit, render};
use reply_retrieval::usecases::ai_filter::read_yes_no;
use smart_inbox::MessageFacts;

use super::words;
use crate::replies_work::adapters::model_ports::Calls;
use crate::replies_work::adapters::provider::cut;
use crate::replies_work::limits::{SHARE_BODY, SHARE_THREAD};
use crate::replies_work::model::{CallKind, TextModel};

/// The question as the model reads it.
pub(crate) fn decide_prompt(filter: &ReplyFilter, f: &MessageFacts, attachments: &[String], room: usize) -> String {
    let r = filter.ai_reads;
    let mut out = format!("{}\nAnswer yes or no first, then one short line that says why.\n", filter.ai_description.trim());
    if r.subject { out.push_str(&format!("\nSubject: {}", f.subject)); }
    if r.body { out.push_str(&format!("\nBody:\n{}", cut(&f.body, room / SHARE_BODY))); }
    if r.attachments { out.push_str(&format!("\nAttachments: {}", if attachments.is_empty() { "none".to_string() } else { attachments.join(", ") })); }
    if r.rest_of_thread {
        let thread = render(&fit(&f.thread_before, room / SHARE_THREAD));
        if !thread.is_empty() { out.push_str(&format!("\n\n{thread}")); }
    }
    out
}

/// The model's verdict (None = no answer) and its one line (the model's words, or the failure's), for the try's report.
pub(crate) fn decide(model: &dyn TextModel, filter: &ReplyFilter, f: &MessageFacts, attachments: &[String]) -> (Option<bool>, Option<String>) {
    match Calls(model).call(CallKind::Decide, None, &decide_prompt(filter, f, attachments, model.prompt_chars())) {
        Ok(answer) => (read_yes_no(&answer), answer.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)),
        Err(e) => (None, Some(format!("the model did not answer: {}", words(&e)))),
    }
}
