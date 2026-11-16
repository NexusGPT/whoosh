//! A140 (owner 2026-11-11: "If you have the three step. Step 1 does a pre-filter and select some emails (e.g., all unread), then step
//! 2 if selected gets AI ruling on each selected emails, then step 3 if selected gets fixed ruling on all AI selected emails"; card
//! "Yes, build it like this"): a label is a PIPELINE of three steps, each skipped when switched off. This replaces the A94 order, in
//! which a matching entry chain filed the message straight in and the model never read it.
//! 1. ENTRY rules (step 1) PICK mails: the label's entry chains are alternatives (AND or OR inside one, by its mode). No entry chain,
//!    or the entry card off = every mail passes step 1 (`step_one`).
//! 2. AI sorting (step 2) is asked ONLY about the mails that passed step 1 of that label: per message, the model is offered only the
//!    AI-on labels whose step 1 the message passed (`step_one_view`), and is not asked at all when there is none (`decide_pipeline`).
//!    The AI step is on when the AI card is on AND the label carries a description (what the model reads); else the label is AI-off.
//! 3. EXIT filter (step 3) applies to the mails the AI accepted, or to the mails step 1 picked when the AI is off: the exit chains are
//!    alternatives; kept = it stays; rejected = it goes to the label's fallback (another label, Primary or Junk), one hop.
//! Between labels: the AI-off labels whose step 1 MATCHED a chain win first, in sidebar order (the Labels page's rule entries,
//! `OutcomeList::labels_from_records`, now hold only those labels' entry chains); else the AI chooses among the AI-on labels whose step
//! 1 the message passed. With the AI off, step 1 alone files the mail (then step 3); a label with no entry chain and the AI off files
//! nothing. Every filing names its card and its chain (`FlowVerdict`): "why here" and the four figures read it.
//!
//! This file holds steps 1 and 3, the per-message view step 2 is asked on, and the pipeline call every caller uses
//! (`decide_pipeline`); `flip` re-reads 1 and 3 when a fact flips, without the model.
use crate::decide::{DecideMessage, Decisions};
use crate::entry::{label_paths, EntryKind, LabelNode, NodeChain, Value};
use crate::labels::rule_facts;
use crate::outcome::Outcome;
use crate::verdict::{Source, Verdict};
use contracts::{
    default_ai_reads, exit_verdict, inbox_view_id, mailbox_key, matched_conditions, rule_set_matches, why_here, AiReads, ChainPlacement, DecidedBy, ExitVerdict,
    FlowFiling, FlowRejection, InboxView, LabelChain, Mailbox, MessageFacts as RuleFacts, WhyCard, WhyHere,
};
use smart_inbox::MessageFacts;
use std::collections::HashMap;

/// Where a message ends after the flow: under a label, in Primary (no label), or in the Junk mailbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowPlace {
    Label(String),
    Primary,
    Junk,
}

impl FlowPlace {
    /// The id the store and the state paths use: the label id, "primary" (the view) or "junk" (the mailbox key).
    pub fn key(&self) -> String {
        match self {
            FlowPlace::Label(id) => id.clone(),
            FlowPlace::Primary => inbox_view_id(InboxView::Primary),
            FlowPlace::Junk => mailbox_key(Mailbox::Junk),
        }
    }

    /// The page's value for this place: a label, or no label (Primary and Junk carry none; the Junk move is the caller's).
    pub fn value(&self) -> Value {
        match self {
            FlowPlace::Label(id) => Value::Label(Some(id.clone())),
            FlowPlace::Primary | FlowPlace::Junk => Value::Label(None),
        }
    }
}

/// Reads a stored fallback: "primary", "junk" or a label id; None when there is none (or it is blank).
pub fn fallback_place(fallback: Option<&str>) -> Option<FlowPlace> {
    let f = fallback?.trim();
    if f.is_empty() {
        return None;
    }
    Some(if f == inbox_view_id(InboxView::Primary) {
        FlowPlace::Primary
    } else if f == mailbox_key(Mailbox::Junk) {
        FlowPlace::Junk
    } else {
        FlowPlace::Label(f.to_string())
    })
}

/// What the flow decided for one message, beside the page's `decided_by`: the card that placed it, the chain (entry and exit only),
/// the label the AI chose whatever happened after (None for an entry filing), and where it ends.
///
/// A140: `entry_chain_id` is the step-1 chain that PICKED the message for the label that holds the decision (the AI's pick, or the
/// AI-off label whose step 1 matched); None when step 1 let every mail pass (no chain, or the entry card off). On an AI-off label whose
/// step 3 sent the message on, `ai_pick` names that label (card Fallback): it is the label whose step 3 rejected it, which is what the
/// `flow_rejection` row needs to re-read steps 1 and 3 on a flip; `decided_by` then still names the rule.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowVerdict {
    pub card: WhyCard,
    pub chain_id: Option<u32>,
    pub ai_pick: Option<String>,
    pub place: FlowPlace,
    pub entry_chain_id: Option<u32>,
}

