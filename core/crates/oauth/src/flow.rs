//! The sign-in of one account being added, from the pasted client ID to the refresh token in the Keychain.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use contracts::{check_oauth_client_id, oauth_endpoints, ActionFailed, Clock, CoreError, KeychainRef, OAuthAuthority, OAuthProvider, OAuthSignInState};
use secrets::{ReadSecret, Secret, SecretKind, StoreSecret};

use crate::loopback::{Loopback, LoopbackAnswer};
use crate::pkce::{random_url_safe, Pkce};
use crate::token::{exchange_code, TokenEndpoint};
use crate::url::authorize_url;
use crate::{authority_secret_name, client_secret_name, refresh_secret_name};

/// Opens a URL in the person's browser (the app's NSWorkspace call; a test follows the redirect itself).
pub trait OpenBrowser: Send + Sync {
    fn open(&self, url: &str) -> Result<(), CoreError>;
}

/// Where an address signs in when the default authority refuses its client ID (slice 2b): a person's OWN registration made
/// for "this organizational directory only" is refused at Microsoft's `common` authority with AADSTS50194; the sign-in then
/// runs once more at the authority `fallback` answers (the tenant of the address's domain, or `organizations`).
pub trait Authorities: Send + Sync {
    /// The authority a sign-in starts at (Microsoft: `common`, right for a multi-tenant registration such as the default).
    fn first(&self, provider: OAuthProvider) -> OAuthAuthority;
    fn fallback(&self, provider: OAuthProvider, address: &str) -> Result<OAuthAuthority, CoreError>;
}

/// Microsoft's refusal of a single-tenant registration at a multi-tenant authority.
pub const SINGLE_TENANT_REFUSAL: &str = "AADSTS50194";

/// The code a browser that would not open carries (the shared refusal list of slice 2b).
pub const BROWSER_CODE: &str = "oauth.browser";

struct Entry { provider: OAuthProvider, state: OAuthSignInState, authority: Option<OAuthAuthority> }

pub struct SignIns {
    pub store: Arc<dyn StoreSecret>,
    pub read: Arc<dyn ReadSecret>,
    pub endpoint: Arc<dyn TokenEndpoint>,
    pub browser: Arc<dyn OpenBrowser>,
    pub clock: Arc<dyn Clock>,
    /// how long the loopback waits for the browser's answer (OAUTH_LOOPBACK_TIMEOUT_S)
    pub timeout: Duration,
    /// the provider endpoints; the contract's table in the app, a local fake authority in tests
    pub endpoints: fn(OAuthProvider) -> OAuthAuthority,
    /// the first authority and the fallback per address after an AADSTS50194 refusal; None = `endpoints`, no fallback
    pub authorities: Option<Arc<dyn Authorities>>,
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    /// the Keychain references the store answered, by item name (the client ID and the refresh token)
    refs: Arc<Mutex<HashMap<String, KeychainRef>>>,
}

