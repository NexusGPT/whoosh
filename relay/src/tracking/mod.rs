//! Sent-mail tracking storage and link signing. The relay keeps an opaque message token and an
//! event time per open or click, never a recipient address, an IP address or a destination URL:
//! a tracked link carries its destination inside the URL, signed with the relay's key, so only a
//! link the relay minted can redirect (no open redirect) and nothing about the link is stored.
use crate::registry::{Store, StoreError};
use hmac::{Hmac, Mac};
use rusqlite::{params, OptionalExtension};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub struct Signer {
    key: Vec<u8>,
    ephemeral: bool,
}

impl Signer {
    /// A missing key is allowed (dev, tests): links then die with the process, and the health
    /// route says `ephemeral`. A hosted relay sets RELAY_TRACKING_KEY.
    pub fn new(configured: Option<String>) -> Signer {
        match configured {
            Some(k) => Signer { key: k.into_bytes(), ephemeral: false },
            None => Signer { key: rand::random::<[u8; 32]>().to_vec(), ephemeral: true },
        }
    }

    pub fn is_ephemeral(&self) -> bool {
        self.ephemeral
    }

    fn mac(&self, token: &str, url: &str) -> Result<HmacSha256, String> {
        let mut mac = HmacSha256::new_from_slice(&self.key).map_err(|e| e.to_string())?;
        mac.update(token.as_bytes());
        mac.update(b"\n");
        mac.update(url.as_bytes());
        Ok(mac)
    }

    pub fn sign(&self, token: &str, url: &str) -> Result<String, String> {
        Ok(hex::encode(self.mac(token, url)?.finalize().into_bytes()))
    }

    pub fn verify(&self, token: &str, url: &str, signature: &str) -> bool {
        let Ok(bytes) = hex::decode(signature) else { return false };
        self.mac(token, url).map(|m| m.verify_slice(&bytes).is_ok()).unwrap_or(false)
    }
}

pub fn new_token() -> String {
    format!("{:032x}", rand::random::<u128>())
}

pub fn create_message(store: &Store, token: &str, now: i64) -> Result<(), StoreError> {
    store.with(|c| {
        c.execute("INSERT INTO tracked_message (token, created_at) VALUES (?1, ?2)", params![token, now])?;
        Ok(())
    })
}

pub fn message_exists(store: &Store, token: &str) -> Result<bool, StoreError> {
    store.with(|c| {
        let n: i64 = c.query_row("SELECT COUNT(1) FROM tracked_message WHERE token = ?1", params![token], |r| r.get(0))?;
        Ok(n > 0)
    })
}

/// Records an event only for a message the sender opted in. Returns whether a row was written.
pub fn record(store: &Store, token: &str, kind: &str, now: i64) -> Result<bool, StoreError> {
    store.with(|c| {
        let n = c.execute(
            "INSERT INTO tracking_event (token, kind, occurred_at)
             SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM tracked_message WHERE token = ?1)",
            params![token, kind, now],
        )?;
        Ok(n > 0)
    })
}

pub struct Summary {
    pub opens: i64,
    pub clicks: i64,
    pub first_open_at: Option<i64>,
    pub last_open_at: Option<i64>,
    pub last_click_at: Option<i64>,
}

pub fn summary(store: &Store, token: &str) -> Result<Option<Summary>, StoreError> {
    store.with(|c| {
        if c.query_row("SELECT 1 FROM tracked_message WHERE token = ?1", params![token], |_| Ok(())).optional()?.is_none() {
            return Ok(None);
        }
        c.query_row(
            "SELECT
               COALESCE(SUM(kind = 'open'), 0), COALESCE(SUM(kind = 'click'), 0),
               MIN(CASE WHEN kind = 'open' THEN occurred_at END),
               MAX(CASE WHEN kind = 'open' THEN occurred_at END),
               MAX(CASE WHEN kind = 'click' THEN occurred_at END)
             FROM tracking_event WHERE token = ?1",
            params![token],
            |r| Ok(Summary { opens: r.get(0)?, clicks: r.get(1)?, first_open_at: r.get(2)?, last_open_at: r.get(3)?, last_click_at: r.get(4)? }),
        )
        .map(Some)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signature_binds_the_token_and_the_destination() {
        let s = Signer::new(Some("k".into()));
        let sig = s.sign("aa", "https://x.example/").unwrap();
        assert!(s.verify("aa", "https://x.example/", &sig));
        assert!(!s.verify("aa", "https://y.example/", &sig));
        assert!(!s.verify("bb", "https://x.example/", &sig));
        assert!(!s.verify("aa", "https://x.example/", "zz"));
        assert!(!Signer::new(Some("other".into())).verify("aa", "https://x.example/", &sig));
    }

    #[test]
    fn a_missing_key_is_reported_ephemeral() {
        assert!(Signer::new(None).is_ephemeral());
        assert!(!Signer::new(Some("k".into())).is_ephemeral());
    }
}
