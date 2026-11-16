//! The Labels page built from the owner's label records (contract delta 9), and the reads around it.
//!
//! Owner, 2026-11-08 09:10: a label carries "explanation of what the label is, potential fixed rules to classify, etc. which
//! would allow for decision model to classify"; 09:45: "The decision model files into labels: fixed rules first, then the
//! label's meaning." So the Labels page is: one rule entry per label rule (the `Rule` records with action Label, by
//! position), then ONE model entry when a label carries a description, then no label. The rules are judged by the
//! contract's own `rule_set_matches`, the function the add-label sheet's live count and the store call: a rule is written
//! once, so what the sheet counts is what arrival files. An account-specific label is filed only on that account's mail
//! and is offered to the model only there.
//!
//! Delta 17 (docs/PRD.md "A94 pick"): a label is a flow of three cards. A140 (2026-11-11) replaces its order: the rule entries of this
//! page are the ENTRY cards of the AI-OFF labels whose entry card is on, in sidebar order (the label higher in the sidebar wins), each
//! label's chains by number; an AI-on label's entry chains pick the messages the model is asked about (`crate::flow::decide_pipeline`);
//! the EXIT card is applied after the model, or after step 1 of an AI-off label, by `crate::flow`.
use crate::entry::{label_paths, Entry, EntryKind, LabelNode, OutcomeList, Value, When};
use crate::outcome::Outcome;
use crate::tuning::model_trust_label;
use contracts::{count_rule_matches, rule_set_matches, ChainPlacement, CoreError, MessageFacts as RuleFacts, Rule, RuleAction, RuleCondition, RuleSet, RuleSetCount};
use smart_inbox::{Cond, MessageFacts};
use std::collections::HashMap;

/// The id of the Labels page's model entry (`decided_by` names it when the meaning decided).
pub const LABELS_MODEL_ID: &str = "labels/model";
/// The question of the Labels model entry. Each label's meaning follows it, one line per described label (`meaning_prompt`);
/// the contract's choice carries option names only, so the meaning rides in the instructions ("Contract deltas", 2).
pub const LABELS_QUESTION: &str = "Which of these labels fits this email? Answer with the label's full path, or a new label under one of them.";

/// The facts a label rule reads (`contracts::MessageFacts`) from the smart inbox's facts, field for field (the sender's
/// name and the unread bit arrived with contracts delta 12: a From rule matches the name as well as the address, and the
/// count's unread figure is real). The list header is the List-Id when it was at hand, else a marker word when any list
/// header was present.
pub fn rule_facts(f: &MessageFacts) -> RuleFacts {
    f.rule_facts()
}

/// A rule record's conditions and mode as the one rule set the contract judges.
pub fn rule_set(r: &Rule) -> RuleSet {
    RuleSet { conditions: r.conditions.clone(), match_mode: r.match_mode }
}

/// The label a rule files into, when it is a switched-on chain of a label's ENTRY card (delta 17: an exit chain files nothing by
/// itself, it filters what the model put there).
fn label_target(r: &Rule) -> Option<&str> {
    match (&r.action, r.enabled, r.placement) {
        (RuleAction::Label { label_id }, true, ChainPlacement::Entry) => Some(label_id.as_str()),
        _ => None,
    }
}

/// The question text the model reads for the Labels page: the sentence, then one line per described label,
/// "<path>: <meaning>", in path order. Labels with no description add no line, nor does a label whose AI card is off.
pub fn meaning_prompt(question: &str, labels: &[LabelNode]) -> String {
    let paths = label_paths(labels);
    let path_of: HashMap<&str, &str> = paths.iter().map(|(path, id)| (id.as_str(), path.as_str())).collect();
    let mut lines: Vec<String> = labels
        .iter()
        .filter(|l| l.ai_enabled && !l.description.trim().is_empty())
        .filter_map(|l| path_of.get(l.id.as_str()).map(|p| format!("{p}: {}", l.description.trim())))
        .collect();
    lines.sort();
    let mut out = question.to_string();
    for line in lines {
        out.push('\n');
        out.push_str(&line);
    }
    out
}

