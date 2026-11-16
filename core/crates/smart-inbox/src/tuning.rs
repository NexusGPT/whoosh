//! Numbers this crate needs. Those the ledger holds are READ from docs/CONSTANTS.md (never retyped);
//! the two trust levels come from docs/DECISION-MODEL.md section 1 and are not in the ledger yet
//! (lane notes "Constants to add": MODEL_TRUST_TAB, MODEL_TRUST_NEEDS_REPLY).
const CONSTANTS: &str = include_str!("../../../../docs/CONSTANTS.md");

/// "Trust it from 80 %" on the Which tab entry.
pub const MODEL_TRUST_TAB: f64 = 0.80;
/// "Trust it from 75 %" on the Needs my reply entry.
pub const MODEL_TRUST_NEEDS_REPLY: f64 = 0.75;

/// The first integer of the Value cell of a ledger row.
pub fn ledger_int(name: &str) -> Option<usize> {
    let line = CONSTANTS.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |")))?;
    let cell = line.split('|').nth(2)?;
    let digits: String = cell.trim().chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
    digits.parse().ok()
}

/// DECISION_BODY_CHARS: how much of the body a model entry may be shown.
pub fn body_chars() -> usize {
    ledger_int("DECISION_BODY_CHARS").unwrap_or(2000)
}
