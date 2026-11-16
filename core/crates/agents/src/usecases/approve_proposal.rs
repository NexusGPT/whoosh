//! Approve or dismiss a proposal (Command::ApproveProposal; C-J5.6: nothing changes before approve).
//! The approval is persisted BEFORE the command runs: two approvals race on the store's atomic move, one wins, and only
//! the winner applies. A refused command puts the proposal back to pending and returns the refusal's own words.
use contracts::{CommandOutcome, CoreError, ProposalDecision};

use crate::runtime::AgentPorts;

pub struct ApproveProposal {
    ports: AgentPorts,
}

impl ApproveProposal {
    pub fn new(ports: AgentPorts) -> ApproveProposal {
        ApproveProposal { ports }
    }

    pub fn execute(&self, proposal_id: &str, approve: bool) -> Result<CommandOutcome, CoreError> {
        let at_ms = self.ports.clock.execute();
        if !approve {
            self.ports.decide_proposal.execute(proposal_id, ProposalDecision::Rejected { at_ms })?;
            return Ok(CommandOutcome { undo_entry_id: None, created_id: None });
        }
        let claimed = self.ports.decide_proposal.execute(proposal_id, ProposalDecision::Approved { at_ms })?;
        match self.ports.apply.execute(claimed.command) {
            Ok(outcome) => Ok(outcome),
            Err(refused) => match self.ports.decide_proposal.execute(proposal_id, ProposalDecision::Pending) {
                Ok(_) => Err(refused),
                Err(stuck) => Err(CoreError::Unexpected { reason_words: format!("{refused}; the proposal could not be put back to waiting: {stuck}") }),
            },
        }
    }
}
