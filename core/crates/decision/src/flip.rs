//! Use case: a fact flipped on a message (the person replied, read it, starred it...). Owner, A92 (2026-11-09 17:25): "When the fact
//! changes (the person replies), the label leaves the message by itself (a re-run on the fact, not on the model)". A140 (2026-11-11):
//! "A fact that flips (the mail is read) is re-read at once for steps 1 and 3 and the mail leaves or joins; the AI is asked again only
//! for new mail, on Re-run, or the first time a mail passes step 1."
//!
//! So, at once and with no engine call:
//! 1. the AI-off labels, in sidebar order: one whose step 1 now matches takes the message (then its step 3), as on arrival;
//! 2. else, when the AI placed it (card AI, exit or fallback, its pick an AI-on label): that label's step 1 on the new facts; passed =
//!    its step 3 on the new facts (kept = it goes (back) there, rejected = the fallback); missed = it LEAVES (Primary, no label);
//! 3. else a filing by step 1 of an AI-off label (or by its step 3's fallback) that step 1 no longer matches: it LEAVES;
//! 4. else, a message in Primary that an AI-on label's step-1 chain now picks: `AskAi` names those labels. The flip never asks the
//!    model: the caller (the filing worker, never a view) asks it once, the first time the message passes step 1 of that label.
//!
//! A filing by the owner's hand never moves. It holds no store handle: the caller unfiles, files (`label_file_flow`) and writes or
//! clears the rejection row from the action.
use crate::decide::DecideMessage;
use crate::entry::{LabelNode, TabOption};
use crate::flow::{after_ai, ai_picked_by_chain, ai_step_on, step_one, FlowPlace, FlowVerdict};
use crate::outcome::Outcome;
use crate::settings::DecisionSettings;
use crate::verdict::Source;
use contracts::{DecidedBy, DecisionProviderKind, WhyCard};
use smart_inbox::MessageFacts;

/// Where the message sits today and what placed it, as the store holds it (`label_assignment.flow_card`, `flow_rejection`).
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentFiling {
    pub place: FlowPlace,
    /// The card that placed it; `Hand` = the owner, or the default (no card).
    pub card: WhyCard,
    /// The chain that placed it (entry and exit cards only).
    pub chain_id: Option<u32>,
    /// The label the AI chose (cards AI, exit and fallback); for a fallback of an AI-off label, the label whose step 3 sent it on.
    pub ai_pick: Option<String>,
    /// Who decided the filing today (the owner, a rule, Laya or Jev, or the default).
    pub decided_by: DecidedBy,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlipAction {
    /// It stays where it is.
    Keep,
    /// It moves: `to` is where and by which card and chain; `decided_by` is who decides the new filing. A140: a message that leaves
    /// its label because step 1 no longer picks it moves to Primary with card `Hand` and `DecidedBy::Default` (no label, no row).
    Move { to: FlowVerdict, decided_by: DecidedBy },
    /// A140: it stays where it is (Primary), and these AI-on labels' step-1 chains now pick it: the model may rule on it, once, off
    /// the flip path (the filing worker decides whether it is the first time).
    AskAi { labels: Vec<String> },
}

pub struct RefileOnFact<'a> {
    /// The decision settings: the Labels page's rule entries are the AI-off labels' entry chains (only the Labels page is walked,
    /// engine off).
    pub settings: &'a DecisionSettings,
    pub tabs: &'a [TabOption],
    /// Every label, in sidebar order, with its chains (`LabelNode::with_chains`).
    pub labels: &'a [LabelNode],
}

/// Nothing to write: the same place, by the same card and chain.
fn same(a: &FlowVerdict, now: &CurrentFiling) -> bool {
    a.place == now.place && a.card == now.card && a.chain_id == now.chain_id
}

/// A140: the message leaves the label step 1 no longer picks it for: Primary, no label, no rejection row.
fn leave() -> FlowVerdict {
    FlowVerdict { card: WhyCard::Hand, chain_id: None, ai_pick: None, place: FlowPlace::Primary, entry_chain_id: None }
}

fn move_or_keep(to: FlowVerdict, decided_by: DecidedBy, now: &CurrentFiling) -> FlipAction {
    if same(&to, now) {
        FlipAction::Keep
    } else {
        FlipAction::Move { to, decided_by }
    }
}

impl RefileOnFact<'_> {
    pub fn execute(&self, f: &MessageFacts, now: &CurrentFiling) -> FlipAction {
        if matches!(now.decided_by, DecidedBy::Owner) {
            return FlipAction::Keep;
        }
        let visible: Vec<LabelNode> = self.labels.iter().filter(|l| l.visible_to(&f.account_id)).cloned().collect();
        // the page holds the AI step at all (a trust level and an AI-on label): else every label is a fixed-rules label
        let pipeline = self.settings.page(Outcome::Labels).is_some_and(|p| p.list.has_model_entry());

        // 1. the AI-off labels' step 1 (the page's rule entries), with no engine: only a sure rule match decides here; step 3 follows
        let mut rules_only = self.settings.clone();
        rules_only.engine = DecisionProviderKind::Rules;
        for p in &mut rules_only.pages {
            p.enabled = p.list.outcome == Outcome::Labels;
        }
        let d = DecideMessage { settings: &rules_only, engine: None, tabs: self.tabs, labels: self.labels }.execute(f);
        if let Some(v) = d.get(Outcome::Labels).filter(|v| v.decided_by.source == Source::Rule) {
            if let Some(fl) = v.flow.clone() {
                return move_or_keep(fl, v.decided_by.to_contract(), now);
            }
        }

        // 2. the label the AI chose: its step 1 and its step 3, on the new facts
        let ai_placed = matches!(now.card, WhyCard::Ai | WhyCard::Exit | WhyCard::Fallback);
        let pick = now.ai_pick.as_ref().filter(|_| ai_placed).and_then(|p| visible.iter().find(|l| &l.id == p));
        if let Some(node) = pick.filter(|n| !pipeline || ai_step_on(n)) {
            if pipeline && !step_one(node, f).passes() {
                return move_or_keep(leave(), DecidedBy::Default, now);
            }
            return move_or_keep(after_ai(node, f), now.decided_by.clone(), now);
        }

        // 3. placed by step 1 of an AI-off label (or by its step 3's fallback) and no AI-off label's step 1 matches now: it leaves
        if pipeline && (now.card == WhyCard::Entry || (now.card == WhyCard::Fallback && pick.map_or(true, |n| !ai_step_on(n)))) {
            return move_or_keep(leave(), DecidedBy::Default, now);
        }

        // 4. in Primary with no flow filing: the AI-on labels whose step-1 chain picks it now (the caller asks the model once)
        if pipeline && now.place == FlowPlace::Primary && now.card == WhyCard::Hand {
            let labels = ai_picked_by_chain(&visible, f);
            if !labels.is_empty() {
                return FlipAction::AskAi { labels };
            }
        }
        FlipAction::Keep
    }
}
