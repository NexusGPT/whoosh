//! Send again: a sent message becomes a new draft with the same recipients, subject, text and attachments.
//! The composer opens it; Send happens through the outbox as for any draft.
use crate::engine::{load, new_id, Env};
use store::StoreError;
use contracts::{CommandOutcome, CoreError};

pub struct SendAgain {
    env: Env,
}

impl SendAgain {
    pub fn new(env: Env) -> SendAgain {
        SendAgain { env }
    }

    pub fn execute(&self, message_id: String) -> Result<CommandOutcome, CoreError> {
        let now = self.env.now();
        let made = self.env.store.write(move |ctx| {
            if let Err(e) = load(ctx, std::slice::from_ref(&message_id))? {
                return Ok(Err(e));
            }
            match ctx.draft_from_sent(&message_id, &new_id("draft"), now) {
                Ok(id) => Ok(Ok(id)),
                Err(StoreError::Invalid(words)) => Ok(Err(CoreError::Invalid { field: "message_id".into(), reason_words: words })),
                Err(other) => Err(other),
            }
        })??;
        Ok(CommandOutcome { undo_entry_id: None, created_id: Some(made) })
    }
}