impl FlowVerdict {
    /// The row the four figures count (`contracts::count_label_flow`).
    pub fn filing(&self, message_id: &str) -> FlowFiling {
        let ended_in = match &self.place {
            FlowPlace::Label(id) => Some(id.clone()),
            _ => None,
        };
        FlowFiling { message_id: message_id.to_string(), ended_in, card: self.card, ai_pick: self.ai_pick.clone() }
    }

    /// The `flow_rejection` row to write when the exit filter sent the AI's pick elsewhere; None otherwise (the caller then clears
    /// any older row of the message: "a pass removes any earlier rejection row").
    pub fn rejection(&self, message_id: &str, now_ms: i64) -> Option<FlowRejection> {
        match (self.card, &self.ai_pick) {
            (WhyCard::Fallback, Some(ai)) => Some(FlowRejection { message_id: message_id.to_string(), ai_label_id: ai.clone(), sent_to: self.place.key(), chain_id: None, decided_at_ms: now_ms }),
            _ => None,
        }
    }

    /// The chain that placed it, from the label's nodes (entry or exit card of the label it sits in).
    pub fn chain<'a>(&self, labels: &'a [LabelNode]) -> Option<&'a LabelChain> {
        let placement = match self.card {
            WhyCard::Entry => ChainPlacement::Entry,
            WhyCard::Exit => ChainPlacement::Exit,
            _ => return None,
        };
        let FlowPlace::Label(id) = &self.place else { return None };
        let n = self.chain_id?;
        labels.iter().find(|l| &l.id == id)?.chains.iter().find(|c| c.placement == placement && c.chain.chain_id == n).map(|c| &c.chain)
    }

    /// "Why here?": the card, the chain and the conditions of that chain that hold for the message, and who decided. A140: when the
    /// AI placed it with no exit chain to name (card AI), the chain named is step 1's, the one that picked the message for the AI.
    pub fn why_here(&self, labels: &[LabelNode], f: &MessageFacts, decided_by: DecidedBy) -> WhyHere {
        if self.card == WhyCard::Ai {
            if let Some(c) = self.entry_chain(labels) {
                return WhyHere { card: self.card, chain_id: Some(c.chain_id), matched: matched_conditions(c.rules.clone(), rule_facts(f)), decided_by };
            }
        }
        why_here(self.card, self.chain(labels).cloned(), rule_facts(f), decided_by)
    }

    /// A140: the step-1 chain that picked the message, from the nodes (of the AI's pick, else of the label it sits in).
    pub fn entry_chain<'a>(&self, labels: &'a [LabelNode]) -> Option<&'a LabelChain> {
        let n = self.entry_chain_id?;
        let id = match (&self.ai_pick, &self.place) {
            (Some(pick), _) => pick,
            (None, FlowPlace::Label(id)) => id,
            _ => return None,
        };
        labels.iter().find(|l| &l.id == id)?.chains.iter().find(|c| c.placement == ChainPlacement::Entry && c.chain.chain_id == n).map(|c| &c.chain)
    }

    /// A140: the steps in words, for the log and the "why here" line: step 1's chain, the AI's answer (`decided_by_words`, the
    /// page's own "the model said …" words), step 3's outcome.
    pub fn steps_words(&self, decided_by_words: &str) -> String {
        let step1 = match self.entry_chain_id {
            Some(n) => format!("step 1 picked it by entry chain {n}"),
            None => "step 1 lets every mail pass".to_string(),
        };
        let step3 = match (self.card, &self.place) {
            (WhyCard::Exit, _) => match self.chain_id {
                Some(n) => format!("step 3 kept it by exit chain {n}"),
                None => "step 3 has no chain and keeps it".to_string(),
            },
            (WhyCard::Fallback, place) => format!("step 3 rejected it and sent it to {}", place.key()),
            (WhyCard::Entry, _) => "step 3 did not move it".to_string(),
            _ => "step 3 is off".to_string(),
        };
        format!("{step1} · {decided_by_words} · {step3}")
    }
}

/// A140: whether the label's step 2 (the AI) is on: its AI card is on AND it carries a description (the model judges the label from
/// it; a label with no description has nothing to be judged by, so it is a fixed-rules label).
pub fn ai_step_on(label: &LabelNode) -> bool {
    label.ai_enabled && !label.description.trim().is_empty()
}

