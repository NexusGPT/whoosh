//! The gate every mail tool passes: the app holds a grant, and the owner approves THIS request. The grant is read
//! from the store on every call (revoking ends access at once) and read again after the owner answered, so a
//! revoke made while the prompt was open still wins. A grant store that fails closes the gate.
use std::sync::Arc;

use crate::domain::{ApprovalRequest, Decision, McpError};
use crate::ports::{Approver, GrantLookup};

pub struct Gate {
    grants: Arc<dyn GrantLookup>,
    approver: Arc<dyn Approver>,
}

impl Gate {
    pub fn new(grants: Arc<dyn GrantLookup>, approver: Arc<dyn Approver>) -> Gate {
        Gate { grants, approver }
    }

    /// Does the app hold an allowed grant right now? An unnamed client never does.
    pub fn authorise(&self, client: Option<&str>) -> Result<String, McpError> {
        let name = match client {
            Some(n) => n.to_string(),
            None => return Err(McpError::NotAuthorised { client: "This app".to_string() }),
        };
        match self.grants.execute(&name) {
            Ok(Some(grant)) if grant.allowed => Ok(name),
            // No row, a revoked row, or a store that failed: the gate stays closed.
            _ => Err(McpError::NotAuthorised { client: name }),
        }
    }

    /// Asks the owner, then checks the grant again. Returns only when both say yes.
    pub fn approve(&self, client: &str, request: ApprovalRequest) -> Result<(), McpError> {
        match self.approver.execute(&request) {
            Decision::Approved => self.authorise(Some(client)).map(|_| ()),
            Decision::Refused => Err(McpError::Refused { client: client.to_string() }),
        }
    }
}
