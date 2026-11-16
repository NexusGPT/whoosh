//! The tone analysis (J10.3): the last `sample_size` sent messages become a short tone text. Two steps so the engine can say what it is doing:
//! `prepare_analysis` is local and fast (read the store, strip, cut to the budget) and tells how many messages it will read (`Running { read, of }`);
//! `PreparedAnalysis::write` is the slow model call, run in the background. `analyse_voice` does both.
use contracts::{check_voice_sample_size, CoreError, ReplyVoice, ReplyVoiceAnalysis};

use crate::clean::clean_sent_text;
use crate::ports::{SentMail, SentQuery, VoiceRequest, VoiceSample, VoiceWriter};

/// What one run is asked: how many sent messages (the pane's "100 derniers"), the app's language (an English name such as "French": the model
/// writes the tone in it, so the owner never reads a mix), and the model's room in characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRequest {
    pub sample_size: u32,
    pub language: String,
    pub budget_chars: usize,
}

/// Why an analysis did not produce a voice. The `kind` is a code the catalogue turns into the owner's language; `reason_words` is English and, for a
/// port failure, carries the REAL reason it gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceFailure {
    pub kind: VoiceFailureKind,
    pub reason_words: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceFailureKind {
    /// the sample size is 0 or above the ceiling (`check_voice_sample_size`)
    BadSample,
    /// no language was given
    BadLanguage,
    /// the store could not be read
    MailUnreadable,
    /// the Sent folder holds no message with words of the owner's own: no model call is made, no voice is invented
    NoSentMail,
    /// every message is longer than the model's room
    NothingFits,
    /// the model could not be reached or refused
    ModelFailed,
    /// the model answered with nothing
    ModelWroteNothing,
}

impl VoiceFailureKind {
    /// The refusal code, in the contract's style (`reply_ai.rs` refusal codes).
    pub fn code(self) -> &'static str {
        match self {
            Self::BadSample => contracts::REPLY_BAD_SAMPLE_CODE,
            Self::BadLanguage => "bad_voice_language",
            Self::MailUnreadable => "voice_mail_unreadable",
            Self::NoSentMail => "voice_no_sent_mail",
            Self::NothingFits => "voice_nothing_fits",
            Self::ModelFailed => "voice_model_failed",
            Self::ModelWroteNothing => "voice_model_wrote_nothing",
        }
    }
}

impl From<VoiceFailure> for ReplyVoiceAnalysis {
    fn from(f: VoiceFailure) -> Self {
        ReplyVoiceAnalysis::Failed { reason_words: f.reason_words }
    }
}

fn fail(kind: VoiceFailureKind, words: impl Into<String>) -> VoiceFailure {
    VoiceFailure { kind, reason_words: words.into() }
}

/// What a finished analysis gives: the tone text and the count really read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceAnalysis {
    pub tone_text: String,
    /// the sent messages that went to the model (whole messages that fit the budget)
    pub read: u32,
    /// the sent messages with words of the owner's own that were found (at most the sample size); `read` of them were read
    pub of: u32,
}

impl VoiceAnalysis {
    /// The stored voice after this run: the new text, the count really read (the contract's `ReplyVoice.sample_size`), the time, and the other
    /// setting as it was.
    pub fn into_voice(self, now_ms: i64, previous: &ReplyVoice) -> ReplyVoice {
        ReplyVoice { tone_text: self.tone_text, sample_size: self.read, analysed_at_ms: Some(now_ms), same_person_examples: previous.same_person_examples }
    }
}

/// An analysis whose messages are chosen and whose model call is still to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedAnalysis {
    /// messages that will be read
    pub read: u32,
    /// messages found
    pub of: u32,
    request: VoiceRequest,
}

impl PreparedAnalysis {
    /// The state to publish while the model works (`reply/voice/analysis`).
    pub fn running(&self) -> ReplyVoiceAnalysis {
        ReplyVoiceAnalysis::Running { read: self.read, of: self.of }
    }

    /// What the model will be sent.
    pub fn request(&self) -> &VoiceRequest {
        &self.request
    }

