//! What the adapter is told about one account. No number is defaulted here: every size and
//! duration arrives from the caller, who reads it from docs/CONSTANTS.md (Every number lives once).
use std::fmt;
use std::time::Duration;

use contracts::CoreError;

/// How the account signs in. The secret value is read from the Keychain by the caller and held
/// in memory only; `Debug` never prints it, so no log line can carry it.
#[derive(Clone)]
pub enum JmapAuth {
    /// An API token or an OAuth access token (Fastmail: OAuth with PKCE, or an API token).
    Bearer(String),
    Basic { user: String, password: String },
}

impl fmt::Debug for JmapAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JmapAuth::Bearer(_) => f.write_str("JmapAuth::Bearer(<redacted>)"),
            JmapAuth::Basic { user, .. } => write!(f, "JmapAuth::Basic {{ user: {user:?}, password: <redacted> }}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct JmapConfig {
    /// The contract `Account.id` the messages belong to.
    pub account_id: String,
    /// The words the owner knows the account by; every failure starts with it.
    pub account_name: String,
    /// The JMAP origin without a trailing slash, for example `https://api.fastmail.com`.
    /// The session document is read from `<base_url>/.well-known/jmap`.
    pub base_url: String,
    pub auth: JmapAuth,
    /// Hosts a redirect may lead to (a host that publishes its JMAP service elsewhere).
    pub redirect_hosts: Vec<String>,
    /// Timeout of every request; also the connect timeout of the EventSource stream.
    pub request_timeout: Duration,
}

/// The origin to probe for an address: the domain after the `@`, over HTTPS.
/// A host that publishes JMAP elsewhere answers with a redirect (`redirect_hosts`).
pub fn discovery_base(address: &str) -> Result<String, CoreError> {
    let domain = address.rsplit_once('@').map(|(_, d)| d.trim()).filter(|d| !d.is_empty() && !d.contains('/'));
    domain.map(|d| format!("https://{d}")).ok_or_else(|| CoreError::Invalid {
        field: "address".into(),
        reason_words: format!("{address:?} has no domain to look for a JMAP service on"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_secret() {
        let a = JmapAuth::Bearer("s3cret-token".into());
        let b = JmapAuth::Basic { user: "me".into(), password: "s3cret-pw".into() };
        assert!(!format!("{a:?}").contains("s3cret"));
        assert!(!format!("{b:?}").contains("s3cret"));
    }

    #[test]
    fn discovery_base_takes_the_domain() {
        assert_eq!(discovery_base("me@fastmail.com").unwrap(), "https://fastmail.com");
        assert!(discovery_base("no-at-sign").is_err());
        assert!(discovery_base("me@").is_err());
    }
}
