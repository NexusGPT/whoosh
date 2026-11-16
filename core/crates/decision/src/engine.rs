//! The engine plugged into the smart inbox's "ask the model" hook (`smart_inbox::AskModel`), so arrival placement uses
//! the same engine and the same per-page switches: a page the owner switched off is never asked, and its list falls
//! through to its rules and default. The question wording is the smart-inbox lane's adapter (`ViaDecisionPort`).
use crate::outcome::Outcome;
use crate::settings::DecisionSettings;
use contracts::{DecisionPort, DecisionProviderKind};
use smart_inbox::{AskModel, DecisionPage, ModelAsk, ModelVerdict, ViaDecisionPort};

pub struct EngineAsk<'a> {
    pub settings: &'a DecisionSettings,
    pub engine: Option<&'a dyn DecisionPort>,
}

fn outcome_of(page: DecisionPage) -> Outcome {
    match page {
        DecisionPage::WhichTab => Outcome::WhichTab,
        DecisionPage::NeedsMyReply => Outcome::NeedsMyReply,
    }
}

impl AskModel for EngineAsk<'_> {
    fn execute(&self, ask: &ModelAsk<'_>) -> Option<ModelVerdict> {
        if self.settings.engine == DecisionProviderKind::Rules {
            return None;
        }
        let enabled = self.settings.page(outcome_of(ask.page)).is_some_and(|p| p.enabled);
        if !enabled {
            return None;
        }
        ViaDecisionPort(self.engine?).execute(ask)
    }
}
