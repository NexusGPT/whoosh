//! The HTTP seam. Adapters build a `Request` and read a `Response`; tests replay fixtures through it.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Body {
    None,
    Json(String),
    /// Raw bytes with their content type (a multipart form, or an audio clip as sent to Deepgram).
    Bytes { content_type: String, data: Vec<u8> },
}

#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

const SECRET_HEADERS: [&str; 4] = ["authorization", "x-api-key", "x-goog-api-key", "xi-api-key"];

// A key never reaches a log line: header values that carry one print as <redacted>.
impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(&str, &str)> = self
            .headers
            .iter()
            .map(|(k, v)| (k.as_str(), if SECRET_HEADERS.contains(&k.to_ascii_lowercase().as_str()) { "<redacted>" } else { v.as_str() }))
            .collect();
        let body = match &self.body {
            Body::None => "none".to_string(),
            Body::Json(j) => format!("json, {} bytes", j.len()),
            Body::Bytes { content_type, data } => format!("{content_type}, {} bytes", data.len()),
        };
        f.debug_struct("Request").field("method", &self.method).field("url", &self.url).field("headers", &headers).field("body", &body).finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
    /// Rate limits and server faults can pass on a later try; a refused key cannot.
    pub fn is_retryable(&self) -> bool {
        self.status == 429 || self.status >= 500
    }
}

/// Blocking HTTP. `Err` is a failure before any answer (no route, refused connection, timeout), with the real message.
pub trait Transport: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, String>;
}