/// A140 step 1 of one label on one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOne {
    /// No entry chain, or the entry card off: every mail passes.
    Open,
    /// This entry chain (by number) picked the message: the first by chain number whose conditions hold.
    Picked(u32),
    /// The label has entry chains and none holds, or the label is another account's.
    Missed,
}

impl StepOne {
    pub fn passes(self) -> bool {
        !matches!(self, StepOne::Missed)
    }
    pub fn chain_id(self) -> Option<u32> {
        match self {
            StepOne::Picked(n) => Some(n),
            _ => None,
        }
    }
}

/// Step 1 of `label` on the rule facts of a message (read once per message by the caller: `rule_facts(f)`).
pub fn step_one_on(label: &LabelNode, account_id: &str, facts: &RuleFacts) -> StepOne {
    if !label.visible_to(account_id) {
        return StepOne::Missed;
    }
    // a chain with no condition matches nothing and is not a chain (the delta 9 rule; the Swift test engine reads it the same way)
    let entries: Vec<&NodeChain> = label.chains.iter().filter(|c| c.placement == ChainPlacement::Entry && !c.chain.rules.conditions.is_empty()).collect();
    if !label.entry_enabled || entries.is_empty() {
        return StepOne::Open;
    }
    entries.into_iter().find(|c| rule_set_matches(c.chain.rules.clone(), facts.clone())).map_or(StepOne::Missed, |c| StepOne::Picked(c.chain.chain_id))
}

/// Step 1 of `label` on `f`.
pub fn step_one(label: &LabelNode, f: &MessageFacts) -> StepOne {
    step_one_on(label, &f.account_id, &rule_facts(f))
}

/// A140 step 2's offer for one message: the labels as given, with the AI card left on only for the labels whose AI step is on AND
/// whose step 1 the message passed. The model is offered (`offered_paths`), reads (`offered_reads`) and is told the meaning of
/// (`labels::meaning_prompt`) only these; the paths are still built over every label (a parent's name is part of its child's path).
pub fn step_one_view(labels: &[LabelNode], f: &MessageFacts) -> Vec<LabelNode> {
    let facts = rule_facts(f);
    labels
        .iter()
        .map(|l| {
            let mut n = l.clone();
            n.ai_enabled = ai_step_on(l) && step_one_on(l, &f.account_id, &facts).passes();
            n
        })
        .collect()
}

/// A140: the AI-on labels whose step 1 the message passes (the labels step 2 is asked about), in label order.
pub fn ai_candidates(labels: &[LabelNode], f: &MessageFacts) -> Vec<String> {
    let facts = rule_facts(f);
    labels.iter().filter(|l| ai_step_on(l) && step_one_on(l, &f.account_id, &facts).passes()).map(|l| l.id.clone()).collect()
}

/// A140: the AI-on labels whose step 1 PICKED the message by a chain (not an open step 1), in label order: the labels a fact flip can
/// make a message pass for the first time.
pub fn ai_picked_by_chain(labels: &[LabelNode], f: &MessageFacts) -> Vec<String> {
    let facts = rule_facts(f);
    labels.iter().filter(|l| ai_step_on(l) && matches!(step_one_on(l, &f.account_id, &facts), StepOne::Picked(_))).map(|l| l.id.clone()).collect()
}

/// A140, the one call every caller makes for the Labels page (arrival, Re-run, its preview, the Preview button): the page as built by
/// `labels_from_records` (its rule entries are the AI-off labels' entry chains: they win first) walked over `step_one_view`, so the model
/// is offered only the AI-on labels whose step 1 the message passed; when there is none, the model entry is dropped for this message
/// and the model is not asked at all. A page with no model entry (no trust level, or no AI-on label) is walked as it is.
pub fn decide_pipeline(d: &DecideMessage<'_>, f: &MessageFacts) -> Decisions {
    let asks = d.settings.page(Outcome::Labels).is_some_and(|p| p.enabled && p.list.has_model_entry());
    if !asks {
        return d.walk(f);
    }
    let view = step_one_view(d.labels, f);
    if view.iter().any(|l| l.ai_enabled && l.visible_to(&f.account_id)) {
        return DecideMessage { settings: d.settings, engine: d.engine, tabs: d.tabs, labels: &view }.walk(f);
    }
    let mut no_ai = d.settings.clone();
    if let Some(p) = no_ai.page_mut(Outcome::Labels) {
        p.list.entries.retain(|e| !matches!(e.kind, EntryKind::Model { .. }));
    }
    DecideMessage { settings: &no_ai, engine: d.engine, tabs: d.tabs, labels: &view }.walk(f)
}

