//! The sign-in's runtime mutants (slice 2b), read only by the hidden test copy (`WHOOSH_CORE_MUTANT`) and handed in by the
//! tests: each removes one step of the sign-in, so the test and the check that prove that step are seen red on it.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::{Secret, SecretKind, StoreSecret};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthMutant {
    None,
    /// `oauth-never-waiting`: Start returns without opening the sign-in, so the state never reads Waiting.
    NeverWaiting,
    /// `oauth-never-exchanges`: the loopback's code is never sent to the token endpoint.
    NeverExchanges,
    /// `oauth-refresh-not-stored`: the refresh token is dropped instead of kept in the Keychain.
    RefreshNotStored,
    /// `oauth-common-tenant`: after an AADSTS50194 refusal the sign-in goes to `common` again, not the address's tenant.
    CommonTenant,
    /// `oauth-no-xoauth2-imap`: IMAP is given the access token as a password (LOGIN), not by XOAUTH2.
    NoXoauth2OnImap,
    /// `oauth-add-refuses`: Add account still refuses an OAuth2 account (the slice 1 line).
    AddRefusesOauth2,
}

impl OAuthMutant {
    pub fn from_name(name: &str) -> Self {
        match name {
            "oauth-never-waiting" => Self::NeverWaiting, "oauth-never-exchanges" => Self::NeverExchanges, "oauth-refresh-not-stored" => Self::RefreshNotStored,
            "oauth-common-tenant" => Self::CommonTenant, "oauth-no-xoauth2-imap" => Self::NoXoauth2OnImap, "oauth-add-refuses" => Self::AddRefusesOauth2,
            _ => Self::None,
        }
    }

    /// Read by the rig copy only (the caller checks `rig`).
    pub fn from_env() -> Self { std::env::var("WHOOSH_CORE_MUTANT").map(|v| Self::from_name(v.trim())).unwrap_or(Self::None) }

    pub fn store(self, inner: Arc<dyn StoreSecret>) -> Arc<dyn StoreSecret> {
        if self == Self::RefreshNotStored { Arc::new(DropsRefresh(inner)) } else { inner }
    }

    pub fn endpoint(self, inner: Arc<dyn oauth::TokenEndpoint>) -> Arc<dyn oauth::TokenEndpoint> {
        if self == Self::NeverExchanges { Arc::new(NeverExchange(inner)) } else { inner }
    }
}

struct DropsRefresh(Arc<dyn StoreSecret>);
impl StoreSecret for DropsRefresh {
    fn execute(&self, kind: SecretKind, name: &str, secret: &Secret) -> Result<KeychainRef, CoreError> {
        if name.ends_with(":oauth-refresh") { return Ok(KeychainRef { service: "mutant-dropped".into(), account: name.into() }); }
        self.0.execute(kind, name, secret)
    }
}

struct NeverExchange(Arc<dyn oauth::TokenEndpoint>);
impl oauth::TokenEndpoint for NeverExchange {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, CoreError> {
        if form.iter().any(|(k, v)| *k == "grant_type" && *v == "authorization_code") {
            return Err(CoreError::Unexpected { reason_words: "the code was not exchanged (mutant)".into() });
        }
        self.0.post_form(url, form)
    }
}
