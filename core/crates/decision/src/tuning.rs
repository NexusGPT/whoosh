//! Numbers this crate needs. Those the ledger holds are READ from docs/CONSTANTS.md at build time (never retyped). The
//! trust levels of the two smart-inbox model entries are that crate's (one home); the two below are not in the ledger yet
//! and are listed under "Constants to add" in docs/lanes/core-decision.md.
const LEDGER: &str = include_str!("../../../../docs/CONSTANTS.md");

pub use smart_inbox::tuning::{MODEL_TRUST_NEEDS_REPLY, MODEL_TRUST_TAB};

/// "Trust it from 90 %" on the Sign-in codes model entry (docs/DECISION-MODEL.md section 1).
pub const MODEL_TRUST_SIGN_IN_CODE: f64 = 0.90;
/// The smallest per-option cut when options overflow the head (`max(4, …)` in the authors' rl_common.py build_sequence,
/// as ported in Sources/Features/OnDeviceAI/Core/LayaSequence.swift).
pub const LAYA_MIN_OPTION_TOKENS: usize = 4;

/// The first number (integer or decimal, commas ignored) of a ledger row's Value cell; None when the row is missing or
/// its value is not a number yet (DECISION_PREVIEW_MS reads "measured at the … probe").
pub fn ledger_number(name: &str) -> Option<f64> {
    let line = LEDGER.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |")))?;
    let cell = line.split('|').nth(2)?.trim();
    if !cell.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let num: String = cell.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',').filter(|c| *c != ',').collect();
    num.parse().ok()
}

fn required(name: &str) -> f64 {
    ledger_number(name).unwrap_or_else(|| panic!("docs/CONSTANTS.md has no number for {name}"))
}

pub fn on_device_context_tokens() -> usize {
    required("ON_DEVICE_CONTEXT_TOKENS") as usize
}
pub fn on_device_chars_per_token_floor() -> f64 {
    required("ON_DEVICE_CHARS_PER_TOKEN_FLOOR")
}
pub fn laya_option_token_cap() -> usize {
    required("LAYA_OPTION_TOKEN_CAP") as usize
}
pub fn laya_option_budget_floor() -> usize {
    required("LAYA_OPTION_BUDGET_FLOOR") as usize
}
pub fn laya_min_head_tokens() -> usize {
    required("LAYA_MIN_HEAD_TOKENS") as usize
}
pub fn decision_preview_n() -> usize {
    required("DECISION_PREVIEW_N") as usize
}
/// None while the ledger row holds no measured number.
pub fn decision_preview_ms() -> Option<u64> {
    ledger_number("DECISION_PREVIEW_MS").map(|v| v as u64)
}
pub fn decision_body_chars() -> usize {
    smart_inbox::tuning::body_chars()
}
/// "Trust it from …" on the Labels model entry (contract delta 9). None while the ledger has no MODEL_TRUST_LABEL row:
/// the number is requested from the lead as a dated row, never written here. A value over 1 is read as a percentage.
pub fn model_trust_label() -> Option<f64> {
    ledger_number("MODEL_TRUST_LABEL").map(|v| if v > 1.0 { v / 100.0 } else { v })
}
/// The first word of a ledger row's Value cell (lowercase); None when the row is missing.
pub fn ledger_word(name: &str) -> Option<String> {
    let line = LEDGER.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |")))?;
    let cell = line.split('|').nth(2)?.trim();
    cell.split_whitespace().next().map(|w| w.to_lowercase())
}
/// Lane fix-aisort round 2: how the Labels model entry asks. `per-label` in the ledger row LABEL_AI_QUESTION = one yes/no question per
/// offered label ("this email belongs under <label>: <its description>"), whose yes-probability is the probability that THIS label
/// applies, the number MODEL_TRUST_LABEL is defined on. Missing row (today) = the one choice among the offered labels, whose number is
/// that label's share of a forced choice (`decide::question_for`).
pub fn labels_per_label() -> bool {
    ledger_word("LABEL_AI_QUESTION").is_some_and(|w| w == "per-label")
}
/// The bound of one "Re-run on N filed messages" batch. None while the ledger has no DECISION_RERUN_MS row.
pub fn decision_rerun_ms() -> Option<u64> {
    ledger_number("DECISION_RERUN_MS").map(|v| v as u64)
}
