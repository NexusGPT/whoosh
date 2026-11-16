//! Use case: a message the sync stored runs the labelling order. A140 (owner 2026-11-11), the three-step pipeline the decision crate
//! holds (`decision::flow::decide_pipeline`): the AI-off labels whose step 1 (entry rules) matched win first, in label order, then
//! their step 3; else the model rules over the AI-on labels whose step 1 the message passed (it is not asked when there is none),
//! reading their ticked inputs; then the chosen label's step 3 (exit filter) with its fallback. This file reads what the order needs
//! from the store, hands each message to it, and writes what it decided: the label with its card, chain and who decided, the move to
//! Junk, the `flow_rejection` row. With no provider set the AI-off labels and the exit filters still run; the model step is then
//! absent and the record says so (`AiStep::NoProvider`). It also serves the one later time the model is asked outside a Re-run: a
//! flipped fact makes an AI-on label's step 1 pick a message for the first time (`ask_on_flip`, on the filing worker only).
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use contracts::{DecidedBy, DecisionPort, WhyCard};
use decision::flow::{ai_candidates, ai_picked_by_chain};
use decision::{Decisions, FlipAction, FlowPlace, Outcome, RefileOnFact, Source, Verdict};
use smart_inbox::MessageFacts;
use store::{Store, INBOX_FOLDER};

use crate::flow_ctx::FlowCtx;
use crate::flow_write::{apply, filing_of, junk_folder, Write};
use crate::imap_source::Roles;

/// What happened to the model step of one message.
#[derive(Debug, Clone, PartialEq)]
pub enum AiStep {
    /// An entry rule settled it first: the model was never asked.
    EntryRule,
    /// The walk reached the model and no provider is set: only the entry rules and the exit filters ran.
    NoProvider,
    /// No label offers itself to the model (no AI card is on, or none has a description), or (A140) no AI-on label's step 1 passes
    /// the message: the model is not asked.
    NoLabelOffered,
    /// The model answered (or declined every label: the message stays in Primary).
    Asked,
    /// The provider refused or failed, in its words, or (lane fix-aisort) answered in a way that cannot be read; the message stays in
    /// Primary, no "no" is kept for it (it stays waiting) and it is asked again (`label_ai_failure`).
    Refused(String),
    /// Lane fix-aisort round 11: the judge refused this MAIL itself (its content, language or size, or no label of the list: asking again
    /// gives the same answer), with the reason id (`decision::decide::mail_refusal`). The mail is SET ASIDE: a kept row per AI-on label
    /// whose step 1 it passed (`Store::ai_set_aside_put`), so it neither waits nor is asked again, and it files nowhere by the AI.
    SetAside(String),
}

/// One message's outcome, for the log and the tests: where it ended, which card placed it, and who decided, in words.
#[derive(Debug, Clone, PartialEq)]
pub struct Filed {
    pub message_id: String,
    /// A label id, "primary" or "junk".
    pub place: String,
    pub card: Option<WhyCard>,
    pub chain_id: Option<u32>,
    pub decided_by: String,
    pub ai: AiStep,
    /// A140: the AI-on labels the model ruled on for this message (those whose step 1 it passed), when it was asked and answered.
    pub asked: Vec<String>,
}

/// A140: what the model ruled per message and label in this process, kept by the filing worker: true = it accepted the label. Part 3: a
/// cache over the store, whose `ai_label_no` keeps every "no" across restarts (`keep_the_nos`, read back by `ask_on_flip`); the book adds
/// only the "yes" of a message that has since left the label, so it goes back without a question. After a restart that "yes" is gone and
/// such a message is asked once more. Bounded by `AI_BOOK_MAX` messages.
pub(crate) type AiBook = Arc<Mutex<HashMap<String, HashMap<String, bool>>>>;
/// The most messages `AiBook` remembers (a number of this lane, proposed for docs/CONSTANTS.md as LABEL_AI_BOOK_MAX): past it the book
/// starts again empty, which costs at most one more question per message.
pub(crate) const AI_BOOK_MAX: usize = 20_000;

