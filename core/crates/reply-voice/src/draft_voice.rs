//! The voice of one draft (J10.8, J10.9): which tone text, and which of the owner's earlier replies go with it as examples.
use contracts::{check_tone_choice, CoreError, ReplyTone, ReplyUse, ReplyVoice, REPLY_TONE_VOICE_ID};

use crate::clean::{clean_sent_text, cut_to_chars};
use crate::limits::VoiceLimits;
use crate::ports::{SentMail, SentMessage, SentQuery};

/// Who the draft answers and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftContext<'a> {
    pub thread_id: &'a str,
    /// the address the reply goes to
    pub recipient: &'a str,
}

/// One earlier reply of the owner's, stripped of quotes and signature and cut to the example budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceExample {
    pub date_ms: i64,
    pub text: String,
}

/// Where the examples come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExamplesFrom {
    /// the owner's replies in this thread (`match_thread` on, and the owner already wrote in it)
    Thread,
    /// the owner's earlier replies to the same person
    SamePerson,
    /// none: the setting is 0, or the owner never wrote there
    Nothing,
}

/// What the writer is handed about how the owner writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftVoice {
    pub tone_id: String,
    /// "Ma voix": what the analysis wrote (and the owner may have edited), EMPTY when never analysed and never written: nothing is invented.
    /// Any other tone id: that tone's text.
    pub tone_text: String,
    pub examples_from: ExamplesFrom,
    /// newest first
    pub examples: Vec<VoiceExample>,
}

/// The voice for one draft.
///   - tone: `settings.tone_id` is "voice" (the voice's text) or the id of one of `tones` (its text); any other id is `CoreError::NotFound`;
///   - examples, at most `voice.same_person_examples` (0 = none, and the store is not asked): when `settings.match_thread` is on and the owner already
///     wrote in this thread, those replies INSTEAD; else the owner's earlier replies to `context.recipient`;
///   - every example is stripped, dropped when nothing of the owner's own is left, cut to `limits.example_chars`, newest first;
///   - the store's answer is filtered again (the thread, the recipient), so an adapter that returns too much cannot put another person's words in.
pub fn voice_for_draft(
    settings: &ReplyUse,
    voice: &ReplyVoice,
    tones: &[ReplyTone],
    context: &DraftContext<'_>,
    sent: &dyn SentMail,
    limits: &VoiceLimits,
) -> Result<DraftVoice, CoreError> {
    check_tone_choice(settings.tone_id.clone(), tones.to_vec())?;
    let tone_text = if settings.tone_id == REPLY_TONE_VOICE_ID {
        voice.tone_text.trim().to_string()
    } else {
        tones.iter().find(|t| t.id == settings.tone_id).map(|t| t.text.trim().to_string()).unwrap_or_default()
    };
    let wanted = voice.same_person_examples as usize;
    let mut from = ExamplesFrom::Nothing;
    let mut examples = Vec::new();
    if wanted > 0 {
        if settings.match_thread {
            let in_thread = sent.execute(&SentQuery::InThread { thread_id: context.thread_id.to_string() }, wanted)?;
            examples = pick(in_thread.into_iter().filter(|m| m.thread_id == context.thread_id).collect(), wanted, limits);
            if !examples.is_empty() {
                from = ExamplesFrom::Thread;
            }
        }
        if from == ExamplesFrom::Nothing {
            let to_person = sent.execute(&SentQuery::ToRecipient { recipient: context.recipient.to_string() }, wanted)?;
            let me = context.recipient.trim();
            let theirs = to_person.into_iter().filter(|m| m.recipients.iter().any(|r| r.trim().eq_ignore_ascii_case(me))).collect();
            examples = pick(theirs, wanted, limits);
            if !examples.is_empty() {
                from = ExamplesFrom::SamePerson;
            }
        }
    }
    Ok(DraftVoice { tone_id: settings.tone_id.clone(), tone_text, examples_from: from, examples })
}

fn pick(mut messages: Vec<SentMessage>, wanted: usize, limits: &VoiceLimits) -> Vec<VoiceExample> {
    messages.sort_by(|a, b| b.date_ms.cmp(&a.date_ms));
    messages
        .into_iter()
        .map(|m| VoiceExample { date_ms: m.date_ms, text: clean_sent_text(&m.text) })
        .filter(|e| !e.text.is_empty())
        .take(wanted)
        .map(|e| VoiceExample { text: cut_to_chars(&e.text, limits.example_chars), ..e })
        .collect()
}
