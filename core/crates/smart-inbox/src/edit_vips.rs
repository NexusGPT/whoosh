//! Use case: mark or unmark a VIP (an address or a domain). The VIP row and the moves commit together.
//! The VIP entry is the first of the shipped "Which tab" list, so a newly marked sender's mail goes to the VIPs tab at once
//! and an unmarked sender's mail that the VIP entry placed goes back to Primary. A row the owner moved by hand stays.
use crate::error::SmartInboxError;
use crate::list::DecisionPage;
use crate::load::{is_vip, load_context, load_vips, write_placements};
use crate::place::Placement;
use contracts::DecidedBy;
use rusqlite::params;
use store::Store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VipEdit {
    MarkAddress(String),
    MarkDomain(String),
    UnmarkAddress(String),
    UnmarkDomain(String),
}

pub const VIP_ENTRY_ID: &str = "which-tab/vip";

pub struct EditVips<'a> {
    pub store: &'a Store,
}

impl EditVips<'_> {
    /// Returns the ids of the messages that changed tab.
    pub fn execute(&self, edit: VipEdit, now_ms: i64) -> Result<Vec<String>, SmartInboxError> {
        Ok(self.store.write(move |ctx| {
            let (kind, value, mark) = match &edit {
                VipEdit::MarkAddress(v) => ("address", v.trim().to_ascii_lowercase(), true),
                VipEdit::MarkDomain(v) => ("domain", v.trim().trim_start_matches('@').to_ascii_lowercase(), true),
                VipEdit::UnmarkAddress(v) => ("address", v.trim().to_ascii_lowercase(), false),
                VipEdit::UnmarkDomain(v) => ("domain", v.trim().trim_start_matches('@').to_ascii_lowercase(), false),
            };
            if mark {
                ctx.tx.execute("INSERT INTO vip (kind, value, updated_at_ms) VALUES (?1, ?2, ?3) ON CONFLICT(kind, value) DO UPDATE SET updated_at_ms = excluded.updated_at_ms", params![kind, value, now_ms])?;
            } else {
                ctx.tx.execute("DELETE FROM vip WHERE kind = ?1 AND lower(value) = ?2", params![kind, value])?;
            }
            let c = load_context(ctx.tx)?;
            let vips = load_vips(ctx.tx)?;
            let vip_tab = c.tabs.builtin("VIPs").map(|t| t.id.clone());
            let primary = c.tabs.primary();
            let mut st = ctx.tx.prepare_cached(
                "SELECT m.id, m.from_address, COALESCE(mc.tab_id, ?1), mc.decided_by, mc.rule_id FROM message m LEFT JOIN message_category mc ON mc.message_id = m.id \
                 WHERE m.folder = 'INBOX' AND (lower(m.from_address) = ?2 OR lower(m.from_address) LIKE '%@' || ?2 OR lower(m.from_address) LIKE '%.' || ?2)",
            )?;
            let rows: Vec<(String, String, String, Option<String>, Option<String>)> =
                st.query_map(params![primary, value], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<_, _>>()?;
            drop(st);
            let mut writes = Vec::new();
            for (id, from, tab, by, rule_id) in rows {
                if by.as_deref() == Some("owner") {
                    continue;
                }
                let now_vip = is_vip(&vips, &from);
                let target = match (now_vip, &vip_tab) {
                    (true, Some(v)) if !(by.as_deref() == Some("rule") && rule_id.as_deref().is_some_and(crate::tabs::is_custom_entry_id)) => Some((v.clone(), DecidedBy::Rule { rule_id: VIP_ENTRY_ID.into() }, VIP_ENTRY_ID)),
                    (false, _) if rule_id.as_deref() == Some(VIP_ENTRY_ID) => Some((primary.clone(), DecidedBy::Default, "which-tab/default")),
                    _ => None,
                };
                if let Some((tab_id, decided_by, entry_id)) = target {
                    if tab_id != tab || !matches!(&decided_by, DecidedBy::Rule { .. }) {
                        writes.push(Placement { message_id: id, tab_id, decided_by, probability: None, page: DecisionPage::WhichTab, entry_position: 1, entry_id: entry_id.into() });
                    }
                }
            }
            let moved: Vec<String> = writes.iter().map(|p| p.message_id.clone()).collect();
            write_placements(ctx, &writes, now_ms)?;
            Ok(moved)
        })?)
    }
}
