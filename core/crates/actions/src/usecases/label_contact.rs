//! Label every message from a contact (all its addresses) in one action, with the same rules as the label use case.
use super::label::Label;
use crate::engine::Env;
use contracts::{CommandOutcome, CoreError};

pub struct LabelContact {
    env: Env,
}

impl LabelContact {
    pub fn new(env: Env) -> LabelContact {
        LabelContact { env }
    }

    pub fn execute(&self, contact_id: String, label_name: String, create_if_missing: bool) -> Result<CommandOutcome, CoreError> {
        let id = contact_id.clone();
        let ids: Vec<String> = self.env.store.write(move |ctx| ctx.contact_message_ids(&id))?
            .ok_or(CoreError::NotFound { kind: "contact".into(), id: contact_id })?;
        if ids.is_empty() {
            return Ok(CommandOutcome { undo_entry_id: None, created_id: None });
        }
        Label::new(self.env.clone()).execute(ids, label_name, create_if_missing)
    }
}
