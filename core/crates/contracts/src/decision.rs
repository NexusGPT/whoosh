//! The ONE decision contract (research/decision-model.md "## The one contract"): a state plus
//! typed questions (choice, score, noul) in, answers with probabilities out. Providers: local
//! (Laya in-process or a local systemone server), jev (the owner's key), rules (shipped).
use std::collections::HashMap;

use crate::{CoreError, DecisionProviderKind};

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct DecisionState { pub text: String, pub fields: HashMap<String, String> }

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum DecisionQuestion {
    Choice { id: String, prompt: String, options: Vec<String> },
    Score { id: String, prompt: String },
    /// yes / no
    Noul { id: String, prompt: String },
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum DecisionAnswer {
    Choice { id: String, choice: String, probabilities: HashMap<String, f64> },
    Score { id: String, value: f64 },
    Noul { id: String, yes: bool, probability: f64 },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct DecisionResult { pub answers: Vec<DecisionAnswer>, pub answered_by: DecisionProviderKind, pub checkpoint: Option<String> }

#[uniffi::export(with_foreign)]
pub trait DecisionPort: Send + Sync {
    fn provider(&self) -> DecisionProviderKind;
    fn decide(&self, state: DecisionState, questions: Vec<DecisionQuestion>) -> Result<DecisionResult, CoreError>;
}
