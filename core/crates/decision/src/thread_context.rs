//! The rest of the thread as a fourth input of a label's AI card (lane labels-r5; owner 2026-11-10: "here in AI filter we should add
//! 'rest of message thread' - beware because laya has a size limit (same for Jev)").
//!
//! The smart inbox loads the earlier messages of the thread (`smart_inbox::thread::attach_thread`); `decide::state_with_thread` hands
//! them to the engine as the state field `thread` (JSON, oldest first: the contract's `DecisionState` has no thread slot, and a
//! field the adapters do not know is ignored, so an engine that never heard of this input still works). Each ADAPTER then cuts them to
//! ITS model's budget with `fit`: oldest messages dropped first, the newest kept whole, and a newest message bigger than the whole
//! budget cut short instead of refused. Nothing here can fail a decision: a budget too small for even one line gives no thread.
//!
//! Budgets (the limits the providers already declare, never typed here):
//! - Jev: `DECISION_BODY_CHARS` (docs/CONSTANTS.md, "what reaches Jev per message") is the size of one piece of message text. The thread
//!   gets the same size again, or `DECISION_THREAD_CHARS` when the ledger holds that row (a PROPOSAL in docs/lanes/labels-r5.md).
//! - Local (Laya): the room the checkpoint's window leaves after the questions and the head, then after the body (`adapters::local`),
//!   in tokens, converted with ON_DEVICE_CHARS_PER_TOKEN_FLOOR, which counts more tokens than the model will.
use crate::tuning::{decision_body_chars, ledger_number, on_device_chars_per_token_floor};
use serde_json::{json, Value};
use smart_inbox::facts::ThreadPart;

/// The state field that carries the encoded thread.
pub const THREAD_FIELD: &str = "thread";
/// The line that opens the thread in the text the model reads.
pub const THREAD_HEAD: &str = "earlier messages of this thread, newest first:";

/// The Jev budget in characters: the ledger's thread row when it exists, else the body's declared size.
pub fn jev_budget_chars() -> usize {
    ledger_number("DECISION_THREAD_CHARS").map(|v| v as usize).unwrap_or_else(decision_body_chars)
}

/// The most text any model can use of the thread, in characters: what the loader may stop at. The largest budget is the on-device
/// window (the biggest declared size), or the Jev budget when that is larger.
pub fn load_up_to_chars() -> usize {
    let window_chars = (crate::tuning::on_device_context_tokens() as f64 * on_device_chars_per_token_floor()).floor() as usize;
    window_chars.max(jev_budget_chars())
}

/// A token room as characters, at the pessimistic ratio the on-device budget uses.
pub fn chars_for_tokens(tokens: usize) -> usize {
    (tokens as f64 * on_device_chars_per_token_floor()).floor() as usize
}

/// "YYYY-MM-DD" of an epoch millisecond (UTC), by the civil-from-days rule; no clock crate is needed for ten characters.
fn day_words(at_ms: i64) -> String {
    let days = at_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// One message as a line: "[2026-11-09] me: text" or "[2026-11-09] anna@x.example: text".
fn line(p: &ThreadPart) -> String {
    let who = if p.from_owner { "me".to_string() } else { p.from.clone() };
    format!("[{}] {}: {}", day_words(p.at_ms), who, p.text.replace('\n', " "))
}

/// The thread cut to `budget_chars`, oldest first dropped: the newest messages that fit whole, as a block that ends at the newest
/// one (a gap in the middle would read as a conversation that never happened). When even the newest alone is over the budget its text
/// is cut to what is left and kept; when the budget cannot hold the head and one short line, nothing is kept. `parts` and the result
/// are oldest first. Never fails, never exceeds `budget_chars`.
pub fn fit(parts: &[ThreadPart], budget_chars: usize) -> Vec<ThreadPart> {
    let head = THREAD_HEAD.chars().count();
    let mut room = budget_chars.saturating_sub(head);
    let mut kept: Vec<ThreadPart> = Vec::new();
    for p in parts.iter().rev() {
        let need = 1 + line(p).chars().count();
        if need <= room {
            room -= need;
            kept.push(p.clone());
            continue;
        }
        if kept.is_empty() {
            // the newest message alone is over the budget: keep the start of it
            let overhead = need - p.text.chars().count();
            if room > overhead {
                let mut cut = p.clone();
                cut.text = p.text.chars().take(room - overhead).collect();
                kept.push(cut);
            }
        }
        break;
    }
    kept.reverse();
    kept
}

/// The thread as the text the model reads: the head, then one line per message, newest first. Empty for no message (no head alone).
pub fn render(parts: &[ThreadPart]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut out = String::from(THREAD_HEAD);
    for p in parts.iter().rev() {
        out.push('\n');
        out.push_str(&line(p));
    }
    out
}

/// The thread as the state field's JSON, oldest first.
pub fn encode(parts: &[ThreadPart]) -> String {
    Value::Array(parts.iter().map(|p| json!({"from": p.from, "at_ms": p.at_ms, "me": p.from_owner, "text": p.text})).collect()).to_string()
}

/// Reads the state field back; a field that is not this JSON gives no messages (a decision never fails on it).
pub fn decode(field: &str) -> Vec<ThreadPart> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(field) else { return vec![] };
    items
        .iter()
        .filter_map(|v| {
            Some(ThreadPart { from: v.get("from")?.as_str()?.to_string(), at_ms: v.get("at_ms")?.as_i64()?, from_owner: v.get("me")?.as_bool()?, text: v.get("text")?.as_str()?.to_string() })
        })
        .collect()
}

/// The thread of a state, cut to `budget_chars` and rendered: "" when the state carries none or the budget holds nothing.
pub fn text_of(state_fields: &std::collections::HashMap<String, String>, budget_chars: usize) -> String {
    match state_fields.get(THREAD_FIELD) {
        Some(field) => render(&fit(&decode(field), budget_chars)),
        None => String::new(),
    }
}
