//! Empty Trash and Spam in one action. A delete cannot be taken back, so this is the one use case that asks the server
//! FIRST and deletes the local rows only for an account whose server said yes. A refusal leaves that account's rows.
use crate::constants::{SPAM_FOLDER, TRASH_FOLDER};
use crate::engine::Env;
use crate::port::WriteBack;
use contracts::CoreError;

pub struct EmptyTrashAndSpam {
    env: Env,
}

impl EmptyTrashAndSpam {
    pub fn new(env: Env) -> EmptyTrashAndSpam {
        EmptyTrashAndSpam { env }
    }

    /// Returns how many messages were deleted. `account_id` None = every account.
    pub fn execute(&self, account_id: Option<String>) -> Result<usize, CoreError> {
        let only = account_id.clone();
        let accounts: Vec<String> = self.env.store.write(move |ctx| ctx.accounts_with_mail_in(&[TRASH_FOLDER, SPAM_FOLDER], only.as_deref()))?;
        let mut deleted = 0;
        for account in accounts {
            self.env
                .adapter
                .execute(WriteBack::EmptyFolders { account_id: account.clone(), folders: vec![TRASH_FOLDER.into(), SPAM_FOLDER.into()] })
                .map_err(|r| self.env.failure(&r))?;
            deleted += self.env.store.write(move |ctx| Ok(ctx.delete_folder_messages(&account, &[TRASH_FOLDER, SPAM_FOLDER])?.len()))?;
        }
        Ok(deleted)
    }
}
