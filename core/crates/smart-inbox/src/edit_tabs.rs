//! Use case: edit the split tabs. The edit and the re-sort of message_category run in ONE write transaction.
//! A rename touches only the name; placement never reads a name, so a rename moves nothing.
use crate::error::SmartInboxError;
use crate::load::{load_tabs, tab_rule_columns};
use crate::resort::{resort, ResortReport, Scope};
use contracts::{SplitTab, SplitTabRule};
use rusqlite::params;
use store::{Store, StoreError};

#[derive(Debug, Clone, PartialEq)]
pub enum TabEdit {
    Rename { id: String, name: String },
    /// The full new order of tab ids; a tab not listed keeps its place after them.
    Reorder { ids: Vec<String> },
    Add(SplitTab),
    Remove { id: String },
    SetEnabled { id: String, enabled: bool },
    SetRule { id: String, rule: SplitTabRule },
}

pub struct EditSplitTabs<'a> {
    pub store: &'a Store,
}

fn invalid(why: &str) -> StoreError {
    StoreError::Invalid(why.to_string())
}

impl EditSplitTabs<'_> {
    pub fn execute(&self, edit: TabEdit, now_ms: i64) -> Result<ResortReport, SmartInboxError> {
        Ok(self.store.write(move |ctx| {
            let tx = ctx.tx;
            let before = load_tabs(tx)?;
            let enabled_after = |id: &str, on: Option<bool>| before.tabs.iter().filter(|t| if t.id == id { on.unwrap_or(false) } else { t.enabled }).count();
            let scope = match edit {
                TabEdit::Rename { id, name } => {
                    if before.get(&id).is_none() {
                        return Err(invalid("no such tab"));
                    }
                    tx.execute("UPDATE split_tab SET name = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, name, now_ms])?;
                    return Ok(ResortReport::default());
                }
                TabEdit::Reorder { ids } => {
                    for (i, id) in ids.iter().enumerate() {
                        tx.execute("UPDATE split_tab SET sort_order = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, i as i64, now_ms])?;
                    }
                    let mut next = ids.len() as i64;
                    for t in before.tabs.iter().filter(|t| !ids.contains(&t.id)) {
                        tx.execute("UPDATE split_tab SET sort_order = ?2 WHERE id = ?1", params![t.id, next])?;
                        next += 1;
                    }
                    Scope::Custom
                }
                TabEdit::Add(tab) => {
                    if before.get(&tab.id).is_some() {
                        return Err(invalid("a tab with this id exists"));
                    }
                    if matches!(tab.rule, SplitTabRule::Builtin { .. }) {
                        return Err(invalid("a tab the owner adds needs a sender, domain, account, label or category rule"));
                    }
                    let (kind, value) = tab_rule_columns(&tab.rule);
                    tx.execute(
                        "INSERT INTO split_tab (id, name, sort_order, rule_kind, rule_value, enabled, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![tab.id, tab.name, tab.order as i64, kind, value, tab.enabled as i64, now_ms],
                    )?;
                    Scope::Tab(tab.id)
                }
                TabEdit::Remove { id } => {
                    if before.get(&id).is_none() {
                        return Err(invalid("no such tab"));
                    }
                    if enabled_after(&id, Some(false)) == 0 {
                        return Err(invalid("a split inbox keeps at least one tab"));
                    }
                    tx.execute("DELETE FROM split_tab WHERE id = ?1", params![id])?;
                    Scope::Tab(id)
                }
                TabEdit::SetEnabled { id, enabled } => {
                    if before.get(&id).is_none() {
                        return Err(invalid("no such tab"));
                    }
                    if enabled_after(&id, Some(enabled)) == 0 {
                        return Err(invalid("a split inbox keeps at least one tab"));
                    }
                    tx.execute("UPDATE split_tab SET enabled = ?2, updated_at_ms = ?3 WHERE id = ?1", params![id, enabled as i64, now_ms])?;
                    Scope::Tab(id)
                }
                TabEdit::SetRule { id, rule } => {
                    if before.get(&id).is_none() {
                        return Err(invalid("no such tab"));
                    }
                    let (kind, value) = tab_rule_columns(&rule);
                    tx.execute("UPDATE split_tab SET rule_kind = ?2, rule_value = ?3, updated_at_ms = ?4 WHERE id = ?1", params![id, kind, value, now_ms])?;
                    Scope::Tab(id)
                }
            };
            resort(ctx, &scope, now_ms)
        })?)
    }
}
