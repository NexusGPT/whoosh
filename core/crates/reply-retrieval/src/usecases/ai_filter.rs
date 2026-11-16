//! Step 6: the AI filter ("Filtre par l'IA"). Off when the question is None. On: the judge model reads the owner's question, the excerpt and
//! the message (cut to its budget) and answers yes or no, once per excerpt. A no drops the excerpt; an answer that is neither, or an error,
//! keeps it and is counted as not judged (a context the model could not judge is still context).
use crate::domain::limits::JUDGE_TEMPLATE;
use crate::domain::text::fill_within;
use crate::domain::{AiFilterTrace, DropReason, Dropped};
use crate::ports::ExcerptJudge;
use crate::usecases::gather::Candidate;

/// What the judge reads for one excerpt, never over `budget` characters: the question first in priority, then the excerpt, then the message.
pub fn judge_prompt(question: &str, excerpt: &str, message: &str, budget: usize) -> String {
    fill_within(JUDGE_TEMPLATE, &[("question", question), ("excerpt", excerpt), ("message", message)], budget).0
}

/// The answer's first word as yes or no (yes, oui, true; no, non, false; any case, quotes, stars and punctuation around it ignored).
pub fn read_yes_no(answer: &str) -> Option<bool> {
    let first = answer.split_whitespace().next()?.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    match first.as_str() {
        "yes" | "oui" | "true" => Some(true),
        "no" | "non" | "false" => Some(false),
        _ => None,
    }
}

/// Runs the filter over the excerpts, in their order. Returns the excerpts that stay and what the filter did.
pub fn ai_filter(judge: &dyn ExcerptJudge, question: Option<&str>, c: Vec<Candidate>, message: &str, budget: usize, dropped: &mut Vec<Dropped>) -> (Vec<Candidate>, AiFilterTrace) {
    let Some(question) = question else { return (c, AiFilterTrace::Off) };
    let (mut judged, mut no, mut unjudged) = (0, 0, 0);
    let mut kept = Vec::new();
    for cand in c {
        let answer = judge.execute(&judge_prompt(question, &cand.found.text, message, budget)).ok().and_then(|a| read_yes_no(&a));
        match answer {
            Some(true) => {
                judged += 1;
                kept.push(cand);
            }
            Some(false) => {
                judged += 1;
                no += 1;
                dropped.push(Dropped { chunk_id: cand.found.chunk_id, reason: DropReason::AiFilter });
            }
            None => {
                unjudged += 1;
                kept.push(cand);
            }
        }
    }
    (kept, AiFilterTrace::On { question: question.to_string(), judged, dropped: no, unjudged })
}
