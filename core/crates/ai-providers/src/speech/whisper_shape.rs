//! The transcription form OpenAI, Groq and Mistral all accept: multipart `file` + `model` (+ `language`), answered `{"text": ...}`.
use contracts::{CoreError, KeychainRef};

use super::{check_clip, form, text_of, Ctx};
use crate::ports::AudioClip;

pub(crate) fn transcribe(ctx: &Ctx, key: &KeychainRef, clip: &AudioClip) -> Result<String, CoreError> {
    check_clip(clip)?;
    let key = ctx.key(key)?;
    let mut fields = vec![("model", ctx.info.model)];
    if let Some(language) = clip.language.as_deref() {
        fields.push(("language", language));
    }
    let (content_type, data) = form::build(&fields, "file", clip);
    let answer = ctx.post_bytes(ctx.url().to_string(), &key, content_type, data)?;
    text_of(&answer, ctx)
}
