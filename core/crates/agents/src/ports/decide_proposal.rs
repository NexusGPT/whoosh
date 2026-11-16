use contracts::{CoreError, ProposalDecision};

use crate::domain::proposal::StoredProposal;

/// Moves a proposal's decision atomically and returns it as it now stands. The only moves allowed:
/// Pending to Approved or Rejected, and Approved back to Pending (the take-back when the approved command was refused).
/// Any other move is refused with `CoreError::Invalid`, which is what makes an approval apply exactly once.
pub trait DecideProposal: Send + Sync {
    fn execute(&self, proposal_id: &str, to: ProposalDecision) -> Result<StoredProposal, CoreError>;
}
