//! Lane fix-aisort round 3: how a FAKE model answers the Labels question by the question's form, shared by the decision and engine tests
//! (no production caller). The Labels model entry asks either ONE choice among the offered labels (id `labels/model`, ledger row
//! LABEL_AI_QUESTION absent) or one yes/no per offered label (id `labels/model#<label id>`, the statement `decide::label_statement`, row
//! `per-label`). A fake that "picks" label X at probability p answers: the choice X with {X: p}; or, per label, yes-probability p for X and
//! `FAKE_NO` for every other label. A fake that picks nothing answers "nothing fits" (an empty probability map), or `FAKE_NO` everywhere.
//! Both forms then reach the same verdict: X files when p reaches the trust level, nothing files otherwise.
use contracts::{DecisionAnswer, DecisionQuestion};

use crate::decide::{entry_of_question, statement_label, LABEL_QUESTION_SEP};
use crate::labels::LABELS_MODEL_ID;

/// A fake's yes-probability for a label it does not pick: under any trust level (a test value, not a product number).
pub const FAKE_NO: f64 = 0.05;

/// The label path a per-label question asks about; None for any other question.
pub fn asked_label(q: &DecisionQuestion) -> Option<&str> {
    match q {
        DecisionQuestion::Noul { id, prompt } if id.contains(LABEL_QUESTION_SEP) && entry_of_question(id) == LABELS_MODEL_ID => statement_label(prompt).map(|(p, _)| p),
        _ => None,
    }
}

/// The yes-probability a fake that picks `pick` at `p` gives the label `asked`.
pub fn yes_for(asked: &str, pick: Option<&str>, p: f64) -> f64 {
    if pick == Some(asked) { p } else { FAKE_NO }
}

/// The answer of a fake that picks `pick` (None: nothing fits) at `p`, in the form the Labels question took; None when `q` is not the
/// Labels question (the fake answers it its own way).
pub fn labels_answer(q: &DecisionQuestion, pick: Option<&str>, p: f64) -> Option<DecisionAnswer> {
    match q {
        DecisionQuestion::Choice { id, .. } if id == LABELS_MODEL_ID => Some(match pick {
            Some(c) => DecisionAnswer::Choice { id: id.clone(), choice: c.to_string(), probabilities: [(c.to_string(), p)].into_iter().collect() },
            None => DecisionAnswer::Choice { id: id.clone(), choice: "nothing fits".into(), probabilities: Default::default() },
        }),
        DecisionQuestion::Noul { id, .. } => asked_label(q).map(|label| {
            let yes = yes_for(label, pick, p);
            DecisionAnswer::Noul { id: id.clone(), yes: yes >= 0.5, probability: yes }
        }),
        _ => None,
    }
}

/// The label paths offered by the Labels question(s) among `questions`, in either form (the choice's options, or the per-label
/// statements in the order asked).
pub fn offered_labels(questions: &[DecisionQuestion]) -> Vec<String> {
    let choice = questions.iter().find_map(|q| match q {
        DecisionQuestion::Choice { id, options, .. } if id == LABELS_MODEL_ID => Some(options.clone()),
        _ => None,
    });
    choice.unwrap_or_else(|| questions.iter().filter_map(asked_label).map(str::to_string).collect())
}

/// The "<path>: <description>" lines the model read for the labels, in either form (the lines after the choice's sentence, or each
/// per-label statement read back).
pub fn label_meanings(questions: &[DecisionQuestion]) -> Vec<String> {
    let choice = questions.iter().find_map(|q| match q {
        DecisionQuestion::Choice { id, prompt, .. } if id == LABELS_MODEL_ID => Some(prompt.lines().skip(1).map(str::to_string).collect::<Vec<_>>()),
        _ => None,
    });
    choice.unwrap_or_else(|| {
        questions
            .iter()
            .filter_map(|q| match q {
                DecisionQuestion::Noul { prompt, .. } if asked_label(q).is_some() => statement_label(prompt).map(|(p, d)| format!("{p}: {d}")),
                _ => None,
            })
            .collect()
    })
}

/// Whether `questions` hold the Labels question in either form.
pub fn asks_labels(questions: &[DecisionQuestion]) -> bool {
    questions.iter().any(|q| matches!(q, DecisionQuestion::Choice { id, .. } if id == LABELS_MODEL_ID) || asked_label(q).is_some())
}
