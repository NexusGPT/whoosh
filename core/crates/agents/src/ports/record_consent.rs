use contracts::CoreError;

/// Keeps the owner's yes for a provider, so the question is never asked again for it.
pub trait RecordConsent: Send + Sync {
    fn execute(&self, provider_id: &str, at_ms: i64) -> Result<(), CoreError>;
}
