//! Bring-your-own OAuth2 sign-in (contract delta 14). The owner's rule (PRD "Adding an account must be as easy as
//! Thunderbird", SPEC A77): Whoosh ships NO Microsoft or Google client ID and registers nothing with them. A person who
//! wants Microsoft 365 or Gmail sign-in registers an app in their OWN admin and pastes its client ID once; the ID goes to
//! the Keychain (the secrets crate), never to the store, a state path, a log line or the repository. There is no client
//! secret: the flow is a native-app authorization-code flow with PKCE and a loopback redirect (RFC 8252, RFC 7636).
//!
//! An account being added has no account id yet: its `account_draft_id` is the address being added, trimmed and lower
//! case (`oauth_account_draft_id`). Once the sign-in is `Done`, `AddAccountConfigured` with `config.auth_method` Oauth2
//! and an empty password saves the account on that sign-in.
use crate::{CoreError, MailProvider};

/// The two providers whose mail is reached by OAuth2 here.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OAuthProvider { Microsoft, Google }

/// Where a provider signs in and what it is asked for, with the public page each value comes from. Named `OAuthAuthority`, not
/// `OAuthEndpoints`: the sign-in lane's Swift engine (Sources/Features/SignIn/Engine/OAuthTypes.swift) already holds that name.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct OAuthAuthority {
    pub authorize_url: String,
    pub token_url: String,
    /// space-joined into the request's `scope`
    pub scopes: Vec<String>,
    /// the provider's public documentation of these values
    pub doc_url: String,
}

/// The one table of provider endpoints (delta 14). Values as the providers document them:
/// Microsoft: the v2.0 endpoints of the `common` authority and the IMAP/SMTP scopes of "Authenticate an IMAP, POP or SMTP
/// connection using OAuth"; Google: the installed-app endpoints and the full-mail scope of "OAuth 2.0 for iOS & Desktop Apps".
#[uniffi::export]
pub fn oauth_endpoints(provider: OAuthProvider) -> OAuthAuthority {
    match provider {
        OAuthProvider::Microsoft => OAuthAuthority {
            authorize_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".into(),
            token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token".into(),
            scopes: vec!["offline_access".into(), "https://outlook.office.com/IMAP.AccessAsUser.All".into(), "https://outlook.office.com/SMTP.Send".into()],
            doc_url: "https://learn.microsoft.com/en-us/exchange/client-developer/legacy-protocols/how-to-authenticate-an-imap-pop-smtp-application-by-using-oauth".into(),
        },
        OAuthProvider::Google => OAuthAuthority {
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            scopes: vec!["https://mail.google.com/".into()],
            doc_url: "https://developers.google.com/identity/protocols/oauth2/native-app".into(),
        },
    }
}

/// Microsoft's sign-in host. `oauth_endpoints_at` takes it as a parameter so a test can name a local fake authority instead.
pub const MICROSOFT_LOGIN_BASE: &str = "https://login.microsoftonline.com";
/// Microsoft's authority for a work or school account of any directory: used when the address's own domain is not a tenant.
pub const MICROSOFT_ANY_ORGANISATION: &str = "organizations";

/// The tenant an address signs in at: its domain, trimmed and lower case. None when it is not an address.
pub fn oauth_tenant_of(address: &str) -> Option<String> {
    oauth_account_draft_id(address.to_string()).ok().and_then(|a| a.split_once('@').map(|(_, d)| d.to_string()))
}

/// The endpoints of one provider at one tenant. Microsoft: `<login_base>/<tenant>/oauth2/v2.0/{authorize,token}` (a
/// registration made for "this organizational directory only" is refused at the `common` authority with AADSTS50194, so
/// the sign-in names the tenant). Google has one authority: `login_base` and `tenant` are not used.
pub fn oauth_endpoints_at(provider: OAuthProvider, login_base: &str, tenant: &str) -> OAuthAuthority {
    match provider {
        OAuthProvider::Microsoft => {
            let base = login_base.trim_end_matches('/');
            let mut e = oauth_endpoints(provider);
            e.authorize_url = format!("{base}/{tenant}/oauth2/v2.0/authorize");
            e.token_url = format!("{base}/{tenant}/oauth2/v2.0/token");
            e
        }
        OAuthProvider::Google => oauth_endpoints(provider),
    }
}

