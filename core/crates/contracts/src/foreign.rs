//! The traits Swift implements for the core (OS-provided capabilities).
use crate::{CoreError, MediaKind};

#[uniffi::export(with_foreign)]
pub trait Embedder: Send + Sync {
    fn dimension(&self) -> u32;
    /// Embeds documents (the text of a message or of an attachment), with the model's DOCUMENT prefix.
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError>;
    /// Embeds what the owner typed in the search field, with the model's QUERY prefix. Documents and queries take
    /// different prefixes (EmbeddingGemma 2: "task: search result | query: {query}" and "title: {title} | text: {content}"),
    /// so a query embedded as a document ranks worse. Same dimension as `embed` (delta 8).
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError>;
    /// Embeds the chunks of ONE document under its title (delta 12): the mail's subject goes in the title slot, so the
    /// document form is "title: {title} | text: {chunk}" ("title: none" when `title` is None or blank). Same vectors as
    /// `embed` when there is no title. Swift conformers written before delta 12 get a default that ignores the title and
    /// calls `embed` (`Embedder.embedTitled`), so they keep compiling; the on-device model implements it for real.
    fn embed_titled(&self, title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError>;
    /// Embeds one file straight into the text space: an image, an audio file, or a still taken from a video (delta 9; supersedes
    /// `MediaEmbedder`, which stays for the conformers that exist). `path` is a file the app can open. Same dimension as `embed`.
    /// Swift conformers written before delta 9 get a default that refuses in words (`Embedder.embedMedia`), so they keep compiling.
    fn embed_media(&self, kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError>;
}

/// The part of the on-device embedding model that reads files: an image or a voice note is embedded directly into the
/// same space as text, so a text query finds it (delta 8; docs/PRD.md "One search", Update 02:12). The app implements it
/// over the EmbeddingGemma 2 vision and audio encoders; `path` is a file the app can open.
#[uniffi::export(with_foreign)]
pub trait MediaEmbedder: Send + Sync {
    fn embed_image(&self, path: String) -> Result<Vec<f32>, CoreError>;
    fn embed_audio(&self, path: String) -> Result<Vec<f32>, CoreError>;
}

#[uniffi::export(with_foreign)]
pub trait LocalModel: Send + Sync {
    fn generate(&self, prompt: String, max_tokens: u32) -> Result<String, CoreError>;
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SyncChange { pub record_kind: String, pub id: String, pub payload: Vec<u8>, pub changed_at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SyncBatch { pub changes: Vec<SyncChange>, pub next_token: Option<String> }

#[uniffi::export(with_foreign)]
pub trait CloudSync: Send + Sync {
    fn push(&self, change: SyncChange) -> Result<(), CoreError>;
    fn pull(&self, since_token: Option<String>) -> Result<SyncBatch, CoreError>;
}

#[uniffi::export(with_foreign)]
pub trait PushSink: Send + Sync {
    fn register_routing(&self, account_id: String, routing_id: String) -> Result<(), CoreError>;
    fn unregister_routing(&self, account_id: String) -> Result<(), CoreError>;
}

/// Time comes from Swift so tests can move it (the socket verb `clock advance`).
#[uniffi::export(with_foreign)]
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
    fn utc_offset_s(&self) -> i32;
}

// ---- The attachment readers (delta 10, from core/crates/attachment-index/src/ports.rs). Swift answers them with PDFKit and the
// document readers, Vision, Speech (on device only) and AVFoundation; ffi adapts each to the crate's own trait. Paths are file paths.

/// What kind of file the reader is asked about (attachment-index `AttachmentKind`, by its label).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReaderFileKind { Pdf, PlainText, Document, Image, Audio, Video, Unreadable }

#[uniffi::export(with_foreign)]
pub trait AttachmentTextReader: Send + Sync {
    /// None = the file holds no text
    fn read_text(&self, path: String, kind: ReaderFileKind) -> Result<Option<String>, CoreError>;
}

#[uniffi::export(with_foreign)]
pub trait AttachmentImageReader: Send + Sync {
    /// the words in the picture (Vision text recognition); None = no text
    fn ocr(&self, path: String) -> Result<Option<String>, CoreError>;
    /// scene labels (Vision classification) above SCENE_LABEL_MIN_CONFIDENCE, at most SCENE_LABELS_MAX
    fn labels(&self, path: String) -> Result<Vec<String>, CoreError>;
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum TranscriptResult {
    Text { text: String },
    /// the recogniser ran and heard no speech
    NoSpeech,
    /// speech recognition is not allowed or not available for the language; the owner reads `reason_words`
    Refused { reason_words: String },
}

#[uniffi::export(with_foreign)]
pub trait AttachmentTranscriber: Send + Sync {
    /// on-device speech only: nothing leaves the Mac. `from_video`: the sound was pulled out of a video.
    fn transcribe(&self, audio_path: String, from_video: bool) -> Result<TranscriptResult, CoreError>;
}

#[uniffi::export(with_foreign)]
pub trait AttachmentMediaProbe: Send + Sync {
    fn video_duration_s(&self, path: String) -> Result<f64, CoreError>;
    fn extract_frame(&self, path: String, at_s: f64, out_path: String) -> Result<(), CoreError>;
    /// false = the video has no sound track
    fn extract_audio(&self, path: String, out_path: String) -> Result<bool, CoreError>;
}
