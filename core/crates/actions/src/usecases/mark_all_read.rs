//! Mark every unread row of one inbox view read (a tab, optionally one account or label).
use super::mark_read::MarkRead;
use crate::engine::Env;
use contracts::{CommandOutcome, CoreError, InboxQuery};

pub struct MarkAllRead {
    env: Env,
}

impl MarkAllRead {
    pub fn new(env: Env) -> MarkAllRead {
        MarkAllRead { env }
    }

    pub fn execute(&self, query: InboxQuery) -> Result<CommandOutcome, CoreError> {
        let unread = self.env.store.unread_ids(&query)?;
        if unread.is_empty() {
            return Ok(CommandOutcome { undo_entry_id: None, created_id: None });
        }
        MarkRead::new(self.env.clone()).execute(unread, true)
    }
}
