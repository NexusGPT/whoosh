//! The first-match walk. One `Placer` holds the "Which tab" list and the "Needs my reply" list; `place` walks the first
//! and returns the tab, who decided, and which entry (position and id). The "ask the model" entry is asked only when
//! it is reached; a missing, off or unsure model leaves the walk going.
use crate::ask::{AskKind, AskModel, ModelAnswer, ModelAsk};
use crate::facts::MessageFacts;
use crate::list::{DecisionList, DecisionPage, EntryKind, TabRef, Then};
use crate::tabs::TabSet;
use contracts::{DecidedBy, DecisionProviderKind, Rule};

#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub message_id: String,
    pub tab_id: String,
    pub decided_by: DecidedBy,
    pub probability: Option<f64>,
    pub page: DecisionPage,
    /// 1-based position of the entry that decided, in the list as it stood.
    pub entry_position: usize,
    pub entry_id: String,
}

impl Placement {
    /// The "Why?" sentence: "Which tab · entry 3 · the model said Updates (0.91)".
    pub fn why(&self, tab_name: &str) -> String {
        let head = format!("{} · entry {}", self.page.words(), self.entry_position);
        match (&self.decided_by, self.probability) {
            (DecidedBy::Rule { rule_id }, _) => format!("{head} · rule {rule_id} → {tab_name}"),
            (DecidedBy::Laya | DecidedBy::Jev | DecidedBy::Apple | DecidedBy::Model { .. }, Some(p)) => format!("{head} · the model said {tab_name} ({p:.2})"),
            (DecidedBy::Laya | DecidedBy::Jev | DecidedBy::Apple | DecidedBy::Model { .. }, None) => format!("{head} · the model said {tab_name}"),
            (DecidedBy::Owner, _) => format!("{head} · you moved it to {tab_name}"),
            (DecidedBy::Default, _) => format!("{head} · default → {tab_name}"),
        }
    }
}

/// The id a rule placement is stored under: the entry, plus the exact label node when a label rule decided ("tab:t1@l-orbit").
pub fn rule_id_with_node(entry_id: &str, when: &[crate::list::Cond], f: &MessageFacts) -> String {
    match when.iter().find_map(|c| c.label_node(f)) {
        Some(node) => format!("{entry_id}@{node}"),
        None => entry_id.to_string(),
    }
}

pub fn decided_by_provider(kind: DecisionProviderKind) -> DecidedBy {
    match kind {
        DecisionProviderKind::Local => DecidedBy::Laya,
        DecisionProviderKind::Jev => DecidedBy::Jev,
        DecisionProviderKind::Rules => DecidedBy::Default,
        DecisionProviderKind::Apple => DecidedBy::Apple,
    }
}

struct Resolved {
    yes: bool,
    by: Option<(DecisionProviderKind, f64)>,
}

pub struct Placer<'a> {
    pub tabs: &'a TabSet,
    pub which: DecisionList,
    pub needs: DecisionList,
    pub model: &'a dyn AskModel,
}

impl<'a> Placer<'a> {
    /// The shipped lists with the owner's layer (their rules, the tabs they added) in front of the "Which tab" list.
    pub fn new(tabs: &'a TabSet, rules: &[Rule], model: &'a dyn AskModel) -> Placer<'a> {
        let mut which = DecisionList::shipped_which_tab();
        which.insert_first(tabs.custom_entries(rules));
        Placer { tabs, which, needs: DecisionList::shipped_needs_reply(), model }
    }

    fn needs_reply(&self, f: &MessageFacts) -> Resolved {
        for e in &self.needs.entries {
            match &e.kind {
                EntryKind::Rule { when, then: Then::Answer(a) } => {
                    if when.iter().all(|c| c.pure(f) == Some(true)) {
                        return Resolved { yes: *a, by: None };
                    }
                }
                EntryKind::Model { question, trust_from } => {
                    let ask = ModelAsk { page: DecisionPage::NeedsMyReply, question, kind: AskKind::YesNo, facts: f };
                    if let Some(v) = self.model.execute(&ask) {
                        if let ModelAnswer::Yes(y) = v.answer {
                            if v.probability >= *trust_from {
                                return Resolved { yes: y, by: Some((v.by, v.probability)) };
                            }
                        }
                    }
                }
                EntryKind::Default { then: Then::Answer(a) } => return Resolved { yes: *a, by: None },
                _ => {}
            }
        }
        Resolved { yes: false, by: None }
    }

    fn at(&self, f: &MessageFacts, position: usize, id: &str, r: &TabRef, by: DecidedBy, p: Option<f64>) -> Placement {
        Placement { message_id: f.message_id.clone(), tab_id: self.tabs.resolve(r), decided_by: by, probability: p, page: DecisionPage::WhichTab, entry_position: position, entry_id: id.to_string() }
    }

    pub fn place(&self, f: &MessageFacts) -> Placement {
        let mut needs: Option<Resolved> = None;
        for (i, e) in self.which.entries.iter().enumerate() {
            let pos = i + 1;
            match &e.kind {
                EntryKind::Rule { when, then: Then::Tab(r) } => {
                    let mut ok = true;
                    let mut used_needs = false;
                    for c in when {
                        let holds = match c.pure(f) {
                            Some(b) => b,
                            None => {
                                used_needs = true;
                                needs.get_or_insert_with(|| self.needs_reply(f)).yes
                            }
                        };
                        if !holds {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        // When the model said "yes, this needs a reply", the model placed it, and the row says so.
                        let model_said = if used_needs { needs.as_ref().and_then(|n| n.by) } else { None };
                        return match model_said {
                            Some((kind, p)) => self.at(f, pos, &e.id, r, decided_by_provider(kind), Some(p)),
                            None => self.at(f, pos, &e.id, r, DecidedBy::Rule { rule_id: rule_id_with_node(&e.id, when, f) }, None),
                        };
                    }
                }
                EntryKind::Model { question, trust_from } => {
                    let options = self.tabs.model_options();
                    if options.is_empty() {
                        continue;
                    }
                    let ask = ModelAsk { page: DecisionPage::WhichTab, question, kind: AskKind::Choice { options }, facts: f };
                    if let Some(v) = self.model.execute(&ask) {
                        if let ModelAnswer::Choice(c) = &v.answer {
                            if v.probability >= *trust_from {
                                if let Some(r) = self.tabs.tab_for_choice(c) {
                                    return self.at(f, pos, &e.id, &r, decided_by_provider(v.by), Some(v.probability));
                                }
                            }
                        }
                    }
                }
                EntryKind::Default { then: Then::Tab(r) } => return self.at(f, pos, &e.id, r, DecidedBy::Default, None),
                _ => {}
            }
        }
        // A list with no default entry still places the message: in Primary, as a default.
        let primary = TabRef::Id(self.tabs.primary());
        self.at(f, self.which.entries.len() + 1, "default", &primary, DecidedBy::Default, None)
    }
}
