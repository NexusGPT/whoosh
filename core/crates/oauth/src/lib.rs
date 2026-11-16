//! Bring-your-own OAuth2 sign-in (contract delta 14). The person registers an app in their own Microsoft or Google admin
//! and pastes its client ID; this crate runs the native-app flow with it: PKCE (RFC 7636) and a loopback redirect on
//! 127.0.0.1 (RFC 8252), the code exchange, the refresh before expiry, the refresh token in the Keychain. There is no
//! client secret and no vendor client ID anywhere in this crate.
mod flow;
mod loopback;
mod pkce;
mod refresh;
mod token;
mod url;

pub use flow::{Authorities, OpenBrowser, SignIns, BROWSER_CODE, SINGLE_TENANT_REFUSAL};
pub use loopback::{redirect_with, Loopback, LoopbackAnswer};
pub use pkce::{random_url_safe, Pkce};
pub use refresh::RefreshingToken;
pub use token::{exchange_code, refresh_access, TokenEndpoint, TokenSet};
pub use url::{authorize_url, form_encode, percent_encode};

#[cfg(feature = "http")]
pub use token::HttpsTokenEndpoint;

/// The Keychain names of one account's OAuth items (the secrets crate's AccountCredential kind).
pub fn client_secret_name(draft: &str) -> String { format!("{draft}:oauth-client") }
pub fn refresh_secret_name(draft: &str) -> String { format!("{draft}:oauth-refresh") }
/// The token URL of the authority the sign-in used (not a secret; kept beside the client ID so a launch needs no lookup).
pub fn authority_secret_name(draft: &str) -> String { format!("{draft}:oauth-authority") }

/// How long a sign-in waits for the browser's answer: OAUTH_LOOPBACK_TIMEOUT_S (docs/CONSTANTS.md).
pub fn ledger_timeout() -> std::time::Duration { std::time::Duration::from_secs(contracts::constants::OAUTH_LOOPBACK_TIMEOUT_S) }

/// How long before its expiry an access token is refreshed: OAUTH_REFRESH_MARGIN_S (docs/CONSTANTS.md).
pub fn ledger_margin_ms() -> i64 { (contracts::constants::OAUTH_REFRESH_MARGIN_S * 1000) as i64 }
