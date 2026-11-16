//! The six default split tabs. Their names live once, in docs/CONSTANTS.md (SPLIT_TABS_DEFAULT);
//! this file reads that row at build time and seeds the table at first launch (migrations lane finding 5).
use crate::error::StoreError;
use rusqlite::{params, Connection};

const CONSTANTS: &str = include_str!("../../../../docs/CONSTANTS.md");

/// The default tab names, in order, from the ledger row.
pub fn default_tab_names() -> Vec<String> {
    CONSTANTS
        .lines()
        .find(|l| l.trim_start().starts_with("| SPLIT_TABS_DEFAULT |"))
        .and_then(|l| l.split('|').nth(2))
        .map(|cell| cell.split(',').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect())
        .unwrap_or_default()
}

/// "Needs Reply" -> "needs-reply".
pub fn tab_id(name: &str) -> String {
    name.trim().to_lowercase().replace(' ', "-")
}

/// Insert the defaults when split_tab is empty. Seeded rows carry updated_at_ms 0, so any real edit wins.
pub fn seed_split_tabs(conn: &Connection) -> Result<usize, StoreError> {
    let have: i64 = conn.query_row("SELECT count(*) FROM split_tab", [], |r| r.get(0))?;
    if have > 0 {
        return Ok(0);
    }
    let names = default_tab_names();
    if names.is_empty() {
        return Err(StoreError::Invalid("docs/CONSTANTS.md has no SPLIT_TABS_DEFAULT row".into()));
    }
    for (i, name) in names.iter().enumerate() {
        conn.execute(
            "INSERT INTO split_tab (id, name, sort_order, rule_kind, rule_value, enabled, updated_at_ms) VALUES (?1, ?2, ?3, 'builtin', ?2, 1, 0)",
            params![tab_id(name), name, i as i64],
        )?;
    }
    Ok(names.len())
}
