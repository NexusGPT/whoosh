//! The one port a model entry plugs into. The decision lane implements it over its providers; here it is a trait with
//! one method, a fake in the tests, and an adapter over the contract's `DecisionPort`.
use crate::facts::MessageFacts;
use crate::list::DecisionPage;
use contracts::{DecisionAnswer, DecisionPort, DecisionProviderKind, DecisionQuestion, DecisionState};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum AskKind {
    /// Pick one of these (tab names, or the category of a tab the owner added).
    Choice { options: Vec<String> },
    YesNo,
}

pub struct ModelAsk<'a> {
    pub page: DecisionPage,
    /// The question in words: exactly what is sent, so editing the sentence edits the prompt.
    pub question: &'a str,
    pub kind: AskKind,
    pub facts: &'a MessageFacts,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelAnswer {
    Choice(String),
    Yes(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelVerdict {
    pub answer: ModelAnswer,
    /// Confidence in THAT answer, 0..=1. Below the entry's trust level the list continues.
    pub probability: f64,
    pub by: DecisionProviderKind,
}

pub trait AskModel {
    /// None = the engine is off, unavailable or has no answer: the list falls through.
    fn execute(&self, ask: &ModelAsk<'_>) -> Option<ModelVerdict>;
}

/// Engine "Off": every model entry is skipped.
pub struct NoModel;
impl AskModel for NoModel {
    fn execute(&self, _ask: &ModelAsk<'_>) -> Option<ModelVerdict> {
        None
    }
}

/// Adapter over the contract's `DecisionPort` (local or Jev). Sends only what DECISION-MODEL.md lists: from, to, cc,
/// subject, the first DECISION_BODY_CHARS of the body, whether a list header is present.
pub struct ViaDecisionPort<'a>(pub &'a dyn DecisionPort);

impl AskModel for ViaDecisionPort<'_> {
    fn execute(&self, ask: &ModelAsk<'_>) -> Option<ModelVerdict> {
        let f = ask.facts;
        let mut fields = HashMap::new();
        fields.insert("from".to_string(), f.from_address.clone());
        fields.insert("to".to_string(), f.to.join(", "));
        fields.insert("cc".to_string(), f.cc.join(", "));
        fields.insert("subject".to_string(), f.subject.clone());
        fields.insert("list_header".to_string(), if f.has_list_header { "yes" } else { "no" }.to_string());
        let text: String = f.body.chars().take(crate::tuning::body_chars()).collect();
        let id = match ask.page {
            DecisionPage::WhichTab => "which-tab",
            DecisionPage::NeedsMyReply => "needs-reply",
        }
        .to_string();
        let question = match &ask.kind {
            AskKind::Choice { options } => DecisionQuestion::Choice { id: id.clone(), prompt: ask.question.to_string(), options: options.clone() },
            AskKind::YesNo => DecisionQuestion::Noul { id: id.clone(), prompt: ask.question.to_string() },
        };
        let result = self.0.decide(DecisionState { text, fields }, vec![question]).ok()?;
        let by = result.answered_by;
        result.answers.into_iter().find_map(|a| match a {
            DecisionAnswer::Choice { id: aid, choice, probabilities } if aid == id => {
                let probability = probabilities.get(&choice).copied().unwrap_or(0.0);
                Some(ModelVerdict { answer: ModelAnswer::Choice(choice), probability, by })
            }
            DecisionAnswer::Noul { id: aid, yes, probability } if aid == id => {
                // The provider's number is the yes-probability; the confidence in the answer given is its own side.
                Some(ModelVerdict { answer: ModelAnswer::Yes(yes), probability: if yes { probability } else { 1.0 - probability }, by })
            }
            _ => None,
        })
    }
}
