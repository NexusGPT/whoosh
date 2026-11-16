//! A key read from the Keychain for one call. Debug never prints it.
use std::fmt;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

pub(crate) struct Key(String);

impl Key {
    pub(crate) fn read(secrets: &dyn ReadSecret, reference: &KeychainRef) -> Result<Key, CoreError> {
        let secret = secrets.execute(reference)?;
        let text = std::str::from_utf8(secret.expose())
            .map_err(|_| CoreError::Invalid { field: "keychain_ref".into(), reason_words: "the stored key is not text".into() })?
            .trim()
            .to_string();
        if text.is_empty() {
            return Err(CoreError::Invalid { field: "keychain_ref".into(), reason_words: "the stored key is empty".into() });
        }
        Ok(Key(text))
    }
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key(<redacted>)")
    }
}
