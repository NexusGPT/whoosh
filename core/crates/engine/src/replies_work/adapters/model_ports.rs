//! The ports of reply-retrieval (`QueryWriter`, `ExcerptJudge`) and reply-voice (`VoiceWriter`) over the one `TextModel`: each a single call of its
//! own kind, its prompt cut to the model's room.
use contracts::CoreError;
use reply_retrieval::{ExcerptJudge, QueryWriter};
use reply_voice::{VoiceRequest, VoiceWriter};

use super::provider::cut;
use crate::replies_work::model::{CallKind, ModelCall, TextModel};

pub(crate) struct Calls<'a>(pub &'a dyn TextModel);

impl Calls<'_> {
    pub(crate) fn call(&self, kind: CallKind, system: Option<String>, prompt: &str) -> Result<String, CoreError> {
        self.0.execute(&ModelCall { kind, system, prompt: cut(prompt, self.0.prompt_chars()) })
    }
}

impl QueryWriter for Calls<'_> {
    fn execute(&self, prompt: &str) -> Result<String, CoreError> { self.call(CallKind::Questions, None, prompt) }
}

impl ExcerptJudge for Calls<'_> {
    fn execute(&self, prompt: &str) -> Result<String, CoreError> { self.call(CallKind::Judge, None, prompt) }
}

/// The analysis request as one prompt: the crate's instruction, then each sample dated (no address: the crate sends none), newest first.
pub(crate) fn voice_prompt(request: &VoiceRequest) -> String {
    let mut out = format!("{}\nWrite in {}.\n", request.instruction, request.language);
    for (i, s) in request.samples.iter().enumerate() {
        out.push_str(&format!("\nEmail {} ({}):\n{}\n", i + 1, s.date_ms, s.text));
    }
    out
}

impl VoiceWriter for Calls<'_> {
    fn execute(&self, request: &VoiceRequest) -> Result<String, CoreError> { self.call(CallKind::Voice, None, &voice_prompt(request)) }
}
