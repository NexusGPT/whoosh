//! Fixture twins of the OS pieces and of the model, so the crate and its checks run without weights or frameworks.
//! The twin embedder places a text by its words (hashed into EMBED_DIM numbers) and a media file by a CAPTION the fixture gives it:
//! what the real model would see in the picture. It proves the wiring, never retrieval quality (the real model is measured in
//! _captures/attachment-index/timings.txt and by Sources/Features/Embedding).
use crate::constants::EMBED_DIM;
use crate::ports::{ImageReader, MediaProbe, TextReader, Transcriber, TranscriptOutcome};
use crate::{AttachmentKind, TranscriptRequest};
use contracts::{CoreError, Embedder, MediaKind};
use std::collections::HashMap;
use std::path::Path;

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn hashed(text: &str) -> Vec<f32> {
    let mut v = vec![0f32; EMBED_DIM];
    // the twin model ignores filler words, as a real one does: "the" must not make two unrelated texts neighbours
    const FILLER: &[&str] = &["the", "and", "for", "with", "from", "that", "this", "what", "which"];
    for w in text.split(|c: char| !c.is_alphanumeric()).filter(|w| w.len() > 2 && !FILLER.contains(&w.to_lowercase().as_str())) {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in w.to_lowercase().bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
        v[(h % EMBED_DIM as u64) as usize] += 1.0;
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter_mut().for_each(|x| *x /= norm);
    }
    v
}

/// captions: file name (or the part of a frame/audio file name after the attachment's folder) -> what the model would see or hear.
#[derive(Default)]
pub struct TwinEmbedder {
    pub captions: HashMap<String, String>,
}

impl Embedder for TwinEmbedder {
    fn dimension(&self) -> u32 {
        EMBED_DIM as u32
    }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> {
        Ok(texts.iter().map(|t| hashed(t)).collect())
    }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) } // mechanical (contracts delta 12)
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> {
        Ok(hashed(&text))
    }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> {
        let name = file_name(Path::new(&path));
        let caption = self.captions.get(&name).cloned().or_else(|| {
            // frames and sound tracks live in a folder named after the attachment: its caption applies
            let parent = Path::new(&path).parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned())?;
            self.captions.get(&parent).cloned()
        });
        match caption {
            Some(c) => Ok(hashed(&c)),
            None => Err(CoreError::Unexpected { reason_words: format!("the twin model has no caption for {name}") }),
        }
    }
}

#[derive(Default)]
pub struct TwinText(pub HashMap<String, String>);
impl TextReader for TwinText {
    fn read_text(&self, path: &Path, _kind: AttachmentKind) -> Result<Option<String>, CoreError> {
        Ok(self.0.get(&file_name(path)).cloned())
    }
}

#[derive(Default)]
pub struct TwinImage {
    pub ocr: HashMap<String, String>,
    pub labels: HashMap<String, Vec<String>>,
}
impl ImageReader for TwinImage {
    fn ocr(&self, path: &Path) -> Result<Option<String>, CoreError> {
        Ok(self.ocr.get(&file_name(path)).cloned())
    }
    fn labels(&self, path: &Path) -> Result<Vec<String>, CoreError> {
        Ok(self.labels.get(&file_name(path)).cloned().unwrap_or_default())
    }
}

#[derive(Default)]
pub struct TwinSpeech {
    pub said: HashMap<String, String>,
    /// when set, every request is refused in these words (permission off)
    pub refuse: Option<String>,
}
impl Transcriber for TwinSpeech {
    fn transcribe(&self, r: &TranscriptRequest) -> Result<TranscriptOutcome, CoreError> {
        if let Some(w) = &self.refuse {
            return Ok(TranscriptOutcome::Refused { reason_words: w.clone() });
        }
        let key = if r.from_video { r.path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() } else { file_name(&r.path) };
        Ok(self.said.get(&key).cloned().map_or(TranscriptOutcome::NoSpeech, TranscriptOutcome::Text))
    }
}

/// A video of a fixed length whose frames and sound are small files the twin writes.
pub struct TwinProbe {
    pub duration_s: f64,
    pub has_sound: bool,
}
impl MediaProbe for TwinProbe {
    fn video_duration_s(&self, _path: &Path) -> Result<f64, CoreError> {
        Ok(self.duration_s)
    }
    fn extract_frame(&self, _path: &Path, at_s: f64, out: &Path) -> Result<(), CoreError> {
        std::fs::write(out, format!("frame at {at_s}")).map_err(|e| CoreError::Unexpected { reason_words: e.to_string() })
    }
    fn extract_audio(&self, _path: &Path, out: &Path) -> Result<bool, CoreError> {
        if self.has_sound {
            std::fs::write(out, b"sound").map_err(|e| CoreError::Unexpected { reason_words: e.to_string() })?;
        }
        Ok(self.has_sound)
    }
}
