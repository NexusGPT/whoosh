//! `create_draft`: grant, then the owner's approval, then a draft saved in Whoosh. The answer says the draft is
//! unsent. This use case holds no send port, so no input can make it send.
use std::sync::Arc;

use serde_json::{json, Value};

use crate::args;
use crate::domain::{ApprovalAction, ApprovalRequest, McpError};
use crate::ports::DraftWriter;
use crate::usecases::gate::Gate;

pub struct CreateDraft {
    gate: Arc<Gate>,
    drafts: Arc<dyn DraftWriter>,
}

impl CreateDraft {
    pub fn new(gate: Arc<Gate>, drafts: Arc<dyn DraftWriter>) -> CreateDraft {
        CreateDraft { gate, drafts }
    }

    pub fn execute(&self, client: Option<&str>, arguments: &Value) -> Result<Value, McpError> {
        let name = self.gate.authorise(client)?;
        let draft = args::draft(arguments)?;
        self.gate.approve(&name, ApprovalRequest { client: name.clone(), action: ApprovalAction::Draft { to: draft.to.clone(), subject: draft.subject.clone() } })?;
        let id = self.drafts.execute(&draft)?;
        Ok(json!({ "draft_id": id, "sent": false, "note": "Saved as a draft in Whoosh. The owner reviews and sends it." }))
    }
}
