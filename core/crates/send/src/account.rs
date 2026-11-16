//! What one account sends with. The secret is resolved by the caller (core-secrets) and
//! handed in for the call; this crate never stores it and never prints it.
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    /// TLS from the first byte (port 465 style).
    Implicit,
    /// Plain greeting, then STARTTLS (port 587 style).
    StartTls,
    /// No TLS. Refused for any host that is not the loopback address.
    Plaintext,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SmtpCredentials {
    pub username: String,
    /// an app password, or (with `bearer`) the OAuth2 access token
    pub secret: String,
    /// contracts delta 14: `secret` is an OAuth2 access token, sent by SASL XOAUTH2 (Microsoft 365, Gmail), never by
    /// PLAIN or LOGIN. The caller hands a token that is not about to expire (oauth::RefreshingToken).
    pub bearer: bool,
}

impl fmt::Debug for SmtpCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmtpCredentials").field("username", &self.username).field("secret", &"<hidden>").finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub security: Security,
    pub credentials: Option<SmtpCredentials>,
}

/// The sending account: the id the draft names, the words the owner reads in a failure,
/// the address every message is sent From, and how to reach its SMTP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendAccount {
    pub id: String,
    pub name: String,
    pub address: String,
    pub smtp: SmtpSettings,
}
