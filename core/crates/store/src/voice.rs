//! The voice profile: one short summary of how the owner writes, per address, rebuilt from sent mail at most every
//! VOICE_REFRESH_DAYS (docs/CONSTANTS.md, read at build time like the split tabs). The AI jobs build the summary; the store
//! keeps it and says whether it is still fresh.
use crate::error::StoreError;
use crate::Store;
use rusqlite::{params, OptionalExtension};

const CONSTANTS: &str = include_str!("../../../../docs/CONSTANTS.md");
const MS_PER_DAY: i64 = 86_400_000; // a unit, not a product value

/// VOICE_REFRESH_DAYS from the ledger's Value cell (the first integer in it).
pub fn voice_refresh_days() -> Result<i64, StoreError> {
    let cell = CONSTANTS
        .lines()
        .find(|l| l.trim_start().starts_with("| VOICE_REFRESH_DAYS |"))
        .and_then(|l| l.split('|').nth(2))
        .ok_or_else(|| StoreError::Invalid("docs/CONSTANTS.md has no VOICE_REFRESH_DAYS row".into()))?;
    let digits: String = cell.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().map_err(|_| StoreError::Invalid(format!("VOICE_REFRESH_DAYS is not a whole number in docs/CONSTANTS.md: {}", cell.trim())))
}

/// How the owner writes, as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct VoiceProfile {
    pub address: String,
    pub summary_words: String,
    pub built_at_ms: i64,
    pub updated_at_ms: i64,
}

impl VoiceProfile {
    /// True while the profile was built less than VOICE_REFRESH_DAYS before `now_ms`.
    pub fn is_fresh(&self, now_ms: i64) -> Result<bool, StoreError> {
        Ok(now_ms - self.built_at_ms < voice_refresh_days()? * MS_PER_DAY)
    }
}

impl Store {
    /// The profile kept for an address (the match ignores case), if any.
    pub fn voice_profile(&self, address: &str) -> Result<Option<VoiceProfile>, StoreError> {
        self.read(|c| {
            Ok(c.prepare_cached("SELECT address, summary_words, built_at_ms, updated_at_ms FROM voice_profile WHERE lower(address) = lower(?1)")?
                .query_row(params![address], |r| Ok(VoiceProfile { address: r.get(0)?, summary_words: r.get(1)?, built_at_ms: r.get(2)?, updated_at_ms: r.get(3)? }))
                .optional()?)
        })
    }

    /// Keep a profile: a new one for the address, or the old one replaced. An empty summary is refused (a profile with no
    /// words would read as "built" and block the next refresh).
    pub fn set_voice_profile(&self, address: String, summary_words: String, built_at_ms: i64, now_ms: i64) -> Result<VoiceProfile, StoreError> {
        if summary_words.trim().is_empty() {
            return Err(StoreError::Invalid("a voice profile needs words".into()));
        }
        if address.trim().is_empty() {
            return Err(StoreError::Invalid("a voice profile needs the address it describes".into()));
        }
        self.write(move |ctx| {
            let key = address.to_lowercase();
            ctx.tx.execute("DELETE FROM voice_profile WHERE lower(address) = ?1 AND address <> ?2", params![key, address])?;
            ctx.tx.execute(
                "INSERT INTO voice_profile (address, summary_words, built_at_ms, updated_at_ms) VALUES (?1, ?2, ?3, ?4) \
                 ON CONFLICT(address) DO UPDATE SET summary_words = excluded.summary_words, built_at_ms = excluded.built_at_ms, updated_at_ms = excluded.updated_at_ms",
                params![address, summary_words, built_at_ms, now_ms],
            )?;
            Ok(VoiceProfile { address, summary_words, built_at_ms, updated_at_ms: now_ms })
        })
    }
}
