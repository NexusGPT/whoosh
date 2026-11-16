//! The stream a running agent reports on, in order. The FFI maps each event to a contract `CoreEvent` where one exists.
use contracts::{ActionFailed, AgentProposal, CoreEvent};

use super::answer::AgentAnswer;

#[derive(Debug, Clone, PartialEq)]
pub enum AgentEvent {
    /// Nothing was sent: the owner is asked whether mail excerpts may go to this provider. Fires once per provider.
    ConsentNeeded { provider_id: String, provider_name: String },
    /// One model call finished; `words` says what the agent did with it ("Searched your mail for “quote”").
    Step { number: u32, words: String },
    /// Part of the answer text, in order; the chunks joined are the answer's text.
    Chunk { text: String },
    /// A proposal is stored and waits for the owner. Nothing has changed.
    ProposalReady { proposal: AgentProposal },
    /// The run ended with this answer.
    Answered { answer: AgentAnswer },
    /// The run hit AGENT_MAX_STEPS without an answer.
    Stopped { steps: u32, words: String },
    /// The provider refused or could not be reached: its own words.
    Failed { failure: ActionFailed },
}

impl AgentEvent {
    /// The contract event the panel listens to, where the contract has one (docs/CAPABILITY-CONTRACT.md "Core events").
    /// Answered has no arm: the answer is read from the finished turn (delta 7 added the other three).
    pub fn core_event(&self, conversation_id: &str) -> Option<CoreEvent> {
        match self {
            AgentEvent::Chunk { text } => Some(CoreEvent::AgentDelta { conversation_id: conversation_id.to_string(), text: text.clone() }),
            AgentEvent::ProposalReady { proposal } => Some(CoreEvent::ProposalReady { proposal_id: proposal.id.clone() }),
            AgentEvent::Failed { failure } => Some(CoreEvent::ActionFailed { failure: failure.clone() }),
            AgentEvent::ConsentNeeded { provider_id, provider_name } => Some(CoreEvent::AgentConsentNeeded {
                conversation_id: conversation_id.to_string(),
                provider_id: provider_id.clone(),
                provider_name: provider_name.clone(),
            }),
            AgentEvent::Step { number, words } => Some(CoreEvent::AgentStep { conversation_id: conversation_id.to_string(), number: *number, words: words.clone() }),
            AgentEvent::Stopped { steps, words } => Some(CoreEvent::AgentStopped { conversation_id: conversation_id.to_string(), steps: *steps, words: words.clone() }),
            AgentEvent::Answered { .. } => None,
        }
    }
}
