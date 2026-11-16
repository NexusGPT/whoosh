//! `Command::SaveDecisionList` (gap-decisions): the rules the owner files under a node of the label tree on the Labels decision page.
//!
//! A rule of the owner may file under ANY node, also one whose AI card is off (the node is a plain folder for it); the model is offered only the
//! labels whose AI card is on (`decision::decide`, unchanged). A rule is one chain of the node's entry card (`store.label_chain_add`), the same record
//! the label inspector writes, so what the list writes and what the inspector shows are one fact. Nothing is written unless every rule and node is good.
use contracts::{labels_without_views, validate_chain, ChainPlacement, CoreError, RuleSet};
use store::Store;

use crate::rust_core::now_ms;

fn unexpected(what: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::Unexpected { reason_words: format!("{what}: {e}") }
}

/// Writes the rules and answers the labels that gained a chain (once each, in the order given); the caller re-runs them.
pub(crate) fn save(store: &Store, page_id: &str, node_ids: &[String], rules: &[RuleSet]) -> Result<Vec<String>, CoreError> {
    if page_id != "labels" {
        return Err(CoreError::Invalid { field: "page_id".into(), reason_words: format!("only the Labels page files mail under a node of the label tree, not “{page_id}”") });
    }
    if node_ids.len() != rules.len() {
        return Err(CoreError::Invalid { field: "rules".into(), reason_words: "every rule needs the node it files under".into() });
    }
    let known: Vec<String> = labels_without_views(store.labels_full().map_err(|e| unexpected("the labels could not be read", e))?).into_iter().map(|l| l.id).collect();
    for (id, rule) in node_ids.iter().zip(rules) {
        if !known.contains(id) {
            return Err(CoreError::NotFound { kind: "label".into(), id: id.clone() });
        }
        validate_chain(rule.clone())?;
    }
    let now = now_ms();
    let mut written: Vec<String> = Vec::new();
    for (id, rule) in node_ids.iter().zip(rules) {
        let held = store.label_chains(id, ChainPlacement::Entry).map_err(|e| unexpected("the node's rules could not be read", e))?;
        if !held.iter().any(|c| &c.rules == rule) {
            store.label_chain_add(id.clone(), ChainPlacement::Entry, rule.clone(), now)?;
        }
        if !written.contains(id) {
            written.push(id.clone());
        }
    }
    Ok(written)
}
