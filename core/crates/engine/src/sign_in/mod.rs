//! Slice 2b: the browser sign-in of a Microsoft 365 or Google Workspace address on the real core (contract delta 14).
//! `SetOAuthClient` keeps the pasted client ID in the Keychain; `StartOAuthSignIn` opens the provider's sign-in at
//! Microsoft's `common` authority (then, for a single-tenant ID refused there with AADSTS50194, at the tenant of the
//! address's domain) and waits on the loopback; `AddAccountConfigured` with oauth2 adds the account on that
//! sign-in and syncs it by XOAUTH2 (`token`). Every refusal on this path carries a code from `codes`.
pub mod authority;
pub mod browser;
pub mod codes;
pub mod mutant;
pub mod token;

use std::sync::Arc;

use contracts::{CommandOutcome, CoreError, OAuthProvider, OAuthSignInState, MICROSOFT_LOGIN_BASE};
use secrets::{ReadSecret, StoreSecret};

use crate::rust_core::Keyring;
use crate::sync::SystemClock;
pub use mutant::OAuthMutant;

/// What the sign-in is built from: where Microsoft signs in, who opens the page, and the runtime mutant (rig only).
pub struct SignInParts {
    pub login_base: String,
    pub browser: Arc<dyn oauth::OpenBrowser>,
    pub mutant: OAuthMutant,
}

impl SignInParts {
    /// The real authority, no browser and no mutant: a core built for a test or a tool never opens a window.
    pub fn quiet() -> Self { SignInParts { login_base: MICROSOFT_LOGIN_BASE.into(), browser: Arc::new(browser::NoBrowser), mutant: OAuthMutant::None } }

    /// The app's own: the person's browser at the real authority. The hidden test copy (`rig`) opens no browser; with
    /// `WHOOSH_CORE_CHECK_AUTHORITY=<port>` it signs in at the check's local authority, playing the browser itself.
    pub fn production(rig: bool, http: &Arc<authority::AuthorityHttp>) -> Self {
        if !rig { return SignInParts { login_base: MICROSOFT_LOGIN_BASE.into(), browser: Arc::new(browser::SystemBrowser), mutant: OAuthMutant::None }; }
        let mutant = OAuthMutant::from_env();
        match check_authority_port() {
            Some(port) => SignInParts { login_base: format!("http://127.0.0.1:{port}"), browser: Arc::new(browser::Follower { http: http.clone() }), mutant },
            None => SignInParts { mutant, ..SignInParts::quiet() },
        }
    }
}

/// The hidden check's local authority (`WHOOSH_CORE_CHECK_AUTHORITY=<port>`, read only by the rig copy).
pub fn check_authority_port() -> Option<u16> { std::env::var("WHOOSH_CORE_CHECK_AUTHORITY").ok()?.trim().parse().ok() }

/// The core's sign-ins and what an OAuth2 account's token is refreshed through.
pub struct SignIn {
    pub sign_ins: oauth::SignIns,
    pub endpoint: Arc<dyn oauth::TokenEndpoint>,
    pub mutant: OAuthMutant,
}

impl SignIn {
    pub fn new(keychain: Arc<dyn Keyring>, http: Arc<authority::AuthorityHttp>, parts: SignInParts) -> Self {
        let mutant = parts.mutant;
        let read: Arc<dyn ReadSecret> = keychain.clone();
        let store: Arc<dyn StoreSecret> = keychain;
        let store = mutant.store(store);
        let endpoint: Arc<dyn oauth::TokenEndpoint> = http.clone();
        let exchange = mutant.endpoint(endpoint.clone());
        let mut sign_ins = oauth::SignIns::new(store, read, exchange, parts.browser, Arc::new(SystemClock), oauth::ledger_timeout());
        sign_ins.authorities = Some(Arc::new(authority::TenantAuthorities { http, login_base: parts.login_base, common: mutant == OAuthMutant::CommonTenant }));
        SignIn { sign_ins, endpoint, mutant }
    }

    /// `SetOAuthClient`: refused with `oauth.no_client_id` when the paste is not a client ID.
    pub fn set_client(&self, draft: &str, provider: OAuthProvider, client_id: &str) -> Result<CommandOutcome, CoreError> {
        // a malformed paste is "no client ID"; any other failure (the Keychain refusing the write, say) keeps its own words under its own code,
        // so the sheet never tells the person to paste an ID they already pasted (owner's screenshot, main47, 17:30)
        self.sign_ins.set_client(draft, provider, client_id).map_err(|e| match &e {
            CoreError::Invalid { field, .. } if field == "client_id" => codes::coded(e, codes::NO_CLIENT_ID, draft.trim()),
            _ => codes::coded(e, codes::KEYCHAIN, draft.trim()),
        })?;
        Ok(CommandOutcome { undo_entry_id: None, created_id: None })
    }

    /// `StartOAuthSignIn`: `oauth.no_client_id` before a client ID, `oauth.browser` when the page would not open.
    pub fn start(&self, draft: &str) -> Result<CommandOutcome, CoreError> {
        let done = CommandOutcome { undo_entry_id: None, created_id: None };
        if self.mutant == OAuthMutant::NeverWaiting { return Ok(done); }
        match self.sign_ins.start(draft) {
            Ok(()) => Ok(done),
            Err(e @ CoreError::Action { .. }) => Err(codes::coded(e, codes::BROWSER, draft.trim())),
            Err(e @ CoreError::Invalid { .. }) if matches!(&e, CoreError::Invalid { field, .. } if field == "client_id") => Err(codes::coded(e, codes::NO_CLIENT_ID, draft.trim())),
            Err(e) => { let code = codes::refused_code(&e.to_string()); Err(codes::coded(e, code, draft.trim())) }
        }
    }

    /// The sign-in's state. A refusal that says consent is required reads `oauth.consent_required: <words>`, so the sheet finds
    /// its code in the state read as it does in a refused call (the other refusals keep their words as they are).
    pub fn state(&self, draft: &str) -> OAuthSignInState {
        match self.sign_ins.state(draft) {
            OAuthSignInState::Refused { words } if codes::is_consent_required(&words) => OAuthSignInState::Refused { words: format!("{}: {words}", codes::CONSENT_REQUIRED) },
            other => other,
        }
    }
}
