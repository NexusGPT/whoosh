//! Ports: one method each, named `execute`. A key is a `KeychainRef`, never a string.
use contracts::{CoreError, KeychainRef};

pub struct TextRequest {
    pub model: String,
    pub system: Option<String>,
    pub prompt: String,
    /// The caller's ceiling for the answer (one of the ledger's numbers, passed in; this crate holds none).
    pub max_output_tokens: u32,
}

/// The model ids a key can use. `key` is None only for a local provider that needs none.
/// A refused key is `CoreError::Action` whose `reason_words` is the provider's own string.
pub trait ListModels: Send + Sync {
    fn execute(&self, key: Option<&KeychainRef>) -> Result<Vec<String>, CoreError>;
}

pub trait CompleteText: Send + Sync {
    fn execute(&self, key: Option<&KeychainRef>, request: &TextRequest) -> Result<String, CoreError>;
}

/// One recorded clip. The caller deletes the audio when it has the text (C-J3.2 invariant); this crate keeps nothing.
pub struct AudioClip {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub file_name: String,
    /// ISO 639-1 code, when the owner dictates in a known language.
    pub language: Option<String>,
}

pub trait Transcribe: Send + Sync {
    fn execute(&self, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError>;
}
