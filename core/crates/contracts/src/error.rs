//! The error vocabulary. Every failure the owner sees names the account, says why in words,
//! and says whether a retry can help (docs/CAPABILITY-CONTRACT.md "event" column).
use std::fmt;

/// One failed action, as the owner reads it: "<account_name>: <reason_words>".
/// `account_name` is the account's DISPLAY name ("Provanta RFP"), never its id or address.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ActionFailed {
    pub account_name: String,
    pub reason_words: String,
    pub retryable: bool,
    /// The server's own code beside the words (delta 12): the SMTP reply code and, when the server sent one, its RFC 3463
    /// enhanced status, "451 4.7.1" (`smtp_failure_code`). None when the server gave no code or the failure is not a
    /// server's. The words stay the whole message; the code is shown next to them, never instead of them.
    #[uniffi(default = None)]
    pub code: Option<String>,
}

#[derive(uniffi::Error, Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// An action the owner asked for did not happen.
    Action { failure: ActionFailed },
    /// The input crossed a boundary in a shape the contract refuses (parse, don't cast).
    Invalid { field: String, reason_words: String },
    /// The thing named does not exist.
    NotFound { kind: String, id: String },
    /// A Swift implementation of a foreign trait threw something the contract does not name.
    Unexpected { reason_words: String },
    /// A provider asked us to slow down (added 2026-11-07, sync engine): `provider` names it in words
    /// ("Gmail", "Microsoft 365"); `retry_after_ms` is its own wait when it named one.
    Throttled { provider: String, retry_after_ms: Option<u64> },
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Action { failure } => write!(f, "{}: {}", failure.account_name, failure.reason_words),
            CoreError::Invalid { field, reason_words } => write!(f, "{field}: {reason_words}"),
            CoreError::NotFound { kind, id } => write!(f, "no {kind} {id}"),
            CoreError::Unexpected { reason_words } => write!(f, "{reason_words}"),
            CoreError::Throttled { provider, retry_after_ms: Some(ms) } => write!(f, "{provider} asks us to slow down; trying again in {} s", ms.div_ceil(1000)),
            CoreError::Throttled { provider, retry_after_ms: None } => write!(f, "{provider} asks us to slow down; trying again shortly"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<uniffi::UnexpectedUniFFICallbackError> for CoreError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        CoreError::Unexpected { reason_words: e.reason }
    }
}
