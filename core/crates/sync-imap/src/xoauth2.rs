//! SASL XOAUTH2 (Microsoft IMAP OAuth, Gmail): base64("user=" + user + "^Aauth=Bearer " + token + "^A^A").
use base64::{engine::general_purpose::STANDARD, Engine as _};

/// The decoded client response, which `async-imap` base64-encodes itself.
pub(crate) fn xoauth2_raw(user: &str, token: &str) -> String {
    contracts::xoauth2_sasl(user, token)   // delta 14: the contract's one form, shared with the send crate's XOAUTH2
}

/// The base64 form, for tests and for servers that take it as an initial response.
pub fn xoauth2_initial_response(user: &str, token: &str) -> String {
    STANDARD.encode(xoauth2_raw(user, token))
}

pub(crate) struct XOAuth2 {
    pub raw: String,
}

impl async_imap::Authenticator for XOAuth2 {
    type Response = String;
    /// A challenge after the first response is the server's JSON error; the answer is empty
    /// so the server ends the exchange with its tagged NO, which carries the words we show.
    fn process(&mut self, challenge: &[u8]) -> Self::Response {
        if challenge.is_empty() {
            self.raw.clone()
        } else {
            String::new()
        }
    }
}

/// Contracts delta 14: the bring-your-own OAuth2 sign-in as the IMAP token source. The token is refreshed before it
/// expires (`oauth::RefreshingToken`, OAUTH_REFRESH_MARGIN_S); the blocking refresh runs off the async thread.
pub struct OAuthTokenSource(pub std::sync::Arc<oauth::RefreshingToken>);

#[async_trait::async_trait]
impl crate::adapter::TokenSource for OAuthTokenSource {
    async fn access_token(&self) -> Result<String, String> {
        let t = self.0.clone();
        tokio::task::spawn_blocking(move || t.access_token()).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod d14_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use contracts::{Clock, CoreError};
    use oauth::{RefreshingToken, TokenEndpoint, TokenSet};
    use secrets::{MemoryKeychain, Namespace, Secret, SecretKind, StoreSecret};

    struct Fake;
    impl TokenEndpoint for Fake {
        fn post_form(&self, _: &str, _: &[(&str, &str)]) -> Result<String, CoreError> { Ok(r#"{"access_token":"at-new","expires_in":3600}"#.into()) }
    }
    struct At(i64);
    impl Clock for At { fn now_ms(&self) -> i64 { self.0 } fn utc_offset_s(&self) -> i32 { 0 } }

    #[tokio::test]
    async fn the_oauth_token_source_refreshes_a_token_about_to_expire() {
        use crate::adapter::TokenSource;
        let kc = Arc::new(MemoryKeychain::new(Namespace::rig()));
        let r = StoreSecret::execute(&*kc, SecretKind::AccountCredential, "a@x.example:oauth-refresh", &Secret::new(b"rt".to_vec())).unwrap();
        let t = RefreshingToken { endpoint: Arc::new(Fake), token_url: "http://127.0.0.1:1/token".into(), client_id: "id".into(), refresh_ref: r,
            refresh_name: "a@x.example:oauth-refresh".into(), read: kc.clone(), store: kc, clock: Arc::new(At(1_000)), margin_ms: 300_000,
            cached: Mutex::new(Some(TokenSet { access_token: "at-old".into(), refresh_token: None, expires_at_ms: 1_000 + 60_000 })) };
        assert_eq!(OAuthTokenSource(Arc::new(t)).access_token().await.unwrap(), "at-new", "60 s left is inside the margin");
        assert_eq!(xoauth2_raw("a@x.example", "at-new"), "user=a@x.example\x01auth=Bearer at-new\x01\x01");
    }
}
