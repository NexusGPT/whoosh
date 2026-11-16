//! The failures this adapter names. Every message carries the real body Google sent, never only the status.
use contracts::{ActionFailed, CoreError};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GmailError {
    /// The gmail_api_verified flag is off: the adapter is not built.
    FlagOff,
    Invalid { field: String, reason: String },
    /// 401: the access token is no longer accepted. The caller refreshes it, this adapter holds no credential.
    AuthExpired { message: String },
    /// startHistoryId is too old (HTTP 404). Handled inside `sync` by a full resync; public for completeness.
    HistoryExpired,
    RateLimited { message: String, attempts: u32 },
    Http { status: u16, message: String },
    Network { message: String },
    Decode { what: String, reason: String },
    /// The caller's sink refused a batch; the cursor must not advance.
    Sink { message: String },
}

impl fmt::Display for GmailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GmailError::FlagOff => write!(f, "the Gmail API connection is not switched on"),
            GmailError::Invalid { field, reason } => write!(f, "{field}: {reason}"),
            GmailError::AuthExpired { message } => write!(f, "Google no longer accepts the sign-in: {message}"),
            GmailError::HistoryExpired => write!(f, "Google no longer holds the change history from that point"),
            GmailError::RateLimited { message, attempts } => {
                write!(f, "Google is limiting requests, gave up after {attempts} tries: {message}")
            }
            GmailError::Http { status, message } => write!(f, "Google answered {status}: {message}"),
            GmailError::Network { message } => write!(f, "could not reach Google: {message}"),
            GmailError::Decode { what, reason } => write!(f, "could not read {what}: {reason}"),
            GmailError::Sink { message } => write!(f, "the store refused the batch: {message}"),
        }
    }
}

impl std::error::Error for GmailError {}

impl GmailError {
    /// Can the same call succeed later without the owner doing anything?
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            GmailError::RateLimited { .. } | GmailError::Network { .. } | GmailError::Http { status: 500..=599, .. }
        )
    }

    /// The owner-facing form: "<account_name>: <reason in words>" (contracts error vocabulary).
    pub fn to_core_error(&self, account_name: &str) -> CoreError {
        CoreError::Action {
            failure: ActionFailed {
                account_name: account_name.to_string(),
                reason_words: self.to_string(),
                retryable: self.retryable(),
                code: None,
            },
        }
    }
}
