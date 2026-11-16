//! The authorization request URL and form bodies, percent-encoded by RFC 3986's unreserved set.
use contracts::{OAuthAuthority, OAuthProvider};

pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { out.push(b as char) } else { out.push_str(&format!("%{b:02X}")) }
    }
    out
}

pub fn form_encode(fields: &[(&str, &str)]) -> String {
    fields.iter().map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v))).collect::<Vec<_>>().join("&")
}

/// The browser URL of one sign-in: the code flow with PKCE S256, the loopback redirect, the state, the address as a hint.
/// Google returns a refresh token only with `access_type=offline` and a consent prompt; Microsoft returns one for the
/// `offline_access` scope.
pub fn authorize_url(provider: OAuthProvider, e: &OAuthAuthority, client_id: &str, redirect_uri: &str, state: &str, challenge: &str, login_hint: &str) -> String {
    let scope = e.scopes.join(" ");
    let mut f: Vec<(&str, &str)> = vec![
        ("client_id", client_id), ("response_type", "code"), ("redirect_uri", redirect_uri), ("scope", &scope),
        ("state", state), ("code_challenge", challenge), ("code_challenge_method", "S256"), ("login_hint", login_hint),
    ];
    if provider == OAuthProvider::Google { f.push(("access_type", "offline")); f.push(("prompt", "consent")); }
    format!("{}?{}", e.authorize_url, form_encode(&f))
}
