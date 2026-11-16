//! Step 1: the questions. The retrieval prompt (the Recherche pane) with `{{message}}` and `{{instruction}}` filled, cut to the question model's
//! budget, goes to `QueryWriter`; its answer is read as at most `questions_max` questions. A model that fails, says nothing or writes nothing
//! that can be a question degrades to ONE search on the message subject, and the report says which of the three happened.
use crate::domain::limits::QUESTION_CHARS_MAX;
use crate::domain::text::{chars, fill_within, subject_without_prefixes, take_chars};
use crate::domain::{FallbackWhy, QuestionOrigin, ReplyMessage};
use crate::ports::QueryWriter;
use contracts::{strip_prompt_notes, RetrievalSettings, REPLY_QUESTIONS_CEILING};

/// The questions to search and where they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Questions {
    pub origin: QuestionOrigin,
    pub questions: Vec<String>,
    pub prompt_cut: bool,
}

/// The value of `{{message}}`: the subject, a blank line, the body (the subject alone when there is no body, the body alone when no subject).
pub fn message_value(m: &ReplyMessage) -> String {
    let (s, b) = (m.subject.trim(), m.body.trim());
    match (s.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (false, true) => s.to_string(),
        (false, false) => format!("{s}\n\n{b}"),
    }
}

/// What the question model reads: the prompt's `//` lines dropped, the instruction placed first in priority (the ask is kept when room is
/// short), then the message; never over `budget` characters. Returns the prompt and whether it was cut.
pub fn question_prompt(settings: &RetrievalSettings, message: &ReplyMessage, instruction: Option<&str>, budget: usize) -> (String, bool) {
    let template = strip_prompt_notes(settings.retrieval_prompt.clone());
    let msg = message_value(message);
    fill_within(&template, &[("instruction", instruction.unwrap_or("")), ("message", &msg)], budget)
}

/// A number of the Recherche pane as used: at least 1, at most its contract ceiling (the pane refuses both already; a stored row from
/// elsewhere cannot make retrieval ask for zero or a thousand).
pub fn clamp_count(n: u32, ceiling: u32) -> usize {
    n.clamp(1, ceiling) as usize
}

/// One line of the model's answer as a question: bullets, numbering ("1.", "2)", "Q3:") and surrounding quotes or stars removed. None when
/// what is left is empty, introduces a list (ends with ':'), holds no letter, or is longer than `QUESTION_CHARS_MAX`.
pub fn question_of_line(line: &str) -> Option<String> {
    let mut s = line.trim().trim_start_matches(|c: char| "-*•·–—#>".contains(c)).trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    if b.len() > 1 && (b[0] == b'q' || b[0] == b'Q') && b[1].is_ascii_digit() {
        i = 1;
    }
    let digits_from = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i > digits_from && i < b.len() && matches!(b[i], b'.' | b')' | b':') {
        s = s[i + 1..].trim_start();
    }
    let s = s.trim_matches(|c: char| "\"'«»“”*`,".contains(c) || c.is_whitespace());
    let ok = !s.is_empty() && !s.ends_with(':') && s.chars().any(char::is_alphabetic) && chars(s) <= QUESTION_CHARS_MAX;
    ok.then(|| s.to_string())
}

/// The model's answer as questions: one per line, or a JSON-like list `["...", "..."]`; each once (any case), at most `max`.
/// Err names why there is none: no text at all, or text with no usable line.
pub fn parse_questions(raw: &str, max: usize) -> Result<Vec<String>, FallbackWhy> {
    let t = raw.trim();
    if t.is_empty() {
        return Err(FallbackWhy::ModelSaidNothing);
    }
    let lines: Vec<&str> = if t.starts_with('[') && t.ends_with(']') { t[1..t.len() - 1].split("\",").collect() } else { t.lines().collect() };
    let mut out: Vec<String> = Vec::new();
    for q in lines.iter().filter_map(|l| question_of_line(l)) {
        if out.len() == max {
            break;
        }
        if !out.iter().any(|o| o.to_lowercase() == q.to_lowercase()) {
            out.push(q);
        }
    }
    if out.is_empty() {
        Err(FallbackWhy::ModelGarbage)
    } else {
        Ok(out)
    }
}

/// The search when the model gave nothing: the subject without Re:/Fwd:; with no subject, the start of the body. None when both are blank.
pub fn fallback_question(m: &ReplyMessage) -> Option<String> {
    let s = subject_without_prefixes(&m.subject);
    let q = if s.is_empty() { take_chars(m.body.trim().lines().next().unwrap_or("").trim(), QUESTION_CHARS_MAX) } else { take_chars(&s, QUESTION_CHARS_MAX) };
    (!q.trim().is_empty()).then(|| q.trim().to_string())
}

/// Step 1 whole: ask the model, read its answer, fall back to the subject when it gives nothing usable. Never fails.
pub fn write_questions(writer: &dyn QueryWriter, settings: &RetrievalSettings, message: &ReplyMessage, instruction: Option<&str>, budget: usize) -> Questions {
    let (prompt, prompt_cut) = question_prompt(settings, message, instruction, budget);
    let max = clamp_count(settings.questions_max, REPLY_QUESTIONS_CEILING);
    let parsed = match writer.execute(&prompt) {
        Ok(raw) => parse_questions(&raw, max),
        Err(e) => Err(FallbackWhy::ModelFailed { reason_words: e.to_string() }),
    };
    match parsed {
        Ok(questions) => Questions { origin: QuestionOrigin::Model, questions, prompt_cut },
        Err(why) => Questions { origin: QuestionOrigin::SubjectFallback { why }, questions: fallback_question(message).into_iter().collect(), prompt_cut },
    }
}
