//! Reply voice (delta 20, docs/JOURNEYS.md J10.3 and J10.8 to J10.10; thinking in docs/design/AI-REPLIES-MODEL.md section 2).
//! Three pieces of pure logic, each behind the two ports of `ports`:
//!   1. `analyse`: the owner's last `sample_size` sent messages, stripped of quotes and signatures, cut to the model's character budget (newest first,
//!      whole messages), turned into a short tone text in the app's language. The count really read comes back with it.
//!   2. `draft_voice::voice_for_draft`: the tone text a draft uses, and the owner's earlier replies as examples (this thread's when `match_thread`
//!      is on and the owner already wrote in it, else those to the same person).
//!   3. `clean`: the stripping and the cut both use.
//! Nothing here reads a store, calls a network or starts a thread: the engine lane implements `SentMail` over the local database and `VoiceWriter`
//! over the model, and runs the analysis in the background (no view waits on it; the owner's "<50ms for any operation").
mod analyse;
mod clean;
mod draft_voice;
mod limits;
mod ports;

pub use analyse::{
    analyse_voice, prepare_analysis, voice_instruction, AnalysisRequest, PreparedAnalysis, VoiceAnalysis, VoiceFailure, VoiceFailureKind,
};
pub use clean::{clean_sent_text, cut_to_chars};
pub use draft_voice::{voice_for_draft, DraftContext, DraftVoice, ExamplesFrom, VoiceExample};
pub use limits::{VoiceLimits, VOICE_ANALYSIS_CHARS, VOICE_EXAMPLE_CHARS};
pub use ports::{SentMail, SentMessage, SentQuery, VoiceRequest, VoiceSample, VoiceWriter};
