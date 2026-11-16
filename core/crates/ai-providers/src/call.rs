//! One request through the seam, answered as JSON or as the provider's words.
use contracts::CoreError;
use serde_json::Value;

use crate::key::Key;
use crate::transport::{Request, Transport};
use crate::words;

pub(crate) fn json(transport: &dyn Transport, provider: &str, request: Request, key: Option<&Key>) -> Result<Value, CoreError> {
    let response = transport.send(&request).map_err(|real| words::unreachable(provider, real, key))?;
    if !response.is_success() {
        return Err(words::refused(provider, response.status, &response.body, key));
    }
    serde_json::from_str(&response.body).map_err(|_| words::unreadable(provider))
}

/// RFC 3986 unreserved characters stay; everything else is percent-encoded.
pub(crate) fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
