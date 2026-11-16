use contracts::CoreError;

use crate::domain::proposal::StoredProposal;

/// Stores a new, pending proposal.
pub trait SaveProposal: Send + Sync {
    fn execute(&self, proposal: StoredProposal) -> Result<(), CoreError>;
}
