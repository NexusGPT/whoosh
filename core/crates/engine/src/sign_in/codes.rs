//! The codes the engine's refusals carry beside their words (`ActionFailed.code`), the shared list of slice 2b. The Swift
//! sheet maps a code to the catalogue; the words stay the whole message.
use contracts::{ActionFailed, CoreError};

/// A command the local engine does not run yet.
pub const NOT_BUILT_YET: &str = "engine.not_built_yet";
/// No client ID set for this address, or the pasted one is not a client ID.
pub const NO_CLIENT_ID: &str = "oauth.no_client_id";
/// Add account asked for an OAuth2 account whose sign-in is not finished.
pub const SIGN_IN_NOT_DONE: &str = "oauth.sign_in_not_done";
/// The provider (or the token server) refused the sign-in.
pub const REFUSED: &str = "oauth.refused";
/// The sign-in is done but its refresh token is not in the Keychain.
pub const NO_REFRESH_TOKEN: &str = "oauth.no_refresh_token";
/// The browser could not open the provider's sign-in.
pub const BROWSER: &str = oauth::BROWSER_CODE;
/// The client ID is well formed but the Keychain refused to keep it (or to give it back): the Keychain's own words follow.
pub const KEYCHAIN: &str = "oauth.keychain";

/// Microsoft answered the sign-in with "consent required": the tenant lets no plain user approve the app, an admin must (A97).
pub const CONSENT_REQUIRED: &str = "oauth.consent_required";

/// True when the provider's words say consent is required: `error=consent_required` at the loopback, or AADSTS65001 (the
/// loopback's description or the token exchange's invalid_grant body).
pub fn is_consent_required(words: &str) -> bool {
    let w = words.to_ascii_lowercase();
    w.contains("aadsts65001") || w.contains("consent_required")
}

/// The code of a refused sign-in's words: `oauth.consent_required` for a consent refusal, `oauth.refused` for any other.
pub fn refused_code(words: &str) -> &'static str { if is_consent_required(words) { CONSENT_REQUIRED } else { REFUSED } }

/// A refusal with its code: the words of `e` as they are, the code set when `e` carries none.
pub fn coded(e: CoreError, code: &str, account_name: &str) -> CoreError {
    let (words, retryable, kept) = match e {
        CoreError::Action { failure } => (failure.reason_words, failure.retryable, failure.code),
        CoreError::Invalid { reason_words, .. } => (reason_words, false, None),
        CoreError::NotFound { kind, id } => (format!("no {kind} {id}"), false, None),
        CoreError::Unexpected { reason_words } => (reason_words, true, None),
        other => (other.to_string(), true, None),
    };
    CoreError::Action { failure: ActionFailed { account_name: account_name.to_string(), reason_words: words, retryable, code: Some(kept.unwrap_or_else(|| code.to_string())) } }
}

/// A refusal in words with its code.
pub fn refusal(words: &str, code: &str, account_name: &str) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: account_name.to_string(), reason_words: words.to_string(), retryable: false, code: Some(code.to_string()) } }
}
