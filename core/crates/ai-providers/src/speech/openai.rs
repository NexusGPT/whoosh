//! OpenAI transcription (the audio/transcriptions endpoint).
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{whisper_shape, Ctx};
use crate::ports::{AudioClip, Transcribe};
use crate::table::SpeechId;
use crate::transport::Transport;

pub struct OpenAiTranscription {
    ctx: Ctx,
}

impl OpenAiTranscription {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> OpenAiTranscription {
        OpenAiTranscription { ctx: Ctx::new(SpeechId::OpenAi, transport, secrets, url) }
    }
}

impl Transcribe for OpenAiTranscription {
    fn execute(&self, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError> {
        whisper_shape::transcribe(&self.ctx, key, clip)
    }
}
