//! Mistral Voxtral through the audio/transcriptions endpoint.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{whisper_shape, Ctx};
use crate::ports::{AudioClip, Transcribe};
use crate::table::SpeechId;
use crate::transport::Transport;

pub struct Mistral {
    ctx: Ctx,
}

impl Mistral {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> Mistral {
        Mistral { ctx: Ctx::new(SpeechId::Mistral, transport, secrets, url) }
    }
}

impl Transcribe for Mistral {
    fn execute(&self, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError> {
        whisper_shape::transcribe(&self.ctx, key, clip)
    }
}
