//! Groq's hosted Whisper, the OpenAI-shaped transcription endpoint.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{whisper_shape, Ctx};
use crate::ports::{AudioClip, Transcribe};
use crate::table::SpeechId;
use crate::transport::Transport;

pub struct Groq {
    ctx: Ctx,
}

impl Groq {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> Groq {
        Groq { ctx: Ctx::new(SpeechId::Groq, transport, secrets, url) }
    }
}

impl Transcribe for Groq {
    fn execute(&self, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError> {
        whisper_shape::transcribe(&self.ctx, key, clip)
    }
}
