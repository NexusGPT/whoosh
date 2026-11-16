//! In-memory books. Each holds its state behind one lock, so a decision move is atomic.
use std::collections::HashMap;
use std::sync::Mutex;

use contracts::{CoreError, ProposalDecision};

use crate::domain::proposal::StoredProposal;
use crate::ports::{ConsentGiven, DecideProposal, RecordConsent, SaveProposal};

fn poisoned(what: &str) -> CoreError {
    CoreError::Unexpected { reason_words: format!("the {what} book is unreadable after a crash") }
}

/// The owner's yes per provider, with when it was given.
#[derive(Default)]
pub struct MemoryConsent {
    given: Mutex<HashMap<String, i64>>,
}

impl ConsentGiven for MemoryConsent {
    /// An unreadable book counts as no: the owner is asked again, nothing leaves unasked.
    fn execute(&self, provider_id: &str) -> bool {
        self.given.lock().map(|g| g.contains_key(provider_id)).unwrap_or(false)
    }
}

impl RecordConsent for MemoryConsent {
    fn execute(&self, provider_id: &str, at_ms: i64) -> Result<(), CoreError> {
        self.given.lock().map_err(|_| poisoned("consent"))?.entry(provider_id.to_string()).or_insert(at_ms);
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryProposals {
    book: Mutex<HashMap<String, StoredProposal>>,
}

impl MemoryProposals {
    pub fn get(&self, proposal_id: &str) -> Option<StoredProposal> {
        self.book.lock().ok().and_then(|b| b.get(proposal_id).cloned())
    }
}

impl SaveProposal for MemoryProposals {
    fn execute(&self, proposal: StoredProposal) -> Result<(), CoreError> {
        let mut book = self.book.lock().map_err(|_| poisoned("proposal"))?;
        if book.contains_key(&proposal.proposal.id) {
            return Err(CoreError::Invalid { field: "proposal_id".into(), reason_words: format!("a proposal {} already exists", proposal.proposal.id) });
        }
        book.insert(proposal.proposal.id.clone(), proposal);
        Ok(())
    }
}

fn state_words(d: &ProposalDecision) -> &'static str {
    match d {
        ProposalDecision::Pending => "is still waiting",
        ProposalDecision::Approved { .. } => "was already approved",
        ProposalDecision::Rejected { .. } => "was already dismissed",
    }
}

impl DecideProposal for MemoryProposals {
    fn execute(&self, proposal_id: &str, to: ProposalDecision) -> Result<StoredProposal, CoreError> {
        let mut book = self.book.lock().map_err(|_| poisoned("proposal"))?;
        let stored = book.get_mut(proposal_id).ok_or_else(|| CoreError::NotFound { kind: "proposal".into(), id: proposal_id.to_string() })?;
        let allowed = matches!(
            (&stored.proposal.decision, &to),
            (ProposalDecision::Pending, ProposalDecision::Approved { .. })
                | (ProposalDecision::Pending, ProposalDecision::Rejected { .. })
                | (ProposalDecision::Approved { .. }, ProposalDecision::Pending)
        );
        if !allowed {
            return Err(CoreError::Invalid { field: "proposal".into(), reason_words: format!("this proposal {}", state_words(&stored.proposal.decision)) });
        }
        stored.proposal.decision = to;
        Ok(stored.clone())
    }
}
