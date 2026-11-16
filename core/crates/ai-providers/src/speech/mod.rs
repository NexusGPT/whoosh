//! Cloud speech, one file per provider. Each implements `Transcribe` with the owner's key, read by `KeychainRef`.
use std::sync::Arc;

use contracts::{CoreError, KeychainRef};
use secrets::ReadSecret;
use serde_json::Value;

use crate::call;
use crate::key::Key;
use crate::ports::{AudioClip, Transcribe};
use crate::table::{speech_info, SpeechId, SpeechInfo};
use crate::transport::{Body, Method, Request, Transport};
use crate::words;

mod deepgram;
mod elevenlabs;
mod form;
mod groq;
mod mistral;
mod openai;
mod whisper_shape;

pub use deepgram::Deepgram;
pub use elevenlabs::ElevenLabs;
pub use groq::Groq;
pub use mistral::Mistral;
pub use openai::OpenAiTranscription;

/// The adapter for `id`. `url` replaces the table's address (a proxy, a test server).
pub fn build(id: SpeechId, transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> Box<dyn Transcribe> {
    match id {
        SpeechId::OpenAi => Box::new(OpenAiTranscription::new(transport, secrets, url)),
        SpeechId::Deepgram => Box::new(Deepgram::new(transport, secrets, url)),
        SpeechId::ElevenLabs => Box::new(ElevenLabs::new(transport, secrets, url)),
        SpeechId::Groq => Box::new(Groq::new(transport, secrets, url)),
        SpeechId::Mistral => Box::new(Mistral::new(transport, secrets, url)),
    }
}

pub(crate) struct Ctx {
    pub(crate) info: &'static SpeechInfo,
    url: String,
    transport: Arc<dyn Transport>,
    secrets: Arc<dyn ReadSecret>,
}

impl Ctx {
    pub(crate) fn new(id: SpeechId, transport: Arc<dyn Transport>, secrets: Arc<dyn ReadSecret>, url: Option<String>) -> Ctx {
        let info = speech_info(id);
        Ctx { info, url: url.unwrap_or_else(|| info.url.to_string()), transport, secrets }
    }

    pub(crate) fn key(&self, reference: &KeychainRef) -> Result<Key, CoreError> {
        Key::read(&*self.secrets, reference)
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    /// POST raw bytes of `content_type` to `url`, answered as JSON or as the provider's words.
    pub(crate) fn post_bytes(&self, url: String, key: &Key, content_type: String, data: Vec<u8>) -> Result<Value, CoreError> {
        let headers = self.info.auth.headers(Some(key.as_str()));
        let request = Request { method: Method::Post, url, headers, body: Body::Bytes { content_type, data } };
        call::json(&*self.transport, self.info.name, request, Some(key))
    }

    pub(crate) fn unreadable(&self) -> CoreError {
        words::unreadable(self.info.name)
    }
}

pub(crate) fn text_of(answer: &Value, ctx: &Ctx) -> Result<String, CoreError> {
    answer["text"].as_str().map(|t| t.trim().to_string()).ok_or_else(|| ctx.unreadable())
}

pub(crate) fn check_clip(clip: &AudioClip) -> Result<(), CoreError> {
    if clip.bytes.is_empty() {
        return Err(CoreError::Invalid { field: "audio".into(), reason_words: "the recording is empty".into() });
    }
    Ok(())
}
