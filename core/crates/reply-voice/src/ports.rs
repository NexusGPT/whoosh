//! The two ports another lane implements. One method each, named `execute`, `Send + Sync`, errors are `CoreError` (as ai-providers and ai-jobs).
use contracts::CoreError;

/// One message of the owner's Sent mail, as the store holds it: the plain-text body (quotes and signature still in it: this crate strips them),
/// who it went to, and when. `thread_id` is the thread it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentMessage {
    pub thread_id: String,
    /// every To address (the first of them is the person the owner wrote to)
    pub recipients: Vec<String>,
    pub date_ms: i64,
    pub text: String,
}

/// Which sent messages to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentQuery {
    /// the owner's latest sent messages, any recipient (the analysis)
    Latest,
    /// the owner's sent messages to this address (same-person examples)
    ToRecipient { recipient: String },
    /// the owner's sent messages in this thread (match thread)
    InThread { thread_id: String },
}

/// The owner's Sent mail, from the LOCAL store only (nothing waits on the network). Newest first, at most `limit`. The caller does not trust the
/// order, the limit or the filter: it sorts, cuts and filters again.
pub trait SentMail: Send + Sync {
    fn execute(&self, query: &SentQuery, limit: usize) -> Result<Vec<SentMessage>, CoreError>;
}

/// One sent message as the model sees it: its date and its own words (no address leaves the Mac for the analysis).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSample {
    pub date_ms: i64,
    pub text: String,
}

/// What the model is asked: the instruction (written here, in English: the model reads it, the owner does not), the language the answer is
/// written in, and the samples, newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceRequest {
    pub instruction: String,
    pub language: String,
    pub samples: Vec<VoiceSample>,
}

/// The model that describes a voice. Returns the tone text. A model that cannot be reached is `CoreError::Action` or `Unexpected` with the REAL reason.
pub trait VoiceWriter: Send + Sync {
    fn execute(&self, request: &VoiceRequest) -> Result<String, CoreError>;
}
