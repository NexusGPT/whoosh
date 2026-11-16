//! engine-act: the real core runs the mail actions through the actions crate (it was written for this and is tested on the store), and
//! the sends through the outbox. This file holds which command goes where and the few answers the engine adds: Undo answers the id
//! of the entry it reversed (as the fixture core does), a send answers its outbox entry and pushes the undo entry Z cancels with.
//! Every action commits locally first; its server write-back is a `pending_action` row of the same transaction (writeback.rs),
//! applied by the worker this file kicks after the commit.
use std::sync::{Arc, OnceLock};

use actions::{Actions, Env, RecordSend};
use contracts::{Clock, Command, CommandOutcome, CoreError, Draft};
use outbox::LoadDraftPort;
use store::Store;

use crate::outbox_tick::{quick_reply_draft, MailOut};
use crate::writeback::Worker;

/// The wall clock with this Mac's offset from UTC (read once, from `date +%z`: no time-zone crate in the core). Snooze words
/// ("tomorrow morning") resolve with it.
pub struct SystemClock;
fn local_offset_s() -> i32 {
    static OFF: OnceLock<i32> = OnceLock::new();
    *OFF.get_or_init(|| {
        let out = std::process::Command::new("/bin/date").arg("+%z").output().ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
        let t = out.trim();
        if t.len() != 5 { return 0; }
        let sign = if t.starts_with('-') { -1 } else { 1 };
        let h: i32 = t[1..3].parse().unwrap_or(0);
        let m: i32 = t[3..5].parse().unwrap_or(0);
        sign * (h * 3600 + m * 60)
    })
}
impl Clock for SystemClock {
    fn now_ms(&self) -> i64 { crate::rust_core::now_ms() }
    fn utc_offset_s(&self) -> i32 { local_offset_s() }
}

pub struct Act {
    pub actions: Actions,
    pub worker: Arc<Worker>,
    pub mail: Arc<MailOut>,
    pub imap: Arc<crate::writeback::ImapWriteBack>,
}

/// The commands the actions crate answers here. Mute stays with engine-actions' own arm (mute.rs), which runs before this.
pub fn is_action(c: &Command) -> bool {
    matches!(c,
        Command::Archive { .. } | Command::Snooze { .. } | Command::Pin { .. } | Command::Label { .. } | Command::MoveToAccount { .. }
        | Command::MarkRead { .. } | Command::Undo | Command::Star { .. } | Command::MarkSpam { .. } | Command::MarkNotSpam { .. } | Command::SaveDraft { .. } | Command::UpdateDraft { .. } | Command::DeleteDraft { .. })
}

/// The commands the outbox answers here.
pub fn is_send(c: &Command) -> bool {
    matches!(c, Command::Send { .. } | Command::QuickSend { .. } | Command::QuickSendFrom { .. } | Command::Schedule { .. } | Command::SendNow { .. } | Command::CancelSend { .. })
}

fn ok(created: Option<String>, undo: Option<String>) -> CommandOutcome { CommandOutcome { undo_entry_id: undo, created_id: created } }

impl Act {
    /// The hidden check's mutant `act-never-writes-back` (rig only).
    pub fn never_write_back(&self, never: bool) { self.imap.set_never(never) }
    pub fn env(&self) -> &Env { self.actions.env() }
    fn store(&self) -> &Arc<Store> { &self.env().store }

    /// The newest undo entry not yet undone: the one `Undo` reverses.
    pub(crate) fn newest_undo(&self) -> Option<String> {
        self.store().write(|ctx| Ok(ctx.tx.query_row("SELECT id FROM undo_entry WHERE consumed = 0 ORDER BY at_ms DESC, rowid DESC LIMIT 1", [], |r| r.get::<_, String>(0)).ok())).ok().flatten()
    }

    pub fn run(&self, command: Command) -> Result<CommandOutcome, CoreError> {
        let undo = matches!(command, Command::Undo);
        let reversed = if undo { self.newest_undo() } else { None };
        let command = self.whole_conversations(command);
        let acted = matches!(command, Command::MarkSpam { .. } | Command::Archive { .. } | Command::Snooze { .. } | Command::Label { .. }).then(|| command.clone());   // fix-suggest: the act, for its offer
        let out = self.actions.run(command)?;
        // the local write is committed: its server rows are there for the worker
        self.worker.kick();
        // fix-suggest: the follow-up offered after the act (suggest_offers.rs), one call line per offering act
        let out = match &acted {
            Some(Command::MarkSpam { message_ids }) => crate::suggest_offers::after_junk(self.store(), message_ids, out),
            Some(Command::Archive { message_ids }) => crate::suggest_offers::after_archive(self.store(), message_ids, out),
            Some(Command::Snooze { message_ids, .. }) => crate::suggest_offers::after_snooze(self.store(), message_ids, out),
            Some(Command::Label { .. }) => crate::suggest_offers::after_label(self.store(), out),
            _ => out,
        };
        Ok(if undo { CommandOutcome { undo_entry_id: reversed, created_id: out.created_id } } else { out })
    }

