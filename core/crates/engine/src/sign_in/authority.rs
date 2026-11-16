//! The adapter to the provider's sign-in authority over HTTP (reqwest blocking, ring TLS): the tenant's OpenID
//! configuration, the token endpoint, and the check's browser stand-in. HTTPS only; clear HTTP to this Mac's own address
//! only (the local fake authority of the tests and the hidden check). No redirect is followed by the client itself.
use std::sync::OnceLock;
use std::time::Duration;

use contracts::{oauth_endpoints, oauth_endpoints_at, oauth_tenant_check_url, oauth_tenant_of, CoreError, OAuthAuthority, OAuthProvider, MICROSOFT_ANY_ORGANISATION};

use crate::consts::OAUTH_HTTP_TIMEOUT_MS;

pub struct AuthorityHttp { client: OnceLock<Result<reqwest::blocking::Client, String>> }

impl Default for AuthorityHttp { fn default() -> Self { Self::new() } }

/// A transport failure in words. A reqwest error is passed `without_url()`: a URL can carry a code or a state.
fn unreachable(url_host: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::Action { failure: contracts::ActionFailed { account_name: "Sign-in".into(), reason_words: format!("the sign-in server {url_host} could not be reached: {e}"), retryable: true, code: None } }
}

/// The host of a URL, for words (never the query: it can carry a code).
pub fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split(['/', '?']).next().unwrap_or("").to_string()
}

/// HTTPS anywhere; clear HTTP to 127.0.0.1, ::1 or localhost only.
pub fn allowed(url: &str) -> bool {
    if url.starts_with("https://") { return true; }
    if !url.starts_with("http://") { return false; }
    let name = netguard::host_of_url(url);   // rig-net-rust: the host the client dials (the guard's one parser), not a hand split
    matches!(name.as_str(), "127.0.0.1" | "localhost" | "[::1]")
}

impl AuthorityHttp {
    /// The client is built on first use (a launch with no sign-in never builds one).
    pub fn new() -> Self { AuthorityHttp { client: OnceLock::new() } }

    fn client(&self, url: &str) -> Result<&reqwest::blocking::Client, CoreError> {
        netguard::outbound_allowed_url(url)?;   // engine-act-r6 round 5: the sign-in authority of a rig copy is this Mac only
        if !allowed(url) { return Err(CoreError::Invalid { field: "authority".into(), reason_words: format!("the sign-in server {} is not reached over HTTPS, so it was not asked", host_of(url)) }); }
        let c = self.client.get_or_init(|| {
            // ring is the TLS provider (Cargo.toml: rustls-no-provider); an Err means one is installed already, which is fine
            let _ = rustls::crypto::ring::default_provider().install_default();
            reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_millis(OAUTH_HTTP_TIMEOUT_MS)).build().map_err(|e| format!("{e:?}"))
        });
        c.as_ref().map_err(|e| CoreError::Unexpected { reason_words: format!("the app could not start its sign-in connection: {e}") })
    }

    /// The status of a GET (the tenant's OpenID configuration).
    pub fn get_status(&self, url: &str) -> Result<u16, CoreError> {
        let r = self.client(url)?.get(url).send().map_err(|e| unreachable(&host_of(url), e.without_url()))?;
        Ok(r.status().as_u16())
    }

    /// A GET without following its redirect: the `Location` it answered, when it answered one.
    pub fn get_location(&self, url: &str) -> Result<Option<String>, CoreError> {
        let r = self.client(url)?.get(url).send().map_err(|e| unreachable(&host_of(url), e.without_url()))?;
        Ok(r.headers().get(reqwest::header::LOCATION).and_then(|v| v.to_str().ok()).map(str::to_string))
    }
}

impl oauth::TokenEndpoint for AuthorityHttp {
    /// The provider's answer body, error answers included (status 400 carries the provider's words, read by the oauth crate).
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<String, CoreError> {
        let r = self.client(url)?.post(url).header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded").body(oauth::form_encode(form)).send()
            .map_err(|e| { let e = e.without_url(); eprintln!("sign-in: the token server {} did not answer: {e}", host_of(url)); unreachable(&host_of(url), e) })?;
        let status = r.status().as_u16();
        let body = r.text().map_err(|e| CoreError::Unexpected { reason_words: format!("the sign-in server's answer could not be read: {}", e.without_url()) })?;
        if status >= 400 && !body.trim_start().starts_with('{') {
            // not the provider's JSON error: keep its real status and the start of its body in the words (R#20)
            let head: String = body.chars().take(200).collect();
            return Err(unreachable(&host_of(url), format!("it answered {status}: {head}")));
        }
        Ok(body)
    }
}

/// Where an address signs in: Microsoft at `common` first (the shipped default ID is a multi-tenant registration); after an
/// AADSTS50194 refusal (a person's own single-tenant ID), at the tenant of the address's domain, asked of the authority
/// first (a domain that is not a tenant answers 400, and the sign-in then goes to `organizations`). Google at its one authority.
pub struct TenantAuthorities {
    pub http: std::sync::Arc<AuthorityHttp>,
    /// `contracts::MICROSOFT_LOGIN_BASE` in the app; a local fake authority in the tests and the hidden check
    pub login_base: String,
    /// the mutant `oauth-common-tenant`: the fallback goes to `common` again, not the address's tenant
    pub common: bool,
}

impl TenantAuthorities {
    /// The tenant to sign in at, and why, for one address.
    pub fn tenant(&self, address: &str) -> String {
        if self.common { return "common".into(); }
        let Some(domain) = oauth_tenant_of(address) else { return MICROSOFT_ANY_ORGANISATION.into() };
        match self.http.get_status(&oauth_tenant_check_url(&self.login_base, &domain)) {
            Ok(400) => MICROSOFT_ANY_ORGANISATION.into(),
            Ok(_) => domain,
            Err(e) => { eprintln!("sign-in: could not ask whether {domain} is a tenant ({e}); signing in at {domain}"); domain }
        }
    }
}

impl oauth::Authorities for TenantAuthorities {
    fn first(&self, provider: OAuthProvider) -> OAuthAuthority { oauth_endpoints_at(provider, &self.login_base, "common") }

    fn fallback(&self, provider: OAuthProvider, address: &str) -> Result<OAuthAuthority, CoreError> {
        Ok(match provider {
            OAuthProvider::Microsoft => oauth_endpoints_at(provider, &self.login_base, &self.tenant(address)),
            OAuthProvider::Google => oauth_endpoints(provider),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::allowed;

    // rig-net-rust: clear HTTP only when the host the client dials is this Mac. Mutant "authority-hand-host": `allowed` back to the split
    // on '/', '?' and the last ':' (red: "127.0.0.1:1@evil.example" reads as 127.0.0.1, so clear HTTP to evil.example was allowed).
    #[test]
    fn clear_http_only_to_the_host_the_client_dials_on_this_mac() {
        for here in ["http://127.0.0.1:8/x", "http://[::1]:8/x", "http://localhost/x", "https://evil.example/x"] { assert!(allowed(here), "{here}"); }
        for away in ["http://127.0.0.1:1@evil.example/x", "http://evil.example\\@127.0.0.1/x", "http://evil.example/x", "ftp://127.0.0.1/x"] { assert!(!allowed(away), "{away}"); }
    }
}