fn remember(book: &AiBook, filed: &[Filed], picks: &HashMap<String, Option<String>>) {
    let mut b = book.lock().unwrap_or_else(|e| e.into_inner());
    for f in filed.iter().filter(|f| f.ai == AiStep::Asked && !f.asked.is_empty()) {
        if b.len() >= AI_BOOK_MAX && !b.contains_key(&f.message_id) {
            b.clear();
        }
        let pick = picks.get(&f.message_id).cloned().flatten();
        let row = b.entry(f.message_id.clone()).or_default();
        for label in &f.asked {
            row.insert(label.clone(), pick.as_deref() == Some(label.as_str()));
        }
    }
}

fn ai_step(ctx: &FlowCtx, f: &MessageFacts, d: &Decisions, v: &Verdict) -> AiStep {
    if v.decided_by.source == Source::Rule {
        return AiStep::EntryRule;
    }
    if !ctx.asks_model || ai_candidates(&ctx.nodes, f).is_empty() {
        return AiStep::NoLabelOffered;
    }
    if ctx.port.is_none() {
        return AiStep::NoProvider;
    }
    match &d.engine_error {
        Some(e) => match decision::decide::mail_refusal(e) {
            Some(reason) => AiStep::SetAside(reason),
            None => AiStep::Refused(e.to_string()),
        },
        None => AiStep::Asked,
    }
}

fn words(v: &Verdict, ai: &AiStep) -> String {
    // A140: the words name step 1's chain, the AI's answer (or the rule) and step 3's outcome
    let base = match &v.flow {
        Some(fl) => fl.steps_words(&v.decided_by.why()),
        None => v.decided_by.why(),
    };
    match ai {
        AiStep::NoProvider => format!("{base} · no AI provider is set, only the entry rules and the exit filters ran"),
        AiStep::Refused(why) => format!("{base} · the AI step was refused: {why}"),
        AiStep::SetAside(why) => format!("{base} · the AI set the mail aside: {why}"),
        _ => base,
    }
}

fn place_of(v: &Verdict) -> (String, Option<WhyCard>, Option<u32>) {
    match &v.flow {
        Some(fl) => (fl.place.key(), Some(fl.card), fl.chain_id),
        None => (FlowPlace::Primary.key(), None, None),
    }
}

/// The record and the write of one message's verdict. A message the flow leaves in Primary with no rejection writes nothing.
fn placement(store: &Store, roles: &Roles, ctx: &FlowCtx, f: &MessageFacts, d: &Decisions, v: &Verdict, now_ms: i64) -> (Option<Write>, Filed) {
    let ai = ai_step(ctx, f, d, v);
    let (place, card, chain_id) = place_of(v);
    let asked = if ai == AiStep::Asked { ai_candidates(&ctx.nodes, f) } else { vec![] };
    let filed = Filed { message_id: f.message_id.clone(), place, card, chain_id, decided_by: words(v, &ai), ai, asked };
    let Some(fl) = &v.flow else { return (None, filed) };
    let write = Write {
        message_id: f.message_id.clone(),
        unassign: None,
        back_to_inbox: false,
        file: filing_of(v, fl),
        to_junk: matches!(fl.place, FlowPlace::Junk).then(|| junk_folder(store, roles, &f.account_id)).flatten(),
        rejection: fl.rejection(&f.message_id, now_ms),
    };
    (Some(write).filter(|w| !w.is_noop_for_arrival()), filed)
}

/// Runs the order on the INBOX messages among `ids` (any other folder is never filed by the flow) and writes the result in one
/// transaction. A message that is not stored is skipped.
pub(crate) fn run(store: &Store, port: Option<Arc<dyn DecisionPort>>, roles: &Roles, ids: &[String], now_ms: i64) -> Result<Vec<Filed>, String> {
    run_with_book(store, port, roles, ids, now_ms, None)
}

