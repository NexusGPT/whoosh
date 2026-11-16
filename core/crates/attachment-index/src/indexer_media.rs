//! The media half of the indexer: pictures, sound, video frames, and the words that also go to the meaning index.
use crate::chunk::chunks;
use crate::entry::{AttachmentIndexEntry, EntryVector};
use crate::indexer::AttachmentIndexer;
use crate::plan::{video_plan, MediaEmbedRequest, MediaPart, TranscriptRequest};
use crate::ports::TranscriptOutcome;
use contracts::{CoreError, MediaKind};
use std::path::Path;

impl AttachmentIndexer {
    pub(crate) fn read_image(&self, e: &mut AttachmentIndexEntry, path: &Path) {
        self.embed_media(e, &MediaEmbedRequest { kind: MediaKind::Image, path: path.to_path_buf(), part: MediaPart::Whole });
        self.read_pixels(e, path);
    }

    /// OCR text and scene labels of a picture or a frame, beside its embedding.
    pub(crate) fn read_pixels(&self, e: &mut AttachmentIndexEntry, path: &Path) {
        match self.readers.image.ocr(path) {
            Ok(Some(t)) if !t.trim().is_empty() => e.ocr_text = Some(match e.ocr_text.take() { Some(o) => format!("{o}\n{t}"), None => t }),
            Ok(_) => {}
            Err(err) => Self::note(e, "reading the words in the picture", err),
        }
        match self.readers.image.labels(path) {
            Ok(ls) => ls.into_iter().for_each(|l| if !e.labels.contains(&l) { e.labels.push(l) }),
            Err(err) => Self::note(e, "naming what the picture shows", err),
        }
    }

    pub(crate) fn transcribe(&self, e: &mut AttachmentIndexEntry, path: &Path, from_video: bool) {
        match self.readers.speech.transcribe(&TranscriptRequest { path: path.to_path_buf(), from_video }) {
            Ok(TranscriptOutcome::Text(t)) => {
                let t = Self::cap(t, self.limits.transcript_max_chars);
                e.transcript = Some(match e.transcript.take() { Some(o) => format!("{o}\n{t}"), None => t });
            }
            Ok(TranscriptOutcome::NoSpeech) => {}
            Ok(TranscriptOutcome::Refused { reason_words }) => e.notes.push(reason_words),
            Err(err) => Self::note(e, "transcribing the speech", err),
        }
    }

    pub(crate) fn read_audio(&self, e: &mut AttachmentIndexEntry, path: &Path) {
        self.embed_media(e, &MediaEmbedRequest { kind: MediaKind::Audio, path: path.to_path_buf(), part: MediaPart::Whole });
        self.transcribe(e, path, false);
    }

    pub(crate) fn read_video(&self, e: &mut AttachmentIndexEntry, path: &Path) {
        let duration = match self.readers.probe.video_duration_s(path) {
            Ok(d) => d,
            Err(err) => return Self::note(e, "opening the video", err),
        };
        let dir = self.scratch.join(&e.attachment_id);
        if let Err(err) = std::fs::create_dir_all(&dir) {
            return Self::note(e, "making room for the frames", err);
        }
        for req in video_plan(&dir, duration) {
            match req.part {
                MediaPart::Frame { index, at_s } => match self.readers.probe.extract_frame(path, at_s, &req.path) {
                    Ok(()) => {
                        self.embed_media(e, &req);
                        self.read_pixels(e, &req.path);
                    }
                    Err(err) => Self::note(e, &format!("taking frame {index}"), err),
                },
                MediaPart::AudioTrack => match self.readers.probe.extract_audio(path, &req.path) {
                    Ok(true) => {
                        self.embed_media(e, &req);
                        self.transcribe(e, &req.path, true);
                    }
                    Ok(false) => {}
                    Err(err) => Self::note(e, "taking the sound track", err),
                },
                MediaPart::Whole => {}
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Text, OCR text and transcript also go to the meaning index, in windows.
    pub(crate) fn embed_words(&self, e: &mut AttachmentIndexEntry) {
        let parts = [("t", e.text.clone()), ("o", e.ocr_text.clone()), ("s", e.transcript.clone())];
        for (tag, text) in parts {
            let windows = text.map(|t| chunks(&t)).unwrap_or_default();
            if windows.is_empty() {
                continue;
            }
            match self.embedder.embed(windows.clone()).and_then(|vs| {
                if vs.len() != windows.len() {
                    return Err(CoreError::Invalid { field: "vectors".into(), reason_words: "the model answered for fewer passages than it was given".into() });
                }
                vs.into_iter().map(|v| self.check(v)).collect::<Result<Vec<_>, _>>()
            }) {
                Ok(vs) => {
                    let base = e.base_key();
                    e.vectors.extend(vs.into_iter().enumerate().map(|(i, vector)| EntryVector { key: format!("{base}#{tag}{i}"), vector }));
                }
                Err(err) => Self::note(e, "embedding its words", err),
            }
        }
    }
}