impl OutcomeList {
    /// The Labels page from the owner's labels and rules: one rule entry per ENTRY chain of a label whose entry card is on, in
    /// the order of `labels` (the sidebar order: the label higher in the sidebar wins), a label's chains by rule `position`
    /// (lead ruling 2026-11-09), then by chain number (a rule whose label is not in the tree, switched off, or an exit chain files nothing); an account-specific
    /// label's rule fires only on that account's mail; then ONE model entry trusted from `meaning_trust` when at least one label
    /// with its AI card on carries a description (None = rules only: the shipped page); then no label. Entry ids:
    /// `rule:<rule id>`, `labels/model`, `labels/default`.
    ///
    /// A140 (owner 2026-11-11, the three-step pipeline): when the page holds a model entry, the rule entries are the entry chains of
    /// the AI-OFF labels only (`flow::ai_step_on` false): they file by step 1 and win first, in sidebar order. An AI-on label's entry
    /// chains file nothing by themselves: they pick the messages the model is asked about (`flow::decide_pipeline`). With no model
    /// entry (no trust level, or no AI-on label) every label is a fixed-rules label and all their entry chains are rule entries.
    pub fn labels_from_records(labels: &[LabelNode], rules: &[Rule], meaning_trust: Option<f64>) -> OutcomeList {
        let k = Outcome::Labels.key();
        let with_model = meaning_trust.is_some() && labels.iter().any(crate::flow::ai_step_on);
        let rank = |r: &Rule| label_target(r).and_then(|id| labels.iter().position(|l| l.id == id && l.entry_enabled && !(with_model && crate::flow::ai_step_on(l))));
        let mut label_rules: Vec<(usize, &Rule)> = rules.iter().filter_map(|r| rank(r).map(|i| (i, r))).collect();
        // lead ruling 2026-11-09: label order first (the label higher in the sidebar wins), then rule position inside one label
        label_rules.sort_by(|(ia, a), (ib, b)| ia.cmp(ib).then(a.position.cmp(&b.position)).then(a.chain_id.cmp(&b.chain_id)));
        let label_rules: Vec<&Rule> = label_rules.into_iter().map(|(_, r)| r).collect();
        let mut entries: Vec<Entry> = label_rules
            .into_iter()
            .map(|r| {
                let label_id = label_target(r).unwrap_or("").to_string();
                let node = labels.iter().find(|l| l.id == label_id);
                let mut when = vec![When::LabelRules(rule_set(r))];
                if let Some(account_id) = node.and_then(|l| l.account_id.clone()) {
                    when.push(When::Fact(Cond::User(RuleCondition::Account { account_id })));
                }
                Entry { id: format!("rule:{}", r.id), kind: EntryKind::Rule { when, then: Value::Label(Some(label_id)) } }
            })
            .collect();
        if let Some(trust_from) = meaning_trust.filter(|_| with_model) {
            entries.push(Entry { id: LABELS_MODEL_ID.into(), kind: EntryKind::Model { question: LABELS_QUESTION.into(), trust_from } });
        }
        entries.push(Entry { id: format!("{k}/default"), kind: EntryKind::Default { then: Value::Label(None) } });
        OutcomeList { outcome: Outcome::Labels, entries }
    }

    /// The same page with the trust level read from the ledger (MODEL_TRUST_LABEL). Refused in words while the row is
    /// missing: the number is the lead's to add, never a literal here.
    pub fn labels_from_ledger(labels: &[LabelNode], rules: &[Rule]) -> Result<OutcomeList, CoreError> {
        let trust = model_trust_label().ok_or_else(|| CoreError::Invalid {
            field: "MODEL_TRUST_LABEL".into(),
            reason_words: "docs/CONSTANTS.md holds no trust level for the Labels model entry yet".into(),
        })?;
        Ok(OutcomeList::labels_from_records(labels, rules, Some(trust)))
    }

    /// Whether the page asks the model at all (a model entry is in the list).
    pub fn has_model_entry(&self) -> bool {
        self.model_entries().next().is_some()
    }
}

/// The live count of the fixed rules of `label_id` over `corpus` (the sheet's "14 messages · 3 unread · newest …"): the
/// contract's `count_rule_matches` over the same facts the filing reads, so the count and the filing agree on one corpus.
/// A label with several rules counts a message once.
pub fn label_rule_count(rules: &[Rule], label_id: &str, corpus: &[MessageFacts]) -> RuleSetCount {
    let sets: Vec<RuleSet> = rules.iter().filter(|r| label_target(r) == Some(label_id)).map(rule_set).collect();
    let facts: Vec<RuleFacts> = corpus.iter().map(rule_facts).collect();
    match sets.as_slice() {
        [one] => count_rule_matches(one.clone(), facts),
        _ => {
            let hits: Vec<&RuleFacts> = facts.iter().filter(|f| sets.iter().any(|s| rule_set_matches(s.clone(), (*f).clone()))).collect();
            RuleSetCount { count: hits.len() as u32, unread: hits.iter().filter(|f| f.unread).count() as u32, newest_at_ms: hits.iter().map(|f| f.received_at_ms).max() }
        }
    }
}