/// `run`, and what the model ruled is written in `book` (the filing worker's, A140).
pub(crate) fn run_with_book(store: &Store, port: Option<Arc<dyn DecisionPort>>, roles: &Roles, ids: &[String], now_ms: i64, book: Option<&AiBook>) -> Result<Vec<Filed>, String> {
    let inbox: Vec<String> = ids
        .iter()
        .filter(|id| store.message(id).ok().flatten().is_some_and(|m| m.folder == INBOX_FOLDER && !m.is_outgoing))
        .cloned()
        .collect();
    if inbox.is_empty() {
        return Ok(vec![]);
    }
    let ctx = FlowCtx::open(store, port)?;
    let (mut writes, mut out) = (Vec::new(), Vec::new());
    let mut picks: HashMap<String, Option<String>> = HashMap::new();
    // lane fix-aisort: what the run tells core.log and `label/<id>/flow/ai_refusal` (label_ai_failure.rs)
    let (mut candidates, mut below): (HashMap<String, Vec<String>>, HashMap<String, Vec<f64>>) = (HashMap::new(), HashMap::new());
    let mut scores: Vec<(String, f64)> = Vec::new();
    for f in &ctx.facts(&inbox)? {
        let d = ctx.decide(f);
        let Some(v) = d.get(Outcome::Labels) else { continue };
        let (write, filed) = placement(store, roles, &ctx, f, &d, v, now_ms);
        if matches!(filed.ai, AiStep::Asked | AiStep::Refused(_) | AiStep::NoProvider | AiStep::SetAside(_)) {
            candidates.insert(f.message_id.clone(), ai_candidates(&ctx.nodes, f));
            below.insert(f.message_id.clone(), d.below_trust.iter().map(|(_, p)| *p).collect());
            if filed.ai == AiStep::Asked {
                scores.extend(d.label_scores.iter().cloned());
            }
        }
        writes.extend(write);
        picks.insert(f.message_id.clone(), v.flow.as_ref().and_then(|fl| fl.ai_pick.clone()));
        out.push(filed);
        // a provider answers in its own time: each message is filed as soon as it is decided, not after the whole page
        if ctx.port.is_some() {
            apply(store, std::mem::take(&mut writes), now_ms)?;
        }
    }
    apply(store, writes, now_ms)?;
    keep_the_nos(store, &ctx, &out, &picks, now_ms)?;
    // round 11: the mails the judge refused for themselves are set aside under every AI-on label whose step 1 they passed (Apple's code)
    let aside: Vec<(String, String)> = out
        .iter()
        .filter(|f| matches!(f.ai, AiStep::SetAside(_)))
        .flat_map(|f| candidates.get(&f.message_id).cloned().unwrap_or_default().into_iter().map(move |l| (f.message_id.clone(), l)))
        .collect();
    store.ai_set_aside_put(aside, DecidedBy::Apple, now_ms).map_err(|e| format!("the mails set aside could not be kept: {e}"))?;
    // round 17: a mail the AI could not judge (a failure that can pass, an unreadable answer, no model yet) waits for every AI-on label with
    // no step 1 it was offered to (nothing else says so there: `label_flow::waiting_candidates`); a judged mail no longer waits for any
    let open = crate::flow_ctx::labels_without_step_one(&ctx.nodes);
    let waits: Vec<(String, String)> = out
        .iter()
        .filter(|f| matches!(f.ai, AiStep::Refused(_) | AiStep::NoProvider))
        .flat_map(|f| candidates.get(&f.message_id).cloned().unwrap_or_default().into_iter().filter(|l| open.contains(l)).map(move |l| (f.message_id.clone(), l)))
        .collect();
    store.ai_waiting_put(waits, now_ms).map_err(|e| format!("the mails waiting for the AI could not be marked: {e}"))?;
    let judged: Vec<(String, String)> = out.iter().filter(|f| f.ai == AiStep::Asked).flat_map(|f| f.asked.iter().map(move |l| (f.message_id.clone(), l.clone()))).collect();
    store.ai_waiting_clear(judged).map_err(|e| format!("the mails the AI judged could not be unmarked: {e}"))?;
    let accepted = out.iter().filter(|f| f.ai == AiStep::Asked && picks.get(&f.message_id).cloned().flatten().is_some_and(|p| f.asked.contains(&p))).count();
    let mut accepted_by_label: HashMap<String, usize> = HashMap::new();
    for f in out.iter().filter(|f| f.ai == AiStep::Asked) {
        if let Some(p) = picks.get(&f.message_id).cloned().flatten().filter(|p| f.asked.contains(p)) {
            *accepted_by_label.entry(p).or_default() += 1;
        }
    }
    crate::label_ai_failure::note_run(store, &out, &candidates, &below, accepted, decision::tuning::model_trust_label());
    crate::label_ai_failure::note_spread(store, &scores, &accepted_by_label, decision::tuning::model_trust_label());
    if let Some(book) = book {
        remember(book, &out, &picks);
    }
    Ok(out)
}

