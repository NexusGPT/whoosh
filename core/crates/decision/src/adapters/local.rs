//! Local: a downloaded decision model (such as Laya) run on this Mac through the contract's `LocalModel`. Nothing leaves
//! the device. The runtime is handed the Jev-shaped request as text and answers the Jev-shaped `answers` object, so one
//! reader serves both engines. Every question is cut to Laya's option budget, and the state to the room the largest
//! question leaves in the window.
use crate::adapters::laya_budget::{cut_to_tokens, estimate_tokens, fit, window, LayaShape};
use crate::adapters::wire::{read_answers, request_body, state_text_with_thread};
use crate::thread_context::{chars_for_tokens, THREAD_FIELD};
use contracts::{CoreError, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionResult, DecisionState, LocalModel};
use std::sync::Arc;

const NAME: &str = "On this Mac";

pub struct LocalDecision {
    pub model: Arc<dyn LocalModel>,
    /// The model id, or the model file's path (`DecisionModel.model`).
    pub model_id: String,
    pub shape: LayaShape,
}

fn kind_word(q: &DecisionQuestion) -> &'static str {
    match q {
        DecisionQuestion::Choice { .. } => "choice",
        DecisionQuestion::Score { .. } => "score",
        DecisionQuestion::Noul { .. } => "noul",
    }
}

/// The questions cut to the budget, and the room left for the state (the smallest room over all questions).
pub fn budgeted(questions: &[DecisionQuestion], shape: LayaShape) -> (Vec<DecisionQuestion>, usize) {
    let mut room = window(shape);
    let cut = questions
        .iter()
        .map(|q| {
            let (prompt, options) = match q {
                DecisionQuestion::Choice { prompt, options, .. } => (prompt, options.clone()),
                DecisionQuestion::Score { prompt, .. } | DecisionQuestion::Noul { prompt, .. } => (prompt, vec![]),
            };
            let f = fit(kind_word(q), prompt, &options, shape);
            room = room.min(f.state_room);
            match q {
                DecisionQuestion::Choice { id, .. } => DecisionQuestion::Choice { id: id.clone(), prompt: f.instructions, options: f.options },
                DecisionQuestion::Score { id, .. } => DecisionQuestion::Score { id: id.clone(), prompt: f.instructions },
                DecisionQuestion::Noul { id, .. } => DecisionQuestion::Noul { id: id.clone(), prompt: f.instructions },
            }
        })
        .collect();
    (cut, room)
}

impl DecisionPort for LocalDecision {
    fn provider(&self) -> DecisionProviderKind {
        DecisionProviderKind::Local
    }

    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError> {
        let (cut, room) = budgeted(&questions, self.shape);
        // The body takes the room the largest question leaves; the rest of the thread (lane labels-r5) takes what the body leaves, oldest
        // messages first dropped (`thread_context::fit`), so the thread never costs the body its place. The whole text handed over
        // (fields, questions, the JSON around them) never passes the window: cut what is over, the thread first and then the body,
        // until it fits.
        let w = window(self.shape);
        let head = state_text_with_thread(&state, "", 0);
        let body_room = room.saturating_sub(estimate_tokens(&head));
        let mut text = cut_to_tokens(&state.text, body_room);
        let mut thread_chars = if state.fields.contains_key(THREAD_FIELD) { chars_for_tokens(body_room.saturating_sub(estimate_tokens(&text))) } else { 0 };
        let mut prompt = request_body(&state_text_with_thread(&state, &text, thread_chars), &self.model_id, &cut);
        while estimate_tokens(&prompt) > w && (thread_chars > 0 || !text.is_empty()) {
            let over = estimate_tokens(&prompt) - w;
            if thread_chars > 0 {
                thread_chars = thread_chars.saturating_sub(chars_for_tokens(over.max(1)).max(1));
            } else {
                text = cut_to_tokens(&text, estimate_tokens(&text).saturating_sub(over.max(1)));
            }
            prompt = request_body(&state_text_with_thread(&state, &text, thread_chars), &self.model_id, &cut);
        }
        let max_tokens = w.saturating_sub(estimate_tokens(&prompt)) as u32;
        let out = self.model.generate(prompt, max_tokens)?;
        // A choice answer names the option as it was SENT (possibly cut); map it back to the full option.
        let mut answers = read_answers(NAME, &out, &cut)?;
        for (a, (full, sent)) in answers.iter_mut().zip(questions.iter().zip(&cut)) {
            if let (contracts::DecisionAnswer::Choice { choice, probabilities, .. }, DecisionQuestion::Choice { options: full_opts, .. }, DecisionQuestion::Choice { options: sent_opts, .. }) = (a, full, sent) {
                if let Some(i) = sent_opts.iter().position(|o| o == choice) {
                    let p = probabilities.remove(choice.as_str()).unwrap_or(0.0);
                    *choice = full_opts[i].clone();
                    probabilities.insert(choice.clone(), p);
                }
            }
        }
        Ok(DecisionResult { answers, answered_by: DecisionProviderKind::Local, checkpoint: Some(self.model_id.clone()) })
    }
}