    /// The model call. A blank answer is a failure: an empty text is never stored as the owner's voice.
    pub fn write(self, writer: &dyn VoiceWriter) -> Result<VoiceAnalysis, VoiceFailure> {
        let answer = writer.execute(&self.request).map_err(|e| fail(VoiceFailureKind::ModelFailed, words_of(&e)))?;
        let tone_text = answer.trim().to_string();
        if tone_text.is_empty() {
            return Err(fail(VoiceFailureKind::ModelWroteNothing, "the model wrote no description of your voice"));
        }
        Ok(VoiceAnalysis { tone_text, read: self.read, of: self.of })
    }
}

fn words_of(e: &CoreError) -> String {
    match e {
        CoreError::Invalid { reason_words, .. } => reason_words.clone(),
        other => other.to_string(),
    }
}

/// The instruction the model gets. In English (the model reads it); the answer is asked in `language`, in the first person, as the Voix pane shows it
/// ("Je vouvoie les clients, je tutoie l'équipe.").
pub fn voice_instruction(language: &str) -> String {
    format!(
        "You read emails written by one person. Describe how this person writes, as three to five short plain sentences, one rule each, written in the \
         first person as that person would say it, so that someone else could write like them: how they greet, how formal they are (and whether they \
         use the informal or the formal \"you\"), how long they write, how they close, and any habit that comes back. Base every rule on what the \
         emails show; never invent a habit they do not show. Write the rules in {language}. Answer with the rules only."
    )
}

/// Reads the sent mail, strips it, cuts it to the budget, and builds what the model is asked. No model call: a refusal here (no sent mail) costs nothing.
///   - the sort is by date, newest first, whatever order the port used; at most `sample_size` messages are taken;
///   - a message with no words of the owner's own (a bare forward, a quote alone) is dropped;
///   - the cut walks newest to oldest and takes a message only when the WHOLE of it still fits the budget (so a message longer than what is left
///     is skipped and an older, shorter one may still be read); `read` is the count that went in, never the count asked for.
pub fn prepare_analysis(sent: &dyn SentMail, request: &AnalysisRequest) -> Result<PreparedAnalysis, VoiceFailure> {
    let sample = check_voice_sample_size(request.sample_size).map_err(|e| fail(VoiceFailureKind::BadSample, words_of(&e)))?;
    if request.language.trim().is_empty() {
        return Err(fail(VoiceFailureKind::BadLanguage, "the language of the tone is missing"));
    }
    let mut found = sent.execute(&SentQuery::Latest, sample as usize).map_err(|e| fail(VoiceFailureKind::MailUnreadable, words_of(&e)))?;
    found.sort_by(|a, b| b.date_ms.cmp(&a.date_ms));
    let own: Vec<VoiceSample> = found
        .into_iter()
        .take(sample as usize)
        .map(|m| VoiceSample { date_ms: m.date_ms, text: clean_sent_text(&m.text) })
        .filter(|s| !s.text.is_empty())
        .collect();
    if own.is_empty() {
        return Err(fail(VoiceFailureKind::NoSentMail, "there is no sent mail with your own words to learn from yet"));
    }
    let of = own.len() as u32;
    let mut room = request.budget_chars;
    let mut samples = Vec::new();
    for s in own {
        let len = s.text.chars().count();
        if len <= room {
            room -= len;
            samples.push(s);
        }
    }
    if samples.is_empty() {
        return Err(fail(VoiceFailureKind::NothingFits, "every sent message is longer than what the model can read"));
    }
    let read = samples.len() as u32;
    Ok(PreparedAnalysis {
        read,
        of,
        request: VoiceRequest { instruction: voice_instruction(&request.language), language: request.language.clone(), samples },
    })
}

/// `prepare_analysis`, then the model call.
pub fn analyse_voice(sent: &dyn SentMail, writer: &dyn VoiceWriter, request: &AnalysisRequest) -> Result<VoiceAnalysis, VoiceFailure> {
    prepare_analysis(sent, request)?.write(writer)
}