/// Step 3 of a label on a message step 1 or the AI gave it. Exit card off = it stays (`stay`). On: kept by a chain (or no chain at
/// all) = it stays (card Exit for an AI pick; for an AI-off label the card stays Entry, named by step 1's chain); rejected = it goes to
/// the fallback (card Fallback, `ai_pick` = this label). A label whose filter is on with no fallback (a state the contract and the
/// store refuse) keeps it: there is nowhere to send it.
fn step_three(label: &LabelNode, f: &MessageFacts, stay: FlowVerdict, by_ai: bool) -> FlowVerdict {
    if !label.exit_enabled {
        return stay;
    }
    match exit_verdict(label.chains_of(ChainPlacement::Exit), rule_facts(f)) {
        ExitVerdict::Kept { chain_id } if by_ai => FlowVerdict { card: WhyCard::Exit, chain_id, ..stay },
        ExitVerdict::Kept { .. } => stay,
        ExitVerdict::Rejected => match fallback_place(label.exit_fallback.as_deref()) {
            Some(place) => FlowVerdict { card: WhyCard::Fallback, chain_id: None, ai_pick: Some(label.id.clone()), place, entry_chain_id: stay.entry_chain_id },
            None => stay,
        },
    }
}

/// Step 1 filed the message (an AI-off label: its entry chain `rule_id` matched), then step 3 (A140: the exit filter applies to what
/// step 1 picked when the AI is off).
pub fn entry_filing(label: &LabelNode, rule_id: &str, f: &MessageFacts) -> FlowVerdict {
    let chain_id = label.chains.iter().find(|c| c.rule_id == rule_id && c.placement == ChainPlacement::Entry).map(|c| c.chain.chain_id);
    let stay = FlowVerdict { card: WhyCard::Entry, chain_id, ai_pick: None, place: FlowPlace::Label(label.id.clone()), entry_chain_id: chain_id };
    step_three(label, f, stay, false)
}

/// Step 3 on the AI's pick `label`. Exit card off = it stays (card Ai). On: kept by a chain (or no chain at all) = it stays (card
/// Exit); rejected = it goes to the fallback (card Fallback). `entry_chain_id` records step 1's chain of the label (A140).
pub fn after_ai(label: &LabelNode, f: &MessageFacts) -> FlowVerdict {
    let entry_chain_id = step_one(label, f).chain_id();
    let stay = FlowVerdict { card: WhyCard::Ai, chain_id: None, ai_pick: Some(label.id.clone()), place: FlowPlace::Label(label.id.clone()), entry_chain_id };
    step_three(label, f, stay, true)
}

/// The flow record of a Labels verdict: step 1 of an AI-off label then step 3, the model's pick then step 3, or None (the default,
/// or a proposed new label, which is never filed). `labels` are the nodes visible to the message's account, with their chains.
pub fn of_verdict(v: &Verdict, labels: &[LabelNode], f: &MessageFacts) -> Option<FlowVerdict> {
    let Value::Label(Some(id)) = &v.value else { return None };
    let node = labels.iter().find(|l| &l.id == id)?;
    match &v.decided_by.source {
        Source::Rule => Some(entry_filing(node, v.decided_by.entry_id.strip_prefix("rule:").unwrap_or(&v.decided_by.entry_id), f)),
        Source::Model { .. } if v.proposal.is_none() => Some(after_ai(node, f)),
        _ => None,
    }
}

/// The labels the model may answer (step 2), path words -> id: the paths are built over every visible label (a parent's name is part
/// of its child's path, AI card or not), then only the labels whose AI card is on are kept.
pub fn offered_paths(labels: &[LabelNode]) -> HashMap<String, String> {
    label_paths(labels).into_iter().filter(|(_, id)| labels.iter().any(|l| &l.id == id && l.ai_enabled)).collect()
}

/// What the model reads for the Labels question: what any offered label ticks (one model call judges them all, so the inputs are the
/// union of theirs). No offered label = the default (the question is then not asked).
pub fn offered_reads(labels: &[LabelNode]) -> AiReads {
    let offered: Vec<&LabelNode> = labels.iter().filter(|l| l.ai_enabled).collect();
    if offered.is_empty() {
        return default_ai_reads();
    }
    AiReads {
        subject: offered.iter().any(|l| l.ai_reads.subject),
        body: offered.iter().any(|l| l.ai_reads.body),
        rest_of_thread: false, // the thread is its own flag (`offered_rest_of_thread`), not part of this union
        attachments: offered.iter().any(|l| l.ai_reads.attachments),
    }
}

/// Whether the Labels question carries the rest of the thread: any offered label (AI card on) ticks it (lane labels-r5). One model
/// call judges all the offered labels, so, as with the other inputs, one label's tick sends it for the call.
pub fn offered_rest_of_thread(labels: &[LabelNode]) -> bool {
    labels.iter().any(|l| l.ai_enabled && l.rest_of_thread)
}
