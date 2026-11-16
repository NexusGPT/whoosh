//! What a page settles for one message, and which entry settled it. `decided_by` is a required field: a verdict without
//! the page and the entry cannot be built.
use crate::entry::Value;
use crate::flow::FlowVerdict;
use crate::outcome::Outcome;
use contracts::{DecidedBy, DecisionProviderKind};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Rule,
    /// The model entry answered at or above its trust level.
    Model { provider: DecisionProviderKind, answer: String, probability: f64 },
    Default,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecidedByEntry {
    pub page: Outcome,
    /// The entry's id ("which-tab/model", "rule:<id>").
    pub entry_id: String,
    /// 1-based position in the page's list, as the owner counts it.
    pub position: usize,
    pub source: Source,
}

/// A NEW child label the model proposed under an existing parent. Returned for the owner to accept; never written.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelProposal {
    pub parent_id: String,
    pub name: String,
    /// The whole path as the model said it ("client > Provanta > Orbit").
    pub path_words: String,
    pub probability: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub message_id: String,
    pub outcome: Outcome,
    pub value: Value,
    pub decided_by: DecidedByEntry,
    pub proposal: Option<LabelProposal>,
    /// Delta 17, Labels page only: the card that placed the message (entry rule, AI, exit filter, fallback), its chain, the AI's
    /// pick and where it ends. None on the other pages, for the default, and for a proposed new label. When the exit filter sent
    /// the pick on, `value` is already the fallback (a label, or no label for Primary and Junk) and `decided_by` still names the
    /// model entry that chose: the AI decided, the exit card moved it.
    pub flow: Option<FlowVerdict>,
}

impl DecidedByEntry {
    /// "Why?" on a row: "Which tab · entry 11 · the model said Updates (0.91)".
    pub fn why(&self) -> String {
        let head = format!("{} · entry {}", self.page.words(), self.position);
        match &self.source {
            Source::Rule => format!("{head} · rule {}", self.entry_id),
            Source::Model { answer, probability, .. } => format!("{head} · the model said {answer} ({probability:.2})"),
            Source::Default => format!("{head} · otherwise"),
        }
    }

    /// The contract's `decided_by` for message_category. The entry id rides in `rule_id` for a rule; the model is named
    /// by its provider (the contract has no field for the model entry's id: see "Contract deltas").
    pub fn to_contract(&self) -> DecidedBy {
        match &self.source {
            Source::Rule => DecidedBy::Rule { rule_id: self.entry_id.clone() },
            Source::Model { provider: DecisionProviderKind::Local, .. } => DecidedBy::Laya,
            Source::Model { provider: DecisionProviderKind::Jev, .. } => DecidedBy::Jev,
            Source::Model { provider: DecisionProviderKind::Apple, .. } => DecidedBy::Apple,
            Source::Model { provider: DecisionProviderKind::Rules, .. } | Source::Default => DecidedBy::Default,
        }
    }

    /// The store's `label_assignment.decided_entry_id` (contracts delta 12): the model entry that decided ("labels/model")
    /// when the model decided; None for a rule or the default, as `Store::label_file_entry` keeps it.
    pub fn decided_entry_id(&self) -> Option<String> {
        match &self.source {
            Source::Model { .. } => Some(self.entry_id.clone()),
            Source::Rule | Source::Default => None,
        }
    }

    pub fn probability(&self) -> Option<f64> {
        match &self.source {
            Source::Model { probability, .. } => Some(*probability),
            _ => None,
        }
    }
}
