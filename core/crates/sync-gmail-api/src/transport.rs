//! The seams the adapter is built on: HTTP, the access token, the clock and the feature flag.
//! The adapter never opens a socket, reads a credential or looks at the clock by itself.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// One call to the Gmail REST API. `path` is relative to `users/me/`, for example `history` or `messages/abc`.
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub bearer: String,
    pub json_body: Option<String>,
}

// The bearer token never reaches a log line.
impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("method", &self.method)
            .field("path", &self.path)
            .field("query", &self.query)
            .field("bearer", &"<redacted>")
            .field("json_body", &self.json_body)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
    /// From a Retry-After header, in milliseconds.
    pub retry_after_ms: Option<i64>,
}

/// Blocking HTTP. `Err` is a failure before any answer (no route, timeout), with the real message.
pub trait Transport: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, String>;
}

/// The current access token. core-secrets and signin own the token; this adapter only asks for it per call.
pub trait TokenSource: Send + Sync {
    fn bearer(&self) -> Result<String, String>;
}

/// Time and waiting, so the fixture runs move a virtual clock.
pub trait Pacer: Send + Sync {
    fn now_ms(&self) -> i64;
    fn sleep_ms(&self, ms: i64);
}

pub struct SystemPacer;

impl Pacer for SystemPacer {
    fn now_ms(&self) -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    }
    fn sleep_ms(&self, ms: i64) {
        if ms > 0 {
            std::thread::sleep(std::time::Duration::from_millis(ms as u64));
        }
    }
}

/// Named product flags (the owner-visible switches live elsewhere; this adapter only asks).
pub trait FeatureFlags {
    fn is_on(&self, name: &str) -> bool;
}