    /// A list row is a conversation (store `other_row_of_thread`): Done, Snooze, Junk and "mark read" on a row take every mail of the
    /// conversation the inbox still shows, so the conversation leaves the list whole and its older mail does not come up as a new row.
    /// "Mark unread" stays on the given mail (the row's own, its newest). The undo entry records the widened set, so Z brings all back.
    fn whole_conversations(&self, command: Command) -> Command {
        let widen = |ids: Vec<String>| self.store().conversation_messages(&ids).unwrap_or(ids);
        match command {
            Command::Archive { message_ids } => Command::Archive { message_ids: widen(message_ids) },
            Command::Snooze { message_ids, when } => Command::Snooze { message_ids: widen(message_ids), when },
            Command::MarkSpam { message_ids } => Command::MarkSpam { message_ids: widen(message_ids) },
            Command::MarkRead { message_ids, read: true } => Command::MarkRead { message_ids: widen(message_ids), read: true },
            other => other,
        }
    }

    /// `OpenThread`: the owner opened the thread, so every message of it is read. It is the one path for "mark read" (the actions
    /// crate's MarkRead: the local flag and the server's `\Seen` row in one transaction), and it records no undo entry (opening a
    /// thread is not undone with Z). An unknown thread is `NotFound`, as the fixture core would for a thread it does not hold.
    pub fn open_thread(&self, thread_id: &str) -> Result<CommandOutcome, CoreError> {
        let thread = self
            .store()
            .thread(thread_id)
            .map_err(|e| CoreError::Unexpected { reason_words: format!("the thread could not be read: {e}") })?
            .ok_or_else(|| CoreError::NotFound { kind: "thread".into(), id: thread_id.into() })?;
        self.run(Command::MarkRead { message_ids: thread.message_ids, read: true })?;
        Ok(ok(None, None))
    }

    /// A draft as the outbox sends it: a reply with no recipient answers its thread (the quick reply's rule).
    fn complete(&self, draft: Draft) -> Result<Draft, CoreError> {
        let nobody = draft.to.iter().chain(&draft.cc).chain(&draft.bcc).all(|a| a.trim().is_empty());
        match (&draft.reply_to_thread_id, nobody) {
            (Some(thread), true) => {
                let r = quick_reply_draft(self.store(), thread, Some(&draft.from_account_id), draft.body.clone(), draft.id.clone())?;
                Ok(Draft { to: r.to, subject: if draft.subject.trim().is_empty() { r.subject } else { draft.subject }, ..draft })
            }
            (None, true) => Err(CoreError::Invalid { field: "draft".into(), reason_words: "add a recipient before you send".into() }),
            _ => Ok(draft),
        }
    }

    fn queued(&self, entry_id: String) -> Result<CommandOutcome, CoreError> {
        let undo = RecordSend::new(self.env().clone()).execute(entry_id.clone())?;
        self.mail.kick();
        Ok(ok(Some(entry_id), Some(undo)))
    }

    pub fn send(&self, command: Command) -> Result<CommandOutcome, CoreError> {
        let loader = outbox::adapters::store::StoreOutbox::new(self.store().clone());
        let fresh = || store::new_id("draft-quick");
        match command {
            Command::Send { draft_id } => {
                let draft = self.complete(LoadDraftPort::execute(&loader, &draft_id)?)?;
                crate::draft_files::check_sendable(self.store(), &draft_id)?;   // delta 22: nothing is queued half-attached
                let view = self.mail.outbox.send(draft)?;
                self.queued(view.entry.id)
            }
            Command::QuickSend { thread_id, body } => {
                let draft = quick_reply_draft(self.store(), &thread_id, None, body, fresh())?;
                let view = self.mail.outbox.send(draft)?;
                self.queued(view.entry.id)
            }
            Command::QuickSendFrom { thread_id, body, from_account_id } => {
                let draft = quick_reply_draft(self.store(), &thread_id, Some(&from_account_id), body, fresh())?;
                let view = self.mail.outbox.send(draft)?;
                self.queued(view.entry.id)
            }
            Command::Schedule { draft_id, at_ms } => {
                let draft = self.complete(LoadDraftPort::execute(&loader, &draft_id)?)?;
                crate::draft_files::check_sendable(self.store(), &draft_id)?;
                let view = self.mail.outbox.schedule(draft, at_ms)?;
                self.mail.kick();
                Ok(ok(Some(view.entry.id), None))
            }
            Command::SendNow { outbox_id } => { self.mail.send_now(&outbox_id)?; Ok(ok(None, None)) }
            Command::CancelSend { outbox_id } => {
                if self.mail.outbox.get(&outbox_id)?.is_none() { return Err(CoreError::NotFound { kind: "outbox entry".into(), id: outbox_id }); }
                self.mail.outbox.cancel(&outbox_id)?;
                Ok(ok(None, None))
            }
            _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "another part of the core handles this command".into() }),
        }
    }
}
