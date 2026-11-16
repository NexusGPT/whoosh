//! Re-sort message_category after an edit. It re-evaluates the OWNER'S layer only (their rules and the tabs they added):
//! the shipped entries and the model are not asked again, because the stored facts do not hold the List-Id or the Gmail
//! labels they read, and a rename or a reorder must not move mail the model or a header put somewhere. So:
//! - a message the owner's layer now matches goes to that tab (decided_by = the rule or tab entry);
//! - a message the owner's layer used to hold and no longer matches, or that sits in a tab that is gone or off, goes back
//!   to Primary (decided_by default);
//! - everything else stays; a row decided_by 'owner' never moves.
use crate::ask::NoModel;
use crate::list::{Cond, EntryKind};
use crate::load::{facts_for, load_context, tab_rule_columns, write_placements, Context};
use crate::place::{rule_id_with_node, Placement, Placer};
use crate::list::DecisionPage;
use crate::tabs::is_custom_entry_id;
use contracts::{DecidedBy, SplitTabRule};
use rusqlite::{params, Connection};
use std::collections::BTreeSet;
use store::{StoreError, WriteCtx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Everything that sits in this tab, or that its rule matches.
    Tab(String),
    /// Everything the owner's layer holds or matches (a reorder, a rule edit, a VIP edit).
    Custom,
    /// Every INBOX message.
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub message_id: String,
    pub from_tab: String,
    pub to_tab: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResortReport {
    pub moved: Vec<Moved>,
    pub examined: usize,
}

fn ids(conn: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> rusqlite::Result<Vec<String>> {
    let mut st = conn.prepare_cached(sql)?;
    let rows = st.query_map(args, |r| r.get::<_, String>(0))?;
    rows.collect()
}

/// The messages a tab's rule can match, cut down by SQL first (the facts decide after).
fn matching(conn: &Connection, rule: &SplitTabRule) -> rusqlite::Result<Vec<String>> {
    let (kind, value) = tab_rule_columns(rule);
    match kind {
        "sender" => ids(conn, "SELECT id FROM message WHERE folder = 'INBOX' AND lower(from_address) = lower(?1)", &[&value]),
        "domain" => ids(conn, "SELECT id FROM message WHERE folder = 'INBOX' AND (lower(from_address) LIKE '%@' || lower(?1) OR lower(from_address) LIKE '%.' || lower(?1))", &[&value]),
        "account" => ids(conn, "SELECT id FROM message WHERE folder = 'INBOX' AND account_id = ?1", &[&value]),
        "label" => ids(
            conn,
            "WITH RECURSIVE sub(id) AS (SELECT ?1 UNION SELECT l.id FROM label l JOIN sub ON l.parent_id = sub.id) \
             SELECT DISTINCT message_id FROM label_assignment WHERE label_id IN (SELECT id FROM sub)",
            &[&value],
        ),
        _ => Ok(vec![]),
    }
}

fn candidates(conn: &Connection, c: &Context, scope: &Scope) -> rusqlite::Result<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    let user_rules_matter = c.rules.iter().any(|r| r.enabled);
    match scope {
        Scope::All => out.extend(ids(conn, "SELECT id FROM message WHERE folder = 'INBOX'", &[])?),
        _ if user_rules_matter => out.extend(ids(conn, "SELECT id FROM message WHERE folder = 'INBOX'", &[])?),
        Scope::Tab(id) => {
            out.extend(ids(conn, "SELECT message_id FROM message_category WHERE tab_id = ?1", &[id])?);
            if let Some(t) = c.tabs.get(id).filter(|t| t.enabled) {
                out.extend(matching(conn, &t.rule)?);
            }
        }
        Scope::Custom => {
            out.extend(ids(conn, "SELECT message_id FROM message_category WHERE decided_by = 'rule' AND (rule_id LIKE 'tab:%' OR rule_id LIKE 'rule:%')", &[])?);
            for t in c.tabs.tabs.iter().filter(|t| t.enabled) {
                out.extend(matching(conn, &t.rule)?);
            }
        }
    }
    Ok(out)
}

/// Runs inside the caller's write transaction, so an edit and its re-sort commit together or not at all.
pub fn resort(ctx: &mut WriteCtx<'_>, scope: &Scope, now_ms: i64) -> Result<ResortReport, StoreError> {
    let tx = ctx.tx;
    let c = load_context(tx)?;
    let placer = Placer::new(&c.tabs, &c.rules, &NoModel);
    let custom: Vec<_> = placer.which.entries.iter().enumerate().filter(|(_, e)| is_custom_entry_id(&e.id)).collect();
    let default_position = placer.which.entries.len();
    let primary = c.tabs.primary();
    let mut report = ResortReport::default();
    let mut writes: Vec<Placement> = Vec::new();
    for id in candidates(tx, &c, scope)? {
        let Some(f) = facts_for(tx, &c, std::slice::from_ref(&id))?.pop() else { continue };
        report.examined += 1;
        let current: Option<(String, String, Option<String>)> = tx
            .prepare_cached("SELECT tab_id, decided_by, rule_id FROM message_category WHERE message_id = ?1")?
            .query_row(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(Some)
            .or_else(|e| if matches!(e, rusqlite::Error::QueryReturnedNoRows) { Ok(None) } else { Err(e) })?;
        if current.as_ref().is_some_and(|(_, by, _)| by == "owner") {
            continue;
        }
        let hit = custom.iter().find(|(_, e)| match &e.kind {
            EntryKind::Rule { when, .. } => when.iter().all(|cond| matches!(cond, Cond::User(_) | Cond::TabRule(_)) && cond.pure(&f) == Some(true)),
            _ => false,
        });
        let from_tab = current.as_ref().map(|(t, _, _)| t.clone()).unwrap_or_else(|| primary.clone());
        let placement = match (hit, &current) {
            (Some((i, e)), _) => {
                let EntryKind::Rule { when, then: crate::list::Then::Tab(r) } = &e.kind else { continue };
                Placement { message_id: id.clone(), tab_id: c.tabs.resolve(r), decided_by: DecidedBy::Rule { rule_id: rule_id_with_node(&e.id, when, &f) }, probability: None, page: DecisionPage::WhichTab, entry_position: i + 1, entry_id: e.id.clone() }
            }
            (None, Some((tab, by, rule_id))) if (by == "rule" && rule_id.as_deref().is_some_and(is_custom_entry_id)) || !c.tabs.is_enabled(tab) => {
                Placement { message_id: id.clone(), tab_id: primary.clone(), decided_by: DecidedBy::Default, probability: None, page: DecisionPage::WhichTab, entry_position: default_position, entry_id: "which-tab/default".into() }
            }
            _ => continue,
        };
        if placement.tab_id != from_tab {
            report.moved.push(Moved { message_id: id.clone(), from_tab, to_tab: placement.tab_id.clone() });
        }
        writes.push(placement);
    }
    write_placements(ctx, &writes, now_ms)?;
    Ok(report)
}
