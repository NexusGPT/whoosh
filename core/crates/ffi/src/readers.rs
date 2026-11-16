//! The attachment readers Swift implements (contracts `AttachmentTextReader`, `AttachmentImageReader`, `AttachmentTranscriber`,
//! `AttachmentMediaProbe`) adapted to core/crates/attachment-index's own ports, so the Rust indexer reads through PDFKit, Vision,
//! Speech and AVFoundation (delta 10). `readers_from_foreign` is what the real core calls when it builds its `AttachmentIndexer`.
use attachment_index::{AttachmentKind, ImageReader, MediaProbe, Readers, TextReader, TranscriptOutcome, TranscriptRequest, Transcriber};
use contracts::{
    AttachmentImageReader, AttachmentMediaProbe, AttachmentTextReader, AttachmentTranscriber, CoreError, ReaderFileKind, TranscriptResult,
};
use std::path::Path;
use std::sync::Arc;

fn path_text(p: &Path) -> String { p.to_string_lossy().into_owned() }

pub fn reader_kind(kind: AttachmentKind) -> ReaderFileKind {
    match kind {
        AttachmentKind::Pdf => ReaderFileKind::Pdf,
        AttachmentKind::PlainText => ReaderFileKind::PlainText,
        AttachmentKind::Document => ReaderFileKind::Document,
        AttachmentKind::Image => ReaderFileKind::Image,
        AttachmentKind::Audio => ReaderFileKind::Audio,
        AttachmentKind::Video => ReaderFileKind::Video,
        AttachmentKind::Unreadable => ReaderFileKind::Unreadable,
    }
}

pub fn transcript_outcome(r: TranscriptResult) -> TranscriptOutcome {
    match r {
        TranscriptResult::Text { text } => TranscriptOutcome::Text(text),
        TranscriptResult::NoSpeech => TranscriptOutcome::NoSpeech,
        TranscriptResult::Refused { reason_words } => TranscriptOutcome::Refused { reason_words },
    }
}

struct Text(Arc<dyn AttachmentTextReader>);
impl TextReader for Text {
    fn read_text(&self, path: &Path, kind: AttachmentKind) -> Result<Option<String>, CoreError> { self.0.read_text(path_text(path), reader_kind(kind)) }
}

struct Image(Arc<dyn AttachmentImageReader>);
impl ImageReader for Image {
    fn ocr(&self, path: &Path) -> Result<Option<String>, CoreError> { self.0.ocr(path_text(path)) }
    fn labels(&self, path: &Path) -> Result<Vec<String>, CoreError> { self.0.labels(path_text(path)) }
}

struct Speech(Arc<dyn AttachmentTranscriber>);
impl Transcriber for Speech {
    fn transcribe(&self, request: &TranscriptRequest) -> Result<TranscriptOutcome, CoreError> {
        self.0.transcribe(path_text(&request.path), request.from_video).map(transcript_outcome)
    }
}

struct Probe(Arc<dyn AttachmentMediaProbe>);
impl MediaProbe for Probe {
    fn video_duration_s(&self, path: &Path) -> Result<f64, CoreError> { self.0.video_duration_s(path_text(path)) }
    fn extract_frame(&self, path: &Path, at_s: f64, out: &Path) -> Result<(), CoreError> { self.0.extract_frame(path_text(path), at_s, path_text(out)) }
    fn extract_audio(&self, path: &Path, out: &Path) -> Result<bool, CoreError> { self.0.extract_audio(path_text(path), path_text(out)) }
}

/// The indexer's readers, answered by Swift.
pub fn readers_from_foreign(
    text: Arc<dyn AttachmentTextReader>,
    image: Arc<dyn AttachmentImageReader>,
    speech: Arc<dyn AttachmentTranscriber>,
    probe: Arc<dyn AttachmentMediaProbe>,
) -> Readers {
    Readers { text: Arc::new(Text(text)), image: Arc::new(Image(image)), speech: Arc::new(Speech(speech)), probe: Arc::new(Probe(probe)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_reaches_the_reader_under_its_own_name() {
        let pairs = [
            (AttachmentKind::Pdf, ReaderFileKind::Pdf), (AttachmentKind::PlainText, ReaderFileKind::PlainText),
            (AttachmentKind::Document, ReaderFileKind::Document), (AttachmentKind::Image, ReaderFileKind::Image),
            (AttachmentKind::Audio, ReaderFileKind::Audio), (AttachmentKind::Video, ReaderFileKind::Video),
            (AttachmentKind::Unreadable, ReaderFileKind::Unreadable),
        ];
        for (k, r) in pairs { assert_eq!(reader_kind(k), r, "{k:?}"); }
    }

    #[test]
    fn a_refused_transcript_keeps_its_words() {
        assert_eq!(transcript_outcome(TranscriptResult::Refused { reason_words: "allow speech recognition".into() }),
                   TranscriptOutcome::Refused { reason_words: "allow speech recognition".into() });
        assert_eq!(transcript_outcome(TranscriptResult::NoSpeech), TranscriptOutcome::NoSpeech);
    }
}
