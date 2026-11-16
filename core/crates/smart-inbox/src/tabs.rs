//! Split tabs as data (contracts::SplitTab). Placement refers to a tab by its builtin key or its id, never by its
//! display name, so a rename changes no placement.
use crate::list::{Cond, Entry, EntryKind, TabRef, Then};
use contracts::{Rule, SplitTab, SplitTabRule};

#[derive(Debug, Clone, Default)]
pub struct TabSet {
    /// Every tab, by order.
    pub tabs: Vec<SplitTab>,
}

/// The four categories a model may answer with besides the owner's decision-category tabs.
const MODEL_CATEGORIES: &[&str] = &["Primary", "Updates", "Promotions", "Social"];

impl TabSet {
    pub fn new(mut tabs: Vec<SplitTab>) -> TabSet {
        tabs.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        TabSet { tabs }
    }

    pub fn get(&self, id: &str) -> Option<&SplitTab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.get(id).is_some_and(|t| t.enabled)
    }

    /// The tab uncategorised mail sits in: the first enabled tab by order (the store's definition).
    pub fn primary(&self) -> String {
        self.tabs.iter().find(|t| t.enabled).map(|t| t.id.clone()).unwrap_or_else(|| "primary".to_string())
    }

    pub fn builtin(&self, key: &str) -> Option<&SplitTab> {
        self.tabs.iter().find(|t| t.enabled && matches!(&t.rule, SplitTabRule::Builtin { name } if name.eq_ignore_ascii_case(key)))
    }

    /// The tab a reference names; a tab that is gone or off sends the mail to Primary.
    pub fn resolve(&self, r: &TabRef) -> String {
        match r {
            TabRef::Builtin(key) => self.builtin(key).map(|t| t.id.clone()).unwrap_or_else(|| self.primary()),
            TabRef::Id(id) => if self.is_enabled(id) { id.clone() } else { self.primary() },
        }
    }

    /// What the model may answer with: the default categories whose tab is on, and each decision-category tab's category.
    pub fn model_options(&self) -> Vec<String> {
        let mut out: Vec<String> = MODEL_CATEGORIES.iter().filter(|c| self.builtin(c).is_some()).map(|c| c.to_string()).collect();
        for t in self.tabs.iter().filter(|t| t.enabled) {
            if let SplitTabRule::DecisionCategory { category } = &t.rule {
                out.push(category.clone());
            }
        }
        out
    }

    /// The tab a model answer names, if any is on.
    pub fn tab_for_choice(&self, choice: &str) -> Option<TabRef> {
        let c = choice.trim();
        if let Some(name) = MODEL_CATEGORIES.iter().find(|n| n.eq_ignore_ascii_case(c)) {
            return self.builtin(name).map(|_| TabRef::Builtin((*name).to_string()));
        }
        self.tabs
            .iter()
            .find(|t| t.enabled && matches!(&t.rule, SplitTabRule::DecisionCategory { category } if category.eq_ignore_ascii_case(c)))
            .map(|t| TabRef::Id(t.id.clone()))
    }

    /// The owner's layer, first in the "Which tab" list: their rules by position, then the tabs they added by order.
    pub fn custom_entries(&self, rules: &[Rule]) -> Vec<Entry> {
        let mut rs: Vec<&Rule> = rules.iter().filter(|r| r.enabled).collect();
        rs.sort_by_key(|r| r.position);
        let mut out: Vec<Entry> = rs
            .into_iter()
            .map(|r| Entry {
                id: format!("rule:{}", r.id),
                kind: EntryKind::Rule { when: r.conditions.iter().cloned().map(Cond::User).collect(), then: Then::Tab(TabRef::Id(r.tab_id.clone())) },
            })
            .collect();
        for t in self.tabs.iter().filter(|t| t.enabled) {
            if matches!(t.rule, SplitTabRule::Sender { .. } | SplitTabRule::Domain { .. } | SplitTabRule::Account { .. } | SplitTabRule::Label { .. }) {
                out.push(Entry {
                    id: format!("tab:{}", t.id),
                    kind: EntryKind::Rule { when: vec![Cond::TabRule(t.rule.clone())], then: Then::Tab(TabRef::Id(t.id.clone())) },
                });
            }
        }
        out
    }
}

/// An entry id written by the owner's layer (a rule row or a tab they added), as opposed to a shipped entry.
pub fn is_custom_entry_id(id: &str) -> bool {
    id.starts_with("rule:") || id.starts_with("tab:")
}
