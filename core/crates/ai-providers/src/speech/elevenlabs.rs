//! ElevenLabs Scribe: multipart `file` + `model_id` to speech-to-text, the key in `xi-api-key`, answered `{"text": ...}`.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{check_clip, form, text_of, Ctx};
use crate::ports::{AudioClip, Transcribe};
use crate::table::SpeechId;
use crate::transport::Transport;

pub struct ElevenLabs {
    ctx: Ctx,
}

impl ElevenLabs {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> ElevenLabs {
        ElevenLabs { ctx: Ctx::new(SpeechId::ElevenLabs, transport, secrets, url) }
    }
}

impl Transcribe for ElevenLabs {
    fn execute(&self, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError> {
        check_clip(clip)?;
        let key = self.ctx.key(key)?;
        let mut fields = vec![("model_id", self.ctx.info.model)];
        if let Some(language) = clip.language.as_deref() {
            fields.push(("language_code", language));
        }
        let (content_type, data) = form::build(&fields, "file", clip);
        let answer = self.ctx.post_bytes(self.ctx.url().to_string(), &key, content_type, data)?;
        text_of(&answer, &self.ctx)
    }
}
