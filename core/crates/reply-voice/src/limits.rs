//! The two sizes this crate cuts to. PROPOSED for docs/CONSTANTS.md (VOICE_ANALYSIS_CHARS, VOICE_EXAMPLE_CHARS), not written there: the lane that owns
//! the ledger decides. The engine passes the chosen model's real room instead when it knows it.

/// Characters of sent mail one analysis reads: about 3 000 tokens, which a small local model reads whole.
pub const VOICE_ANALYSIS_CHARS: usize = 12_000;
/// Characters of one earlier reply joined to a draft.
pub const VOICE_EXAMPLE_CHARS: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceLimits {
    pub analysis_chars: usize,
    pub example_chars: usize,
}

impl Default for VoiceLimits {
    fn default() -> Self {
        Self { analysis_chars: VOICE_ANALYSIS_CHARS, example_chars: VOICE_EXAMPLE_CHARS }
    }
}
