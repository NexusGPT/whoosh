//! The result for one attachment, and its words for the owner.
use crate::AttachmentKind;
use contracts::AttachmentContent;

/// J2.8 failure paths, verbatim: the words the result row carries.
pub const NOT_DOWNLOADED_WORDS: &str = "not indexed yet, download to search inside";
pub const UNREADABLE_WORDS: &str = "this kind of file cannot be read here, searched by name only";

#[derive(Debug, Clone, PartialEq)]
pub enum IndexState {
    Indexed,
    NotDownloaded,
    Unreadable,
    /// reading was tried and gave nothing usable; `reason_words` is for the owner
    Failed { reason_words: String },
}

/// One vector of an attachment: `key` is "<message_id>/<attachment_id>" plus the part suffix; the vector index files it under it.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryVector {
    pub key: String,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttachmentIndexEntry {
    pub message_id: String,
    pub attachment_id: String,
    pub name: String,
    pub kind: AttachmentKind,
    pub state: IndexState,
    pub text: Option<String>,
    pub ocr_text: Option<String>,
    pub labels: Vec<String>,
    pub transcript: Option<String>,
    pub vectors: Vec<EntryVector>,
    /// what went wrong or was skipped, in words (a refused permission, a frame that would not decode)
    pub notes: Vec<String>,
    pub updated_at_ms: i64,
}

impl AttachmentIndexEntry {
    pub fn base_key(&self) -> String {
        format!("{}/{}", self.message_id, self.attachment_id)
    }

    /// Everything readable out of the file, for the word index (stored in `attachment.extracted_text`, which the text index reads).
    pub fn word_text(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        parts.extend(self.text.as_deref());
        parts.extend(self.ocr_text.as_deref());
        parts.extend(self.labels.iter().map(String::as_str));
        parts.extend(self.transcript.as_deref());
        parts.retain(|p| !p.trim().is_empty());
        parts.join("\n")
    }

    /// The words a result row shows when the file could not be searched inside; None when it was.
    pub fn row_words(&self) -> Option<String> {
        match &self.state {
            IndexState::Indexed => None,
            IndexState::NotDownloaded => Some(NOT_DOWNLOADED_WORDS.to_string()),
            IndexState::Unreadable => Some(UNREADABLE_WORDS.to_string()),
            IndexState::Failed { reason_words } => Some(reason_words.clone()),
        }
    }

    /// The DATA-MAP record. The vector itself is not in it; `vector_key` names where the vector index files it.
    pub fn to_content(&self) -> AttachmentContent {
        let non_empty = |o: &Option<String>| o.clone().filter(|s| !s.trim().is_empty());
        AttachmentContent {
            attachment_id: self.attachment_id.clone(),
            text: non_empty(&self.text),
            ocr_text: non_empty(&self.ocr_text),
            labels: self.labels.clone(),
            transcript: non_empty(&self.transcript),
            embedded: !self.vectors.is_empty(),
            vector_key: if self.vectors.is_empty() { None } else { Some(self.base_key()) },
            updated_at_ms: self.updated_at_ms,
        }
    }
}