/// The FALLBACK endpoints for an address: Microsoft at the tenant of the address's domain (`organizations` when it is not an
/// address); Google as `oauth_endpoints`. A sign-in starts at `oauth_endpoints` (`common`, right for a multi-tenant
/// registration such as the shipped default); only a person's own single-tenant registration, refused there with
/// AADSTS50194, signs in here. Whether the domain IS a tenant is asked by `oauth_tenant_check_url`; the engine uses
/// `MICROSOFT_ANY_ORGANISATION` when that answer is a refusal.
pub fn oauth_endpoints_for(provider: OAuthProvider, address: &str) -> OAuthAuthority {
    let tenant = oauth_tenant_of(address).unwrap_or_else(|| MICROSOFT_ANY_ORGANISATION.to_string());
    oauth_endpoints_at(provider, MICROSOFT_LOGIN_BASE, &tenant)
}

/// The tenant's OpenID configuration: 200 when `tenant` is a Microsoft directory, 400 (AADSTS90002) when it is not.
pub fn oauth_tenant_check_url(login_base: &str, tenant: &str) -> String {
    format!("{}/{tenant}/v2.0/.well-known/openid-configuration", login_base.trim_end_matches('/'))
}

/// The provider an account kind signs in with, when it is one of the two.
#[uniffi::export]
pub fn oauth_provider_for(provider: MailProvider) -> Option<OAuthProvider> {
    match provider {
        MailProvider::Microsoft => Some(OAuthProvider::Microsoft),
        MailProvider::Google => Some(OAuthProvider::Google),
        _ => None,
    }
}

/// The provider a detected one signs in with by OAuth2: Microsoft 365 and Google. iCloud, Fastmail and the Proton
/// bridge take an app or bridge password.
#[uniffi::export]
pub fn oauth_provider_for_detected(provider: crate::DetectedProvider) -> Option<OAuthProvider> {
    match provider {
        crate::DetectedProvider::Microsoft365 => Some(OAuthProvider::Microsoft),
        crate::DetectedProvider::Google => Some(OAuthProvider::Google),
        _ => None,
    }
}

/// The key of an account being added: the address, trimmed, lower case. Refused in words when it is not an address.
#[uniffi::export]
pub fn oauth_account_draft_id(address: String) -> Result<String, CoreError> {
    let a = address.trim().to_lowercase();
    match a.split_once('@') {
        Some((l, d)) if !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.') && !a.contains(char::is_whitespace) => Ok(a),
        _ => Err(CoreError::Invalid { field: "address".into(), reason_words: format!("{} is not an email address", address.trim()) }),
    }
}

/// The client ID the person pasted, trimmed, or the reason it cannot be one, in words. The shapes are the providers'
/// published ones: a Microsoft application (client) ID is a GUID (8-4-4-4-12 hex); a Google OAuth client ID ends in
/// ".apps.googleusercontent.com". Nothing here names a real ID: the repository holds none (checks/no-vendor-id-check.py).
#[uniffi::export]
pub fn check_oauth_client_id(provider: OAuthProvider, client_id: String) -> Result<String, CoreError> {
    let id = client_id.trim().to_string();
    let bad = |why: &str| Err(CoreError::Invalid { field: "client_id".into(), reason_words: why.to_string() });
    if id.is_empty() { return bad("paste the client ID of the app you registered"); }
    if id.contains(char::is_whitespace) { return bad("a client ID has no spaces in it"); }
    match provider {
        OAuthProvider::Microsoft => {
            let parts: Vec<&str> = id.split('-').collect();
            let lens = [8usize, 4, 4, 4, 12];
            let ok = parts.len() == 5 && parts.iter().zip(lens).all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()));
            if ok { Ok(id.to_lowercase()) } else { bad("a Microsoft application (client) ID looks like 8-4-4-4-12 letters and digits; copy it from the app's Overview page") }
        }
        OAuthProvider::Google => {
            let suffix = ".apps.googleusercontent.com";
            match id.strip_suffix(suffix) {
                Some(head) if !head.is_empty() && head.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') => Ok(id),
                _ => bad("a Google client ID ends in .apps.googleusercontent.com; copy it from the OAuth client's page"),
            }
        }
    }
}

/// Where one sign-in stands (`oauth_sign_in_state(account_draft_id)`).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum OAuthSignInState {
    /// No client ID was set for this account yet: the screen asks for one.
    NeedsClientId,
    /// A client ID is set and no sign-in started.
    Ready,
    /// The browser is open on `authorize_url`; the app waits on its loopback address for the answer.
    Waiting { authorize_url: String },
    /// Signed in: the refresh token is in the Keychain.
    Done,
    /// The sign-in did not happen, in words (the provider's own error, a wrong state, a timeout).
    Refused { words: String },
}

/// The SASL XOAUTH2 client response before base64 (Microsoft and Gmail IMAP and SMTP): user, then the bearer token.
pub fn xoauth2_sasl(user: &str, access_token: &str) -> String {
    format!("user={user}\x01auth=Bearer {access_token}\x01\x01")
}
