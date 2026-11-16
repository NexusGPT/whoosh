//! "Analyser mes messages envoyés" (J10.3), on the background thread: reply-voice prepares the last `sample_size` sent messages (local), `reply/voice`
//! shows Running with the count it will read, ONE model call writes the tone text, which is stored with the count through
//! `reply_record_voice_analysis`; then the analysis is Idle again (`analysed_at_ms` says when it last ran). A failure shows Failed with its words and
//! stores nothing.
use reply_voice::{prepare_analysis, AnalysisRequest};

use super::words;
use crate::replies_work::adapters::mail_reads::MailReads;
use crate::replies_work::adapters::model_ports::Calls;
use crate::replies_work::limits::REPLY_VOICE_LANGUAGE;
use crate::replies_work::model::TextModel;
use crate::replies_work::Shared;
use crate::rust_core::now_ms;
use contracts::ReplyVoiceAnalysis;

fn set(s: &Shared, v: ReplyVoiceAnalysis) { *s.voice.lock().unwrap_or_else(|e| e.into_inner()) = v; }

pub(crate) fn run(s: &Shared, sample_size: u32, model: &dyn TextModel) {
    let reads = match MailReads::open(&s.store) { Ok(r) => r, Err(e) => return set(s, ReplyVoiceAnalysis::Failed { reason_words: words(&e) }) };
    // the prompt carries the crate's instruction and the dates around the samples: keep a tenth of the room for them
    let request = AnalysisRequest { sample_size, language: REPLY_VOICE_LANGUAGE.to_string(), budget_chars: model.prompt_chars() - model.prompt_chars() / 10 };
    let prepared = match prepare_analysis(&reads, &request) { Ok(p) => p, Err(f) => return set(s, f.into()) };
    set(s, prepared.running());
    let analysis = match prepared.write(&Calls(model)) { Ok(a) => a, Err(f) => { s.log(&format!("the voice analysis failed: {}", f.reason_words)); return set(s, f.into()) } };
    match s.store.reply_record_voice_analysis(analysis.read, now_ms(), Some(analysis.tone_text)) {
        Ok(()) => set(s, ReplyVoiceAnalysis::Idle),
        Err(e) => set(s, ReplyVoiceAnalysis::Failed { reason_words: format!("the voice was written but not kept: {}", words(&e)) }),
    }
}
