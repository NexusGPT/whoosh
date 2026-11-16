//! The owner's voice profile: built from the owner's own sent mail, rebuilt at most once per VOICE_REFRESH_DAYS.
//! The window is read from docs/CONSTANTS.md at build time, so the number lives once.
//! The profile is a saved description (J3.3); the draft job takes it as its `voice` field.
use std::sync::Arc;

use contracts::{CoreError, VoiceProfile};
use serde_json::Value;

use crate::engine::RunJob;
use crate::input::{Input, MailLine};
use crate::spec::JobId;

const LEDGER: &str = include_str!("../../../../docs/CONSTANTS.md");
const MS_PER_DAY: i64 = 86_400_000;

/// The row `VOICE_REFRESH_DAYS | 7 | ...`.
pub fn voice_refresh_days() -> Result<u32, CoreError> {
    let bad = |why: &str| CoreError::Unexpected { reason_words: format!("docs/CONSTANTS.md row VOICE_REFRESH_DAYS is unreadable: {why}") };
    let line = LEDGER.lines().find(|l| l.trim_start().starts_with("| VOICE_REFRESH_DAYS")).ok_or_else(|| bad("the row is missing"))?;
    let cell = line.split('|').nth(2).ok_or_else(|| bad("no value cell"))?;
    cell.trim().parse::<u32>().ok().filter(|d| *d > 0).ok_or_else(|| bad("expected a whole number of days above zero"))
}

/// One message from the owner's Sent folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMessage {
    pub from_address: String,
    pub date: Option<String>,
    pub text: String,
}

/// The owner's Sent mail for one address, newest first. The adapter reads the Sent folder of that account and nothing else.
pub trait SentMail: Send + Sync {
    fn execute(&self, owner_address: &str) -> Result<Vec<SentMessage>, CoreError>;
}
pub trait ReadVoiceProfile: Send + Sync {
    fn execute(&self, address: &str) -> Result<Option<VoiceProfile>, CoreError>;
}
pub trait WriteVoiceProfile: Send + Sync {
    fn execute(&self, profile: &VoiceProfile) -> Result<(), CoreError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Refresh {
    /// The saved profile is younger than the window: no model call, no write.
    Kept { age_ms: i64 },
    /// There is no sent mail to learn from yet: no model call, no write.
    NoSentMail,
    Rebuilt(VoiceProfile),
}

pub struct VoiceRefresh {
    jobs: Arc<dyn RunJob>,
    sent: Arc<dyn SentMail>,
    read: Arc<dyn ReadVoiceProfile>,
    write: Arc<dyn WriteVoiceProfile>,
    window_ms: i64,
}

impl VoiceRefresh {
    pub fn new(jobs: Arc<dyn RunJob>, sent: Arc<dyn SentMail>, read: Arc<dyn ReadVoiceProfile>, write: Arc<dyn WriteVoiceProfile>) -> Result<Self, CoreError> {
        Ok(Self { jobs, sent, read, write, window_ms: i64::from(voice_refresh_days()?) * MS_PER_DAY })
    }

    pub fn execute(&self, address: &str, now_ms: i64) -> Result<Refresh, CoreError> {
        if let Some(p) = self.read.execute(address)? {
            let age_ms = now_ms - p.built_at_ms;
            if age_ms < self.window_ms {
                return Ok(Refresh::Kept { age_ms });
            }
        }
        // Only the owner's own words reach the model: a message from anyone else is dropped here even if the adapter let it through.
        let own: Vec<MailLine> = self
            .sent
            .execute(address)?
            .into_iter()
            .filter(|m| m.from_address.trim().eq_ignore_ascii_case(address.trim()) && !m.text.trim().is_empty())
            .map(|m| MailLine { from: m.from_address, date: m.date, text: m.text })
            .collect();
        if own.is_empty() {
            return Ok(Refresh::NoSentMail);
        }
        let out = self.jobs.execute(JobId::VoiceProfile, &Input::new().messages("sent", &own))?;
        let summary = out.value.get("summary_words").and_then(Value::as_str).unwrap_or_default().to_string();
        let profile = VoiceProfile { address: address.to_string(), summary_words: summary, built_at_ms: now_ms };
        self.write.execute(&profile)?;
        Ok(Refresh::Rebuilt(profile))
    }
}
