//! Settings › Decisions: one page per outcome (on or off, its ordered list) and the Engine block (Local, Jev or Off).
use crate::entry::OutcomeList;
use crate::outcome::Outcome;
use crate::entry::LabelNode;
use contracts::{DecisionModel, DecisionProviderKind, Rule};

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub enabled: bool,
    pub list: OutcomeList,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionSettings {
    /// `Rules` = Engine Off: every "ask the model" entry is skipped and its list falls through.
    pub engine: DecisionProviderKind,
    /// One page per outcome, in `Outcome::ALL` order.
    pub pages: Vec<Page>,
    /// Lane fix-aisort round 4: the engine answers a choice in free text, so it may name a label that is not offered (a proposed new child
    /// under an existing parent, `decide::read_answer`). Jev does (`adapters/wire.rs` reads the `choice` text as sent back). Laya never does:
    /// it answers the arg-max of the options it was given (LayaEngine.swift `labels[best]`). True by default for Jev only; a caller with
    /// another free-text engine (a local language model through `adapters::local`) sets it. Read only in per-label mode (`walk`).
    pub choice_is_open: bool,
}

impl DecisionSettings {
    /// The shipped lists, with exactly the pages named in `enabled` switched on.
    pub fn shipped(engine: DecisionProviderKind, enabled: &[Outcome]) -> DecisionSettings {
        let pages = Outcome::ALL.into_iter().map(|o| Page { enabled: enabled.contains(&o), list: OutcomeList::shipped(o) }).collect();
        DecisionSettings { engine, pages, choice_is_open: engine == DecisionProviderKind::Jev }
    }

    /// From the stored `DecisionModel` record: its provider and its `decisions_enabled` keys (an unknown key is ignored).
    pub fn from_record(record: &DecisionModel) -> DecisionSettings {
        let enabled: Vec<Outcome> = record.decisions_enabled.iter().filter_map(|k| Outcome::from_key(k)).collect();
        let mut s = DecisionSettings::shipped(record.provider, &enabled);
        s.apply_question_words(record);
        s
    }

    /// From the stored record AND the owner's labels (contract delta 9): the Labels page is built from the label records
    /// and their rules (`OutcomeList::labels_from_records`, fixed rules first, then the meaning at `meaning_trust`); the
    /// other pages are the shipped lists; the record's question words apply to every model entry, the Labels one included.
    pub fn from_record_with_labels(record: &DecisionModel, labels: &[LabelNode], rules: &[Rule], meaning_trust: Option<f64>) -> DecisionSettings {
        let mut s = DecisionSettings::from_record(record);
        if let Some(page) = s.page_mut(Outcome::Labels) {
            page.list = OutcomeList::labels_from_records(labels, rules, meaning_trust);
        }
        s.apply_question_words(record);
        s
    }

    /// A model entry's sentence is what is sent: an edited sentence replaces the shipped one.
    fn apply_question_words(&mut self, record: &DecisionModel) {
        for page in &mut self.pages {
            for e in &mut page.list.entries {
                if let crate::entry::EntryKind::Model { question, .. } = &mut e.kind {
                    if let Some(words) = record.question_words.get(&e.id) {
                        *question = words.clone();
                    }
                }
            }
        }
    }

    pub fn page(&self, o: Outcome) -> Option<&Page> {
        self.pages.iter().find(|p| p.list.outcome == o)
    }
    pub fn page_mut(&mut self, o: Outcome) -> Option<&mut Page> {
        self.pages.iter_mut().find(|p| p.list.outcome == o)
    }
}
