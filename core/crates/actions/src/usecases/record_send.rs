//! engine-act: a message queued in the outbox is one undo entry (`UndoAction::Send`), so Z inside the undo window cancels it
//! through the `SendCanceller` port. The outbox owns the entry; this crate owns the undo stack and its encoding.
use crate::engine::Env;
use crate::undo;
use contracts::{CoreError, UndoAction};

pub struct RecordSend {
    env: Env,
}

impl RecordSend {
    pub fn new(env: Env) -> RecordSend {
        RecordSend { env }
    }

    /// Pushes the entry and returns its id (the `undo_entry_id` the send answers with).
    pub fn execute(&self, outbox_id: String) -> Result<String, CoreError> {
        let now = self.env.now();
        Ok(self.env.store.write(move |ctx| undo::push(ctx, &UndoAction::Send { outbox_id }, now))?)
    }
}
