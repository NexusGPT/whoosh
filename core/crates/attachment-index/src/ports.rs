//! The OS-provided pieces, answered by Sources/Features/AttachmentIndex (PDFKit, Vision, Speech, AVFoundation).
//! Each is a plain Rust trait so the crate and its checks run without the model or the OS; `twins` holds fixture answers.
//! The model itself is the contract `Embedder` (embed, embed_query, embed_media), already conformed by Sources/Features/Embedding.
use contracts::CoreError;
use std::path::Path;

use crate::{AttachmentKind, TranscriptRequest};

/// Text out of a file the core cannot read itself: PDFKit for a PDF the core's reader declined, the system's document readers
/// for word-processor, sheet and slide files. `None` = the file holds no text.
pub trait TextReader: Send + Sync {
    fn read_text(&self, path: &Path, kind: AttachmentKind) -> Result<Option<String>, CoreError>;
}

/// What Vision reads off a picture or a video frame, beside its embedding: the exact words in it, and scene labels.
pub trait ImageReader: Send + Sync {
    fn ocr(&self, path: &Path) -> Result<Option<String>, CoreError>;
    fn labels(&self, path: &Path) -> Result<Vec<String>, CoreError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptOutcome {
    Text(String),
    /// the recogniser ran and heard no speech
    NoSpeech,
    /// the owner has not allowed speech recognition, or it is not available for the language: `reason_words` says so to the owner
    Refused { reason_words: String },
}

/// Speech on device (SFSpeechRecognizer with on-device recognition required: nothing leaves this Mac).
pub trait Transcriber: Send + Sync {
    fn transcribe(&self, request: &TranscriptRequest) -> Result<TranscriptOutcome, CoreError>;
}

/// AVFoundation: how long a video is, a still at a time, the sound of a video. `out` is a file the probe creates.
pub trait MediaProbe: Send + Sync {
    fn video_duration_s(&self, path: &Path) -> Result<f64, CoreError>;
    fn extract_frame(&self, path: &Path, at_s: f64, out: &Path) -> Result<(), CoreError>;
    /// Ok(false) = the video has no sound track
    fn extract_audio(&self, path: &Path, out: &Path) -> Result<bool, CoreError>;
}
