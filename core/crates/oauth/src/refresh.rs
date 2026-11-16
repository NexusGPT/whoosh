//! An access token that is refreshed before it expires: within `margin_ms` of the expiry (OAUTH_REFRESH_MARGIN_S) the
//! next read asks the token endpoint with the refresh token from the Keychain, and a rotated refresh token replaces it.
use std::sync::{Arc, Mutex};

use contracts::{Clock, CoreError, KeychainRef};
use secrets::{ReadSecret, Secret, SecretKind, StoreSecret};

use crate::token::{refresh_access, TokenEndpoint, TokenSet};

pub struct RefreshingToken {
    pub endpoint: Arc<dyn TokenEndpoint>,
    pub token_url: String,
    pub client_id: String,
    /// the refresh token's Keychain item
    pub refresh_ref: KeychainRef,
    /// the Keychain item's name, to write a rotated refresh token under
    pub refresh_name: String,
    pub read: Arc<dyn ReadSecret>,
    pub store: Arc<dyn StoreSecret>,
    pub clock: Arc<dyn Clock>,
    pub margin_ms: i64,
    pub cached: Mutex<Option<TokenSet>>,
}

impl RefreshingToken {
    /// The bearer token for XOAUTH2: the cached one while it has more than the margin left, else a refreshed one.
    pub fn access_token(&self) -> Result<String, CoreError> {
        let now = self.clock.now_ms();
        let mut cached = self.cached.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = cached.as_ref() {
            if t.expires_at_ms - now > self.margin_ms { return Ok(t.access_token.clone()); }
        }
        let secret = self.read.execute(&self.refresh_ref)?;
        let refresh = String::from_utf8_lossy(secret.expose()).into_owned();
        let fresh = refresh_access(self.endpoint.as_ref(), &self.token_url, &self.client_id, &refresh, now)?;
        if let Some(rotated) = &fresh.refresh_token {
            self.store.execute(SecretKind::AccountCredential, &self.refresh_name, &Secret::new(rotated.as_bytes().to_vec()))?;
        }
        let token = fresh.access_token.clone();
        *cached = Some(fresh);
        Ok(token)
    }
}
