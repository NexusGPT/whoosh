//! The token endpoint: the code exchange (RFC 6749 4.1.3 with RFC 7636's verifier) and the refresh (RFC 6749 6).
//! A public client: no client secret is ever sent.
use contracts::CoreError;
use serde_json::Value;

/// POSTs a form to a token endpoint and returns the response body. `Err` only when no answer came; a provider's error
/// answer comes back as its body (status 400) and is read below.
pub trait TokenEndpoint: Send + Sync {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, CoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSet {
    pub access_token: String,
    /// None when the provider did not rotate it (keep the one in the Keychain)
    pub refresh_token: Option<String>,
    pub expires_at_ms: i64,
}

fn parse(body: &str, now_ms: i64) -> Result<TokenSet, CoreError> {
    let v: Value = serde_json::from_str(body).map_err(|e| CoreError::Invalid { field: "token".into(), reason_words: format!("the sign-in server answered something that is not JSON: {e}") })?;
    if let Some(err) = v.get("error").and_then(Value::as_str) {
        let words = v.get("error_description").and_then(Value::as_str).unwrap_or(err);
        return Err(CoreError::Invalid { field: "token".into(), reason_words: format!("the sign-in server refused: {words}") });
    }
    let access = v.get("access_token").and_then(Value::as_str)
        .ok_or_else(|| CoreError::Invalid { field: "token".into(), reason_words: "the sign-in server sent no access token".into() })?;
    let expires_in = v.get("expires_in").and_then(Value::as_i64).unwrap_or(3600);
    Ok(TokenSet { access_token: access.to_string(), refresh_token: v.get("refresh_token").and_then(Value::as_str).map(str::to_string), expires_at_ms: now_ms + expires_in * 1000 })
}

/// The authorization code for tokens.
pub fn exchange_code(ep: &dyn TokenEndpoint, token_url: &str, client_id: &str, code: &str, redirect_uri: &str, verifier: &str, now_ms: i64) -> Result<TokenSet, CoreError> {
    let body = ep.post_form(token_url, &[("grant_type", "authorization_code"), ("client_id", client_id), ("code", code), ("redirect_uri", redirect_uri), ("code_verifier", verifier)])?;
    parse(&body, now_ms)
}

/// A refresh token for a new access token (and, with Microsoft, a rotated refresh token).
pub fn refresh_access(ep: &dyn TokenEndpoint, token_url: &str, client_id: &str, refresh_token: &str, now_ms: i64) -> Result<TokenSet, CoreError> {
    let body = ep.post_form(token_url, &[("grant_type", "refresh_token"), ("client_id", client_id), ("refresh_token", refresh_token)])?;
    parse(&body, now_ms)
}

/// The real endpoint over HTTPS (feature `http`).
#[cfg(feature = "http")]
pub struct HttpsTokenEndpoint;

#[cfg(feature = "http")]
impl TokenEndpoint for HttpsTokenEndpoint {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, CoreError> {
        netguard::outbound_allowed_url(url)?;   // engine-act-r6 round 5
        let c = reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).build().map_err(|e| CoreError::Unexpected { reason_words: format!("the sign-in connection could not start: {e}") })?;
        let r = c.post(url).header("Content-Type", "application/x-www-form-urlencoded").body(crate::url::form_encode(form)).send()
            .map_err(|e| CoreError::Action { failure: contracts::ActionFailed { account_name: "Sign-in".into(), reason_words: format!("the sign-in server could not be reached: {e}"), retryable: true, code: None } })?;
        r.text().map_err(|e| CoreError::Unexpected { reason_words: format!("the sign-in server's answer could not be read: {e}") })
    }
}
