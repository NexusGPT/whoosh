//! Whoosh actions: the use cases that change mail. Each one commits in the store first (the owner sees the result within
//! COMMIT_MS), then writes back through the `MailAdapter` port, and on a refusal takes the change back and answers with
//! `CoreError::Action` naming the account and the reason in words. Archive, snooze, pin, label, move and a send push an
//! undo entry; Z walks them last in, first out.
//!
//! Third-party code: none beyond the store's rusqlite (MIT, bundled SQLite). Nothing is copied.

pub mod constants;
mod engine;
pub mod port;
mod undo;
pub mod usecases;

pub use engine::Env;
pub use port::{ContactsDirectory, DeviceContact, MailAdapter, MessageRef, PermissionDenied, Refusal, SendCanceller, WriteBack, WriteBackLedger};
pub use usecases::archive::Archive;
pub use usecases::build_contacts::BuildContacts;
pub use usecases::empty_trash_and_spam::EmptyTrashAndSpam;
pub use usecases::label::Label;
pub use usecases::label_contact::LabelContact;
pub use usecases::mark_all_read::MarkAllRead;
pub use usecases::mark_read::MarkRead;
pub use usecases::merge_contact::MergeContact;
pub use usecases::move_to_account::MoveToAccount;
pub use usecases::move_to_label::MoveToLabel;
pub use usecases::mute::Mute;
pub use usecases::pin::Pin;
pub use usecases::save_draft::SaveDraft;
pub use usecases::record_send::RecordSend;
pub use usecases::delete_draft::DeleteDraft;
pub use usecases::send_again::SendAgain;
pub use usecases::snooze::Snooze;
pub use usecases::spam::Spam;
pub use usecases::star::Star;
pub use usecases::trash::Trash;
pub use usecases::undo::Undo;
pub use usecases::unsubscribe::Unsubscribe;

use contracts::{Command, CommandOutcome, CoreError};

/// The command side of the core: `run` takes the Command variants this crate owns.
#[derive(Clone)]
pub struct Actions {
    env: Env,
}

impl Actions {
    pub fn new(env: Env) -> Actions {
        Actions { env }
    }

    pub fn env(&self) -> &Env {
        &self.env
    }

    pub fn run(&self, command: Command) -> Result<CommandOutcome, CoreError> {
        let env = self.env.clone();
        match command {
            Command::Archive { message_ids } => Archive::new(env).execute(message_ids),
            Command::Snooze { message_ids, when } => Snooze::new(env).execute(message_ids, when),
            Command::Pin { message_id, pinned } => Pin::new(env).execute(message_id, pinned),
            Command::Label { message_ids, label_name, create_if_missing } => Label::new(env).execute(message_ids, label_name, create_if_missing),
            Command::MoveToAccount { message_ids, account_id } => MoveToAccount::new(env).execute(message_ids, account_id),
            Command::MarkRead { message_ids, read } => MarkRead::new(env).execute(message_ids, read),
            Command::Undo => Undo::new(env).execute(),
            Command::Star { message_ids, starred } => Star::new(env).execute(message_ids, starred),
            Command::Mute { message_ids, muted } => Mute::new(env).execute(message_ids, muted),
            Command::MarkSpam { message_ids } => Spam::new(env).execute(message_ids),
            Command::MarkNotSpam { message_ids } => Spam::new(env).not_spam(message_ids),
            Command::SaveDraft { from_account_id, to, cc, subject, body, reply_to_thread_id } => SaveDraft::new(env).execute(from_account_id, to, cc, subject, body, reply_to_thread_id),
            Command::UpdateDraft { draft_id, from_account_id, to, cc, bcc, subject, body } => SaveDraft::new(env).update(draft_id, from_account_id, to, cc, bcc, subject, body),
            Command::DeleteDraft { draft_id } => DeleteDraft::new(env).execute(draft_id),
            _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "another part of the core handles this command".into() }),
        }
    }
}
