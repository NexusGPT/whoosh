//! What goes to the model for one file: the requests, before anything runs (so they can be checked without a model).
use crate::constants::{VIDEO_FRAMES_MAX, VIDEO_FRAME_EVERY_S};
use contracts::MediaKind;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaPart {
    Whole,
    /// frame number and its position in the video, in seconds
    Frame { index: usize, at_s: f64 },
    /// the sound of a video
    AudioTrack,
}

/// One call of `Embedder::embed_media`: an image file, an audio file, or a still taken from a video.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEmbedRequest {
    pub kind: MediaKind,
    pub path: PathBuf,
    pub part: MediaPart,
}

impl MediaEmbedRequest {
    /// The suffix that keeps the vectors of one attachment apart under its `vector_key`.
    pub fn key_suffix(&self) -> String {
        match self.part {
            MediaPart::Whole => String::new(),
            MediaPart::Frame { index, .. } => format!("#f{index}"),
            MediaPart::AudioTrack => "#audio".to_string(),
        }
    }
}

/// One call of the on-device speech recogniser.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptRequest {
    pub path: PathBuf,
    /// true when `path` is the sound pulled out of a video
    pub from_video: bool,
}

/// Positions (seconds) of the frames sampled from a video of `duration_s`: one per VIDEO_FRAME_EVERY_S, at most VIDEO_FRAMES_MAX,
/// at least one. A longer video is spread evenly so its end is seen too.
pub fn frame_times(duration_s: f64) -> Vec<f64> {
    if !duration_s.is_finite() || duration_s <= 0.0 {
        return vec![0.0];
    }
    let every = VIDEO_FRAME_EVERY_S as f64;
    let wanted = (duration_s / every).ceil().max(1.0) as usize;
    let count = wanted.min(VIDEO_FRAMES_MAX);
    let step = if wanted > VIDEO_FRAMES_MAX { duration_s / count as f64 } else { every };
    (0..count).map(|i| (i as f64 * step).min((duration_s - 0.05).max(0.0))).collect()
}

/// The requests for a video: its sampled frames, then its sound, all under `dir`.
pub fn video_plan(dir: &std::path::Path, duration_s: f64) -> Vec<MediaEmbedRequest> {
    let mut plan: Vec<MediaEmbedRequest> = frame_times(duration_s)
        .into_iter()
        .enumerate()
        .map(|(index, at_s)| MediaEmbedRequest { kind: MediaKind::VideoFrame, path: dir.join(format!("frame-{index}.jpg")), part: MediaPart::Frame { index, at_s } })
        .collect();
    plan.push(MediaEmbedRequest { kind: MediaKind::Audio, path: dir.join("audio.m4a"), part: MediaPart::AudioTrack });
    plan
}
