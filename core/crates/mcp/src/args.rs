//! Tool arguments, parsed at the boundary. Whatever the outside app sends is untrusted: each field is checked
//! and refused in words, never cast.
use contracts::constants::SEARCH_DEFAULT_HITS;
use serde_json::Value;

use crate::domain::{McpError, NewDraft, SearchQuery};

fn invalid(field: &str, reason: &str) -> McpError {
    McpError::InvalidParams { field: field.to_string(), reason_words: reason.to_string() }
}

fn object<'a>(args: &'a Value) -> Result<&'a serde_json::Map<String, Value>, McpError> {
    args.as_object().ok_or_else(|| invalid("arguments", "must be an object"))
}

fn required_text(map: &serde_json::Map<String, Value>, field: &str) -> Result<String, McpError> {
    match map.get(field) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(_) => Err(invalid(field, "must be text")),
        None => Err(invalid(field, "is required")),
    }
}

fn optional_text(map: &serde_json::Map<String, Value>, field: &str) -> Result<Option<String>, McpError> {
    match map.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(invalid(field, "must be text")),
    }
}

/// One address: `name@host`, no spaces, no control characters, no list separators or angle brackets.
fn address(field: &str, raw: &str) -> Result<String, McpError> {
    let a = raw.trim();
    let bad = a.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, ',' | ';' | '<' | '>' | '"'));
    match a.split_once('@') {
        Some((local, host)) if !bad && !local.is_empty() && !host.is_empty() && !host.contains('@') => Ok(a.to_string()),
        _ => Err(invalid(field, "each address must look like name@example.com")),
    }
}

fn addresses(map: &serde_json::Map<String, Value>, field: &str, required: bool) -> Result<Vec<String>, McpError> {
    let list = match map.get(field) {
        None | Some(Value::Null) if !required => return Ok(Vec::new()),
        None | Some(Value::Null) => return Err(invalid(field, "is required")),
        Some(Value::Array(items)) => items,
        Some(_) => return Err(invalid(field, "must be a list of addresses")),
    };
    if required && list.is_empty() {
        return Err(invalid(field, "needs at least one address"));
    }
    list.iter()
        .map(|v| v.as_str().ok_or_else(|| invalid(field, "must be a list of addresses")).and_then(|s| address(field, s)))
        .collect()
}

/// `search_mail`: `query` (text, not empty) and an optional `limit` (a whole number from 1; never above SEARCH_DEFAULT_HITS).
pub fn search(args: &Value) -> Result<SearchQuery, McpError> {
    let map = object(args)?;
    let text = required_text(map, "query")?;
    if text.trim().is_empty() {
        return Err(invalid("query", "must not be empty"));
    }
    let ceiling = usize::try_from(SEARCH_DEFAULT_HITS).unwrap_or(usize::MAX);
    let limit = match map.get("limit") {
        None | Some(Value::Null) => ceiling,
        Some(v) => match v.as_u64() {
            Some(n) if n >= 1 => usize::try_from(n).unwrap_or(usize::MAX).min(ceiling),
            _ => return Err(invalid("limit", "must be a whole number from 1")),
        },
    };
    Ok(SearchQuery { text, limit })
}

/// `create_draft`: `to` (at least one address), optional `cc`, `subject` (one line), `body`, optional
/// `account_id` and `reply_to_thread_id`.
pub fn draft(args: &Value) -> Result<NewDraft, McpError> {
    let map = object(args)?;
    let to = addresses(map, "to", true)?;
    let cc = addresses(map, "cc", false)?;
    let subject = required_text(map, "subject")?;
    if subject.contains(['\r', '\n']) {
        return Err(invalid("subject", "must be one line"));
    }
    let body = required_text(map, "body")?;
    Ok(NewDraft {
        from_account_id: optional_text(map, "account_id")?,
        to,
        cc,
        subject,
        body,
        reply_to_thread_id: optional_text(map, "reply_to_thread_id")?,
    })
}
