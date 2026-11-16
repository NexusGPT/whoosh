//! What the owner reads when a provider says no: the provider's own words, unchanged.
use contracts::{ActionFailed, CoreError};
use serde_json::Value;

use crate::key::Key;

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

/// The reason string out of a provider's error body. Each provider nests it differently; the string itself is never edited.
/// A body with no known shape is returned whole.
pub(crate) fn provider_words(body: &str) -> Option<String> {
    let raw = body.trim();
    if raw.is_empty() {
        return None;
    }
    let Ok(v) = serde_json::from_str::<Value>(raw) else { return Some(raw.to_string()) };
    text(&v["error"]["message"])
        .or_else(|| text(&v["error"]))
        .or_else(|| text(&v["err_msg"]))
        .or_else(|| text(&v["detail"]["message"]))
        .or_else(|| text(&v["detail"]))
        .or_else(|| text(&v["message"]))
        .or_else(|| Some(raw.to_string()))
}

/// A provider that echoes the whole key back would put it in front of the owner and into a log: the one edit made to its words.
fn scrub(words: String, key: Option<&Key>) -> String {
    match key {
        Some(k) if words.contains(k.as_str()) => words.replace(k.as_str(), "<key>"),
        _ => words,
    }
}

fn action(provider: &str, words: String, retryable: bool) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: provider.to_string(), reason_words: words, retryable, code: None } }
}

/// The provider answered with a refusal (a refused key, a quota, a bad model name).
pub(crate) fn refused(provider: &str, status: u16, body: &str, key: Option<&Key>) -> CoreError {
    let words = provider_words(body).unwrap_or_else(|| format!("HTTP {status}"));
    let retryable = status == 429 || status >= 500;
    action(provider, scrub(words, key), retryable)
}

/// The provider answered 2xx in a shape this app does not read.
pub(crate) fn unreadable(provider: &str) -> CoreError {
    action(provider, "answered in a shape this app does not read".to_string(), false)
}

/// No answer at all: the real transport message, so the owner sees "connection refused", not "failed".
pub(crate) fn unreachable(provider: &str, real: String, key: Option<&Key>) -> CoreError {
    action(provider, scrub(real, key), true)
}
