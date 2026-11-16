//! PKCE S256 (RFC 7636 4.1 and 4.2): a high-entropy verifier and its SHA-256 challenge, both base64url without padding.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use contracts::CoreError;
use sha2::{Digest, Sha256};

/// `bytes` random bytes from the OS, base64url without padding (32 bytes give 43 characters: the verifier's minimum).
pub fn random_url_safe(bytes: usize) -> Result<String, CoreError> {
    let mut b = vec![0u8; bytes];
    getrandom::getrandom(&mut b).map_err(|e| CoreError::Unexpected { reason_words: format!("the system gave no random bytes: {e}") })?;
    Ok(URL_SAFE_NO_PAD.encode(b))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce { pub verifier: String, pub challenge: String }

impl Pkce {
    /// A fresh pair: a 43-character verifier (32 random bytes).
    pub fn new() -> Result<Pkce, CoreError> { Ok(Pkce::from_verifier(random_url_safe(32)?)) }

    pub fn from_verifier(verifier: String) -> Pkce {
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Pkce { verifier, challenge }
    }
}