impl SignIns {
    pub fn new(store: Arc<dyn StoreSecret>, read: Arc<dyn ReadSecret>, endpoint: Arc<dyn TokenEndpoint>, browser: Arc<dyn OpenBrowser>, clock: Arc<dyn Clock>, timeout: Duration) -> SignIns {
        SignIns { store, read, endpoint, browser, clock, timeout, endpoints: oauth_endpoints, authorities: None, entries: Arc::new(Mutex::new(HashMap::new())), refs: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> { self.entries.lock().unwrap_or_else(|e| e.into_inner()) }

    /// SetOAuthClient: the client ID is checked, then kept in the Keychain; a sign-in made with an older ID is forgotten.
    pub fn set_client(&self, draft: &str, provider: OAuthProvider, client_id: &str) -> Result<(), CoreError> {
        let draft = contracts::oauth_account_draft_id(draft.to_string())?;
        let id = check_oauth_client_id(provider, client_id.to_string())?;
        let name = client_secret_name(&draft);
        let r = self.store.execute(SecretKind::AccountCredential, &name, &Secret::new(id.into_bytes()))?;
        self.refs.lock().unwrap_or_else(|e| e.into_inner()).insert(name, r);
        self.lock().insert(draft, Entry { provider, state: OAuthSignInState::Ready, authority: None });
        Ok(())
    }

    pub fn state(&self, draft: &str) -> OAuthSignInState {
        let Ok(draft) = contracts::oauth_account_draft_id(draft.to_string()) else { return OAuthSignInState::NeedsClientId };
        self.lock().get(&draft).map(|e| e.state.clone()).unwrap_or(OAuthSignInState::NeedsClientId)
    }

    /// StartOAuthSignIn: opens the browser on the provider's sign-in and returns at once; a thread waits for the answer,
    /// exchanges the code and keeps the refresh token. `state(draft)` reads Waiting, then Done or Refused. An AADSTS50194
    /// refusal (a single-tenant client ID at the default authority) is retried once at `authorities`' fallback.
    pub fn start(&self, draft: &str) -> Result<(), CoreError> {
        let draft = contracts::oauth_account_draft_id(draft.to_string())?;
        let provider = match self.lock().get(&draft) {
            Some(e) if !matches!(e.state, OAuthSignInState::NeedsClientId) => e.provider,
            _ => return Err(CoreError::Invalid { field: "client_id".into(), reason_words: "paste the client ID of the app you registered first".into() }),
        };
        let client = self.read_named(&client_secret_name(&draft))?;
        let a = Attempt {
            entries: self.entries.clone(), refs: self.refs.clone(), endpoint: self.endpoint.clone(), store: self.store.clone(), browser: self.browser.clone(),
            clock: self.clock.clone(), timeout: self.timeout, authorities: self.authorities.clone(), provider, draft, client,
        };
        let e = match &self.authorities { Some(a) => a.first(provider), None => (self.endpoints)(provider) };
        let opened = a.open(&e)?;
        std::thread::spawn(move || a.finish(e, opened, false));
        Ok(())
    }

    /// The authority the last sign-in of `draft` used (its token URL refreshes the access token).
    pub fn authority_of(&self, draft: &str) -> Option<OAuthAuthority> {
        let draft = contracts::oauth_account_draft_id(draft.to_string()).ok()?;
        self.lock().get(&draft).and_then(|e| e.authority.clone())
    }

    /// The provider `draft` signs in with (None before its client ID is set).
    pub fn provider_of(&self, draft: &str) -> Option<OAuthProvider> {
        let draft = contracts::oauth_account_draft_id(draft.to_string()).ok()?;
        self.lock().get(&draft).map(|e| e.provider)
    }

    /// The client ID set for `draft`, read back from the Keychain.
    pub fn client_id_of(&self, draft: &str) -> Result<String, CoreError> {
        let draft = contracts::oauth_account_draft_id(draft.to_string())?;
        self.read_named(&client_secret_name(&draft))
    }

    fn read_named(&self, name: &str) -> Result<String, CoreError> {
        let reference = self.reference_of(name)?;
        Ok(String::from_utf8_lossy(self.read.execute(&reference)?.expose()).into_owned())
    }

    /// The Keychain reference of an item this sign-in wrote (the client ID, or the refresh token once Done).
    pub fn reference_of(&self, name: &str) -> Result<contracts::KeychainRef, CoreError> {
        self.refs.lock().unwrap_or_else(|e| e.into_inner()).get(name).cloned()
            .ok_or_else(|| CoreError::NotFound { kind: "sign-in item".into(), id: name.into() })
    }
}

/// One sign-in being run: what its thread needs, cloned out of `SignIns`.
struct Attempt {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    refs: Arc<Mutex<HashMap<String, KeychainRef>>>,
    endpoint: Arc<dyn TokenEndpoint>,
    store: Arc<dyn StoreSecret>,
    browser: Arc<dyn OpenBrowser>,
    clock: Arc<dyn Clock>,
    timeout: Duration,
    authorities: Option<Arc<dyn Authorities>>,
    provider: OAuthProvider,
    draft: String,
    client: String,
}

/// An opened sign-in page: the loopback that waits for its answer, its PKCE pair and its state.
struct Opened { lb: Loopback, pkce: Pkce, state: String }

impl Attempt {
    fn set(&self, state: OAuthSignInState, e: &OAuthAuthority) {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).insert(self.draft.clone(), Entry { provider: self.provider, state, authority: Some(e.clone()) });
    }

    /// Keeps the authority beside the client ID (a later launch refreshes there without asking again), listens on the
    /// loopback, reads Waiting and opens the browser. A browser that will not open: Refused, and an `oauth.browser` error.
    fn open(&self, e: &OAuthAuthority) -> Result<Opened, CoreError> {
        let r = self.store.execute(SecretKind::AccountCredential, &authority_secret_name(&self.draft), &Secret::new(e.token_url.clone().into_bytes()))?;
        self.refs.lock().unwrap_or_else(|e| e.into_inner()).insert(authority_secret_name(&self.draft), r);
        let pkce = Pkce::new()?;
        let state = random_url_safe(16)?;
        let lb = Loopback::bind().map_err(|err| CoreError::Unexpected { reason_words: format!("the sign-in could not listen on this Mac: {err}") })?;
        let url = authorize_url(self.provider, e, &self.client, &lb.redirect_uri, &state, &pkce.challenge, &self.draft);
        self.set(OAuthSignInState::Waiting { authorize_url: url.clone() }, e);
        if let Err(err) = self.browser.open(&url) {
            let words = format!("the browser could not open the sign-in: {err}");
            self.set(OAuthSignInState::Refused { words: words.clone() }, e);
            return Err(CoreError::Action { failure: ActionFailed { account_name: self.draft.clone(), reason_words: words, retryable: true, code: Some(BROWSER_CODE.into()) } });
        }
        Ok(Opened { lb, pkce, state })
    }

    /// Waits for the browser's answer, exchanges the code and keeps the refresh token; on AADSTS50194, once, the same at
    /// the fallback authority.
    fn finish(self, e: OAuthAuthority, o: Opened, retried: bool) {
        let done = match o.lb.wait(&o.state, self.timeout) {
            LoopbackAnswer::Refused(words) => OAuthSignInState::Refused { words },
            LoopbackAnswer::Code(code) => match exchange_code(self.endpoint.as_ref(), &e.token_url, &self.client, &code, &o.lb.redirect_uri, &o.pkce.verifier, self.clock.now_ms()) {
                Err(err) => OAuthSignInState::Refused { words: err.to_string() },
                Ok(t) => match t.refresh_token {
                    None => OAuthSignInState::Refused { words: "the sign-in server sent no refresh token, so the account could not stay signed in".into() },
                    Some(r) => match self.store.execute(SecretKind::AccountCredential, &refresh_secret_name(&self.draft), &Secret::new(r.into_bytes())) {
                        Ok(reference) => { self.refs.lock().unwrap_or_else(|e| e.into_inner()).insert(refresh_secret_name(&self.draft), reference); OAuthSignInState::Done }
                        Err(err) => OAuthSignInState::Refused { words: format!("the sign-in worked but the Keychain refused to keep it: {err}") },
                    },
                },
            },
        };
        let single_tenant = matches!(&done, OAuthSignInState::Refused { words } if words.contains(SINGLE_TENANT_REFUSAL));
        if let (true, false, Some(auth)) = (single_tenant, retried, self.authorities.clone()) {
            match auth.fallback(self.provider, &self.draft) {
                Ok(next) => match self.open(&next) {
                    Ok(opened) => return self.finish(next, opened, true),
                    Err(_) => return,   // `open` has set Refused with the browser's words
                },
                Err(err) => return self.set(OAuthSignInState::Refused { words: err.to_string() }, &e),
            }
        }
        self.set(done, &e);
    }
}
