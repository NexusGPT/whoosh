//! The one wire shape both engines take: TypeSafe Jev's `{state, model, questions}` request and its `answers` object
//! (retrieval-dossier-jev.md Need 3: the org's jev_client.py body line and reader functions, observed in code; the
//! vendor's own reference is not filed). The local runtime is handed the same JSON, so one reader serves both.
use crate::thread_context;
use contracts::{CoreError, DecisionAnswer, DecisionQuestion, DecisionState};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, HashMap};

/// The fields of the state in the order they are written, then the body text.
const STATE_FIELDS: &[(&str, &str)] = &[("from", "from"), ("to", "to"), ("cc", "cc"), ("subject", "subject"), ("list_header", "list header")];

pub fn question_id(q: &DecisionQuestion) -> &str {
    match q {
        DecisionQuestion::Choice { id, .. } | DecisionQuestion::Score { id, .. } | DecisionQuestion::Noul { id, .. } => id,
    }
}

/// The state as one text: "from: …\nto: …\n…\n\n<body>", then the rest of the thread when the state carries it (lane labels-r5), cut to
/// the Jev budget (`thread_context::jev_budget_chars`: the declared size of one piece of message text, the smaller of the two engines'
/// limits). `text` may be shorter than the state's (the local budget cuts it). A caller that knows its own budget uses
/// `state_text_with_thread`.
pub fn state_text(state: &DecisionState, text: &str) -> String {
    state_text_with_thread(state, text, thread_context::jev_budget_chars())
}

/// `state_text` with the thread cut to `thread_chars` characters, oldest messages first (0 = no thread).
pub fn state_text_with_thread(state: &DecisionState, text: &str, thread_chars: usize) -> String {
    let mut out = String::new();
    for (key, words) in STATE_FIELDS {
        if let Some(v) = state.fields.get(*key) {
            out.push_str(&format!("{words}: {v}\n"));
        }
    }
    out.push('\n');
    out.push_str(text);
    let thread = thread_context::text_of(&state.fields, thread_chars);
    if !thread.is_empty() {
        out.push_str("\n\n");
        out.push_str(&thread);
    }
    out
}

/// One question object. A choice's criteria name each option by itself (the contract carries no option descriptions:
/// see "Contract deltas"); a yes/no question's criteria are the two words.
pub fn question_json(q: &DecisionQuestion) -> Value {
    match q {
        DecisionQuestion::Choice { prompt, options, .. } => {
            let criteria: Map<String, Value> = options.iter().map(|o| (o.clone(), Value::String(o.clone()))).collect();
            json!({"type": "choice", "instructions": prompt, "criteria": criteria})
        }
        DecisionQuestion::Noul { prompt, .. } => json!({"type": "noul", "instructions": prompt, "criteria": {"true": "yes", "false": "no"}}),
        DecisionQuestion::Score { prompt, .. } => json!({"type": "score", "instructions": prompt}),
    }
}

pub fn request_body(state: &str, model: &str, questions: &[DecisionQuestion]) -> String {
    let qs: Map<String, Value> = questions.iter().map(|q| (question_id(q).to_string(), question_json(q))).collect();
    json!({"state": state, "model": model, "questions": qs}).to_string()
}

fn bad_shape(provider: &str, why: &str) -> CoreError {
    CoreError::Action { failure: contracts::ActionFailed { account_name: provider.into(), reason_words: format!("the answer had an unexpected shape: {why}"), retryable: false, code: None } }
}

/// Reads `answers`. Exactly the question ids sent must come back, or the whole answer is refused (jev_client.py's
/// shape check). A yes/no probability sits under `noul`; a choice carries `choice` and `confidence`; a score `score`.
pub fn read_answers(provider: &str, body: &str, questions: &[DecisionQuestion]) -> Result<Vec<DecisionAnswer>, CoreError> {
    let out: Value = serde_json::from_str(body).map_err(|e| bad_shape(provider, &format!("not JSON ({e})")))?;
    let answers = out.get("answers").and_then(Value::as_object).ok_or_else(|| bad_shape(provider, "no answers object"))?;
    let sent: BTreeSet<&str> = questions.iter().map(question_id).collect();
    let got: BTreeSet<&str> = answers.keys().map(String::as_str).collect();
    if sent != got {
        return Err(bad_shape(provider, &format!("asked {sent:?}, answered {got:?}")));
    }
    questions
        .iter()
        .map(|q| {
            let id = question_id(q).to_string();
            let a = &answers[question_id(q)];
            let num = |k: &str| a.get(k).and_then(Value::as_f64).ok_or_else(|| bad_shape(provider, &format!("{} has no number under {k}", question_id(q))));
            Ok(match q {
                DecisionQuestion::Noul { .. } => {
                    let p = num("noul")?;
                    // yes when yes is the more probable side
                    DecisionAnswer::Noul { id, yes: p > 1.0 - p, probability: p }
                }
                DecisionQuestion::Choice { .. } => {
                    let choice = a.get("choice").and_then(Value::as_str).ok_or_else(|| bad_shape(provider, &format!("{id} has no choice")))?.to_string();
                    let p = num("confidence")?;
                    DecisionAnswer::Choice { id, probabilities: HashMap::from([(choice.clone(), p)]), choice }
                }
                DecisionQuestion::Score { .. } => DecisionAnswer::Score { id, value: num("score")? },
            })
        })
        .collect()
}
