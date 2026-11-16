#![allow(dead_code)]
use attachment_index::twins::*;
use attachment_index::{AttachmentIndexer, Limits, Readers};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// An indexer over the twins, with the captions and readings the J2 corpus describes.
pub fn indexer(scratch: &Path, speech_refused: Option<&str>) -> AttachmentIndexer {
    indexer_with(scratch, speech_refused, Limits { text_max_chars: None, transcript_max_chars: None })
}

pub fn indexer_with(scratch: &Path, speech_refused: Option<&str>, limits: Limits) -> AttachmentIndexer {
    let mut model = TwinEmbedder::default();
    model.captions.insert("warehouse-photo.jpg".into(), "warehouse photo pallets of brochures delivery forklift aisle".into());
    model.captions.insert("tiny.png".into(), "red square".into());
    model.captions.insert("clip.wav".into(), "a beep".into());
    model.captions.insert("att-video".into(), "blue screen".into());
    let mut text = TwinText::default();
    text.0.insert("report.docx".into(), "Annual supplier review: the chosen printer is Atelier Merel".into());
    let mut image = TwinImage::default();
    image.ocr.insert("tiny.png".into(), "STOP".into());
    image.labels.insert("tiny.png".into(), vec!["red".into(), "square".into()]);
    image.ocr.insert("frame-1.jpg".into(), "Exit 4".into());
    let mut speech = TwinSpeech::default();
    speech.said.insert("clip.wav".into(), "send the invoice on Friday".into());
    speech.said.insert("att-video".into(), "welcome to the print works".into());
    speech.refuse = speech_refused.map(String::from);
    AttachmentIndexer::new(
        Arc::new(model),
        Readers { text: Arc::new(text), image: Arc::new(image), speech: Arc::new(speech), probe: Arc::new(TwinProbe { duration_s: 20.0, has_sound: true }) },
        limits,
        scratch.to_path_buf(),
    )
}