/// A140 part 3: the model's "no" is kept in the store (`ai_label_no`, migration 0022): for each message the model answered, every AI-on
/// label whose step 1 it passed and that the model did not accept. A "yes" needs no row (the filing records it). Named by the model as the
/// filing rows name it (Laya or Jev); with no model to name, nothing is written.
fn keep_the_nos(store: &Store, ctx: &FlowCtx, out: &[Filed], picks: &HashMap<String, Option<String>>, now_ms: i64) -> Result<(), String> {
    let by = crate::label_ai_failure::model_decider(store, ctx.port.as_ref());
    if !matches!(by, DecidedBy::Laya | DecidedBy::Jev | DecidedBy::Apple | DecidedBy::Model { .. }) {
        return Ok(());
    }
    let mut pairs = Vec::new();
    for f in out.iter().filter(|f| f.ai == AiStep::Asked) {
        let pick = picks.get(&f.message_id).cloned().flatten();
        for label in f.asked.iter().filter(|l| pick.as_deref() != Some(l.as_str())) {
            pairs.push((f.message_id.clone(), label.clone()));
        }
    }
    store.ai_no_put(pairs, by, now_ms).map_err(|e| format!("the AI's answer could not be kept: {e}"))
}

/// A140, after a flip (on the filing worker, never on a view's path): the messages that sit in Primary with no flow filing and that
/// an AI-on label's step-1 chain now picks (`FlipAction::AskAi`). For a label the model accepted before (`book`), the message goes
/// back under it through that label's step 3, without asking; when one of the labels is new to the message, the model is asked
/// ONCE for the message (as on arrival, over every AI-on label whose step 1 it passes) and the book records its ruling. A message
/// whose labels the model has all ruled on and refused stays where it is.
pub(crate) fn ask_on_flip(store: &Store, port: Option<Arc<dyn DecisionPort>>, roles: &Roles, ids: &[String], now_ms: i64, book: &AiBook) -> Result<Vec<Filed>, String> {
    let stored: Vec<String> = ids.iter().filter(|id| store.message(id).ok().flatten().is_some_and(|m| m.folder == INBOX_FOLDER && !m.is_outgoing)).cloned().collect();
    if stored.is_empty() {
        return Ok(vec![]);
    }
    let ctx = FlowCtx::open(store, port)?;
    let refile = RefileOnFact { settings: &ctx.settings, tabs: &[], labels: &ctx.nodes };
    let (mut ask, mut writes) = (Vec::new(), Vec::new());
    for f in &ctx.facts(&stored)? {
        let now = ctx.current(&f.message_id)?;
        let FlipAction::AskAi { labels } = refile.execute(f, &now) else { continue };
        let mut known: HashMap<String, bool> = book.lock().unwrap_or_else(|e| e.into_inner()).get(&f.message_id).cloned().unwrap_or_default();
        // A140 part 3: the store's kept "no" (after a restart too); the in-memory book is only a cache of the "yes" the model gave before
        for label in store.ai_no_labels(&f.message_id).map_err(|e| format!("the AI's kept answers could not be read: {e}"))? {
            known.entry(label).or_insert(false);
        }
        if labels.iter().any(|l| !known.contains_key(l)) {
            ask.push(f.message_id.clone());
            continue;
        }
        // every label was ruled on before: the first one the model accepted takes it back, through its step 3 (no question)
        let accepted: HashSet<&String> = known.iter().filter(|(_, yes)| **yes).map(|(l, _)| l).collect();
        let Some(node) = ai_picked_by_chain(&ctx.nodes, f).iter().find(|l| accepted.contains(l)).and_then(|l| ctx.nodes.iter().find(|n| &n.id == l)) else { continue };
        let to = decision::flow::after_ai(node, f);
        writes.push(Write {
            message_id: f.message_id.clone(),
            unassign: None,
            back_to_inbox: false,
            file: crate::flow_write::flip_filing(&to, contracts::DecidedBy::Default, ctx.port.as_ref()),
            to_junk: matches!(to.place, FlowPlace::Junk).then(|| junk_folder(store, roles, &f.account_id)).flatten(),
            rejection: to.rejection(&f.message_id, now_ms),
        });
    }
    apply(store, writes, now_ms)?;
    if ask.is_empty() {
        return Ok(vec![]);
    }
    run_with_book(store, ctx.port.clone(), roles, &ask, now_ms, Some(book))
}
