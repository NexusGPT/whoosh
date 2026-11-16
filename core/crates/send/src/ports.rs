//! Ports: one method each, named `execute`.
use crate::account::SmtpSettings;
use contracts::CoreError;

/// The SMTP envelope: bare addresses, no display names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Envelope {
    pub from: String,
    pub to: Vec<String>,
}

pub struct DeliverRequest {
    pub account_name: String,
    pub smtp: SmtpSettings,
    pub envelope: Envelope,
    pub rfc822: Vec<u8>,
}

/// Hands one message to a mail server. Implemented here by lettre; faked in tests.
pub trait DeliverPort: Send + Sync {
    fn execute(&self, request: &DeliverRequest) -> Result<(), CoreError>;
}

// AppendSentPort and its request moved to the contract crate (root delta 2, 2026-11-07) so the
// IMAP adapter implements it without depending on this crate; re-exported here so callers are unchanged.
pub use contracts::{AppendSentPort, AppendSentRequest};
