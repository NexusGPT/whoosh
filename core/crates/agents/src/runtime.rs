//! The agents API the FFI binds: three calls (ask, decide a proposal, give consent) over one set of ports.
//! `Command::Ask` maps to `ask`, `Command::ApproveProposal` to `decide_proposal`; the consent answer is the panel's
//! "Allow" button. Events go to `AgentPorts::events` while `ask` runs.
use std::sync::Arc;

use contracts::{CommandOutcome, CoreError};

use crate::ports::{AgentEvents, ApplyCommand, Clock, ConsentGiven, DecideProposal, ReadMail, RecordConsent, ResolveProvider, SaveProposal, SearchMail};
use crate::usecases::approve_proposal::ApproveProposal;
use crate::usecases::ask::{Ask, AskOutcome, AskRequest};
use crate::usecases::give_consent::GiveConsent;

#[derive(Clone)]
pub struct AgentPorts {
    pub providers: Arc<dyn ResolveProvider>,
    pub search: Arc<dyn SearchMail>,
    pub read: Arc<dyn ReadMail>,
    pub apply: Arc<dyn ApplyCommand>,
    pub consent_given: Arc<dyn ConsentGiven>,
    pub record_consent: Arc<dyn RecordConsent>,
    pub save_proposal: Arc<dyn SaveProposal>,
    pub decide_proposal: Arc<dyn DecideProposal>,
    pub events: Arc<dyn AgentEvents>,
    pub clock: Arc<dyn Clock>,
}

pub struct AgentRuntime {
    ports: AgentPorts,
    max_output_tokens: u32,
}

impl AgentRuntime {
    /// `max_output_tokens` is the answer ceiling the caller reads from the ledger; this crate holds no such number.
    pub fn new(ports: AgentPorts, max_output_tokens: u32) -> AgentRuntime {
        AgentRuntime { ports, max_output_tokens }
    }

    pub fn ask(&self, request: AskRequest) -> Result<AskOutcome, CoreError> {
        Ask::new(self.ports.clone(), self.max_output_tokens).execute(request)
    }

    pub fn decide_proposal(&self, proposal_id: &str, approve: bool) -> Result<CommandOutcome, CoreError> {
        ApproveProposal::new(self.ports.clone()).execute(proposal_id, approve)
    }

    pub fn give_consent(&self, provider_id: &str) -> Result<(), CoreError> {
        GiveConsent::new(self.ports.clone()).execute(provider_id)
    }
}
