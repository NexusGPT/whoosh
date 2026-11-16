//! gap-engine-tests (item f2dd23de): the two rule previews that answered a constant zero on the real core. Both read the local store
//! only and answer on the caller's thread, so they are fast reads, never network calls.
//!  - `words_count`: how many stored mails a smart-folder rule written in words matches. A smart folder is a saved search, so the count is
//!    the exact group of the owner's search for those words (`search_cmd::exact_matches`: every word held, the same filters such as
//!    `from:`), never the ranked meaning tail, kept to the mail still in the inbox (the test engine's reading: no archived, muted or snoozed
//!    mail, no sent copy). The group is the search's own, so it stops at SEARCH_RANKED_MAX.
//!  - `split_tab_count`: how many messages in the inbox a split-tab rule holds before the tab is saved. A sender, a domain and an account
//!    are rule sets counted by `smart_inbox::corpus::rule_set_count` (the labels screen's count); a built-in tab or a decision category is
//!    a tab of the strip, counted by the store's `tab_count` (the same rows its list pages through); a label counts the inbox mail filed
//!    under it (one SQL read, the inbox rows `VISIBLE` keeps: in the INBOX folder, not archived, not muted, not snoozed).
use std::collections::HashSet;

use contracts::{MatchMode, RuleCondition, RuleSet, SplitTabRule};
use store::Store;

use crate::rust_core::search_cmd::{self, Parts};

/// The hidden test copy's mutant `rule-counts-zero` (rust_core.rs, set only from the rig-gated `WHOOSH_CORE_MUTANT`): the label rule count answers 0.
/// Never set in the owner's copy.
static ZERO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn set_zero(on: bool) { ZERO.store(on, std::sync::atomic::Ordering::Relaxed); }

pub(crate) fn zeroed() -> bool { ZERO.load(std::sync::atomic::Ordering::Relaxed) }

/// The inbox rows `store::read::VISIBLE` keeps, written for one read here (that constant is private to the store crate).
const INBOX_VISIBLE: &str = "m.folder = 'INBOX' AND m.archived = 0 AND m.muted = 0 AND NOT EXISTS (SELECT 1 FROM snooze s WHERE s.message_id = m.id)";

pub(crate) fn words_count(parts: &Parts, rule_words: &str) -> u32 {
    let hits = match search_cmd::exact_matches(parts, rule_words) {
        Ok(hits) => hits,
        Err(e) => {
            eprintln!("rule_match_count: the mail database could not be read for the count: {e}");
            return 0;
        }
    };
    if hits.is_empty() {
        return 0;
    }
    // the sidebar's folder holds what is still in the inbox, as the test engine's does: no archived, muted or snoozed mail, no sent copy
    match listable_ids(&parts.store) {
        Ok(listed) => hits.iter().filter(|h| listed.contains(h.message_id.as_str())).count() as u32,
        Err(e) => {
            eprintln!("rule_match_count: the mail database could not be read for the count: {e}");
            0
        }
    }
}

/// The ids of the inbound mail still in the inbox (`INBOX_VISIBLE`, and not from one of the owner's own addresses).
fn listable_ids(store: &Store) -> Result<HashSet<String>, String> {
    let conn = search_text::source::open_reader(store.path()).map_err(|e| e.to_string())?;
    let mut st = conn
        .prepare(&format!("SELECT m.id FROM message m WHERE {INBOX_VISIBLE} AND lower(m.from_address) NOT IN (SELECT lower(address) FROM account)"))
        .map_err(|e| e.to_string())?;
    let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<HashSet<_>, _>>().map_err(|e| e.to_string())
}

fn one(condition: RuleCondition) -> RuleSet {
    RuleSet { conditions: vec![condition], match_mode: MatchMode::Any }
}

pub(crate) fn split_tab_count(store: &Store, rule: SplitTabRule) -> u32 {
    let rule_set = |condition| smart_inbox::corpus::rule_set_count(store, one(condition)).count;
    match rule {
        SplitTabRule::Sender { address } => rule_set(RuleCondition::From { words: address }),
        SplitTabRule::Domain { domain } => rule_set(RuleCondition::Domain { domain }),
        SplitTabRule::Account { account_id } => rule_set(RuleCondition::Account { account_id }),
        SplitTabRule::Builtin { name: tab } | SplitTabRule::DecisionCategory { category: tab } => store.tab_count(&tab).unwrap_or(0).min(u32::MAX as u64) as u32,
        SplitTabRule::Label { label_id } => label_count(store, &label_id),
    }
}

fn label_count(store: &Store, label_id: &str) -> u32 {
    let read = || -> Result<i64, String> {
        let conn = search_text::source::open_reader(store.path()).map_err(|e| e.to_string())?;
        conn.query_row(
            &format!("SELECT count(*) FROM message m JOIN label_assignment a ON a.message_id = m.id WHERE a.label_id = ?1 AND {INBOX_VISIBLE}"),
            rusqlite::params![label_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    };
    match read() {
        Ok(n) => n.max(0) as u32,
        Err(e) => {
            eprintln!("split_tab_rule_match_count: the mail database could not be read for the count: {e}");
            0
        }
    }
}
