//! Deepgram: the clip itself as the request body, the model in the query, the key as `Authorization: Token`.
//! The text is `results.channels[0].alternatives[0].transcript`.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;

use super::{check_clip, Ctx};
use crate::call::enc;
use crate::ports::{AudioClip, Transcribe};
use crate::table::SpeechId;
use crate::transport::Transport;

pub struct Deepgram {
    ctx: Ctx,
}

impl Deepgram {
    pub fn new(transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> Deepgram {
        Deepgram { ctx: Ctx::new(SpeechId::Deepgram, transport, secrets, url) }
    }
}

impl Transcribe for Deepgram {
    fn execute(&self, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError> {
        check_clip(clip)?;
        let key = self.ctx.key(key)?;
        let mut url = format!("{}?model={}&smart_format=true", self.ctx.url(), enc(self.ctx.info.model));
        if let Some(language) = clip.language.as_deref() {
            url.push_str(&format!("&language={}", enc(language)));
        }
        let answer = self.ctx.post_bytes(url, &key, clip.content_type.clone(), clip.bytes.clone())?;
        answer["results"]["channels"][0]["alternatives"][0]["transcript"]
            .as_str()
            .map(|t| t.trim().to_string())
            .ok_or_else(|| self.ctx.unreadable())
    }
}
