//! A proposal: the contract record the panel shows, plus the exact command approval runs.
use contracts::{AgentProposal, Command};

#[derive(Debug, Clone, PartialEq)]
pub struct StoredProposal {
    pub proposal: AgentProposal,
    /// What runs on approve, once. The contract record cannot hold a draft id, so the command is kept beside it.
    pub command: Command,
}
