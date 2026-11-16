//! Reads one downloaded attachment and returns what joins the indexes. Every piece fails on its own: a frame that will not decode,
//! a refused speech permission or a model error becomes a note in words, and the other pieces still count.
use crate::entry::{AttachmentIndexEntry, EntryVector, IndexState};
use crate::plan::MediaEmbedRequest;
use crate::ports::{ImageReader, MediaProbe, TextReader, Transcriber};
use crate::{constants, extract_pdf_text, AttachmentKind};
use contracts::{CoreError, Embedder};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Readers {
    pub text: Arc<dyn TextReader>,
    pub image: Arc<dyn ImageReader>,
    pub speech: Arc<dyn Transcriber>,
    pub probe: Arc<dyn MediaProbe>,
}

/// Caps from docs/CONSTANTS.md; a row not there yet means no cap.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub text_max_chars: Option<usize>,
    pub transcript_max_chars: Option<usize>,
}

impl Limits {
    pub fn from_constants() -> Limits {
        Limits { text_max_chars: constants::ATTACHMENT_TEXT_MAX_CHARS, transcript_max_chars: constants::TRANSCRIPT_MAX_CHARS }
    }
}

pub struct AttachmentInput<'a> {
    pub message_id: &'a str,
    pub attachment_id: &'a str,
    pub file_name: &'a str,
    pub mime_type: &'a str,
    /// None until the file is downloaded
    pub local_path: Option<&'a Path>,
    pub now_ms: i64,
}

pub struct AttachmentIndexer {
    pub(crate) embedder: Arc<dyn Embedder>,
    pub(crate) readers: Readers,
    pub(crate) limits: Limits,
    pub(crate) scratch: PathBuf,
}

impl AttachmentIndexer {
    /// `scratch` is a folder the indexer may fill with frames and sound tracks; it cleans up after each video.
    pub fn new(embedder: Arc<dyn Embedder>, readers: Readers, limits: Limits, scratch: PathBuf) -> Self {
        Self { embedder, readers, limits, scratch }
    }

    pub fn index(&self, input: &AttachmentInput) -> AttachmentIndexEntry {
        let kind = AttachmentKind::detect(input.mime_type, input.file_name);
        let mut e = AttachmentIndexEntry {
            message_id: input.message_id.to_string(),
            attachment_id: input.attachment_id.to_string(),
            name: input.file_name.to_string(),
            kind,
            state: IndexState::Indexed,
            text: None,
            ocr_text: None,
            labels: Vec::new(),
            transcript: None,
            vectors: Vec::new(),
            notes: Vec::new(),
            updated_at_ms: input.now_ms,
        };
        if kind == AttachmentKind::Unreadable {
            e.state = IndexState::Unreadable;
            return e;
        }
        let Some(path) = input.local_path.filter(|p| p.is_file()) else {
            e.state = IndexState::NotDownloaded;
            return e;
        };
        match kind {
            AttachmentKind::Pdf => self.read_pdf(&mut e, path),
            AttachmentKind::PlainText => self.read_plain(&mut e, path),
            AttachmentKind::Document => self.read_document(&mut e, path, kind),
            AttachmentKind::Image => self.read_image(&mut e, path),
            AttachmentKind::Audio => self.read_audio(&mut e, path),
            AttachmentKind::Video => self.read_video(&mut e, path),
            AttachmentKind::Unreadable => {}
        }
        self.embed_words(&mut e);
        let nothing = e.word_text().is_empty() && e.vectors.is_empty();
        if nothing && !e.notes.is_empty() {
            e.state = IndexState::Failed { reason_words: e.notes[0].clone() };
        }
        e
    }

    pub(crate) fn cap(text: String, max: Option<usize>) -> String {
        match max {
            Some(m) if text.chars().count() > m => text.chars().take(m).collect(),
            _ => text,
        }
    }

    pub(crate) fn note(e: &mut AttachmentIndexEntry, what: &str, err: impl std::fmt::Display) {
        e.notes.push(format!("{what}: {err}"));
    }

    pub(crate) fn set_text(&self, e: &mut AttachmentIndexEntry, t: Option<String>) {
        e.text = t.map(|t| Self::cap(t, self.limits.text_max_chars)).filter(|t| !t.trim().is_empty());
    }

    fn read_pdf(&self, e: &mut AttachmentIndexEntry, path: &Path) {
        let own = std::fs::read(path).ok().and_then(|b| extract_pdf_text(&b));
        if own.is_some() {
            return self.set_text(e, own);
        }
        // the core's reader declined (scan, glyph-id fonts, encryption): PDFKit answers
        self.read_document(e, path, AttachmentKind::Pdf);
    }

    fn read_plain(&self, e: &mut AttachmentIndexEntry, path: &Path) {
        match std::fs::read(path) {
            Ok(b) => self.set_text(e, Some(String::from_utf8_lossy(&b).into_owned())),
            Err(err) => Self::note(e, "reading the file", err),
        }
    }

    fn read_document(&self, e: &mut AttachmentIndexEntry, path: &Path, kind: AttachmentKind) {
        match self.readers.text.read_text(path, kind) {
            Ok(t) => self.set_text(e, t),
            Err(err) => Self::note(e, "reading its text", err),
        }
    }

    /// One media vector, refused when it is the wrong size, not a number, or all zeros.
    pub(crate) fn embed_media(&self, e: &mut AttachmentIndexEntry, req: &MediaEmbedRequest) {
        let result = self.embedder.embed_media(req.kind, req.path.to_string_lossy().into_owned()).and_then(|v| self.check(v));
        match result {
            Ok(vector) => e.vectors.push(EntryVector { key: format!("{}{}", e.base_key(), req.key_suffix()), vector }),
            Err(err) => Self::note(e, &format!("embedding the {:?}", req.kind).to_lowercase(), err),
        }
    }

    pub(crate) fn check(&self, v: Vec<f32>) -> Result<Vec<f32>, CoreError> {
        let dim = self.embedder.dimension() as usize;
        if v.len() != dim || v.iter().any(|x| !x.is_finite()) || v.iter().all(|x| *x == 0.0) {
            return Err(CoreError::Invalid { field: "vector".into(), reason_words: format!("the model returned {} numbers, not a usable vector of {dim}", v.len()) });
        }
        Ok(v)
    }
}
